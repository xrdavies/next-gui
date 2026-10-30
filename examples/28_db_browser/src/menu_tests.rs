use super::*;
use crate::{Demo, PresentBackend};
use next_gui_render::{encode_scene, Renderer};
use next_gui_ui::UiKey;

#[test]
fn file_menu_groups_preserve_actions_and_shorten_unicode_without_changing_paths() {
    let mut state = AppState::empty();
    state.recent.push_back("previous test session".into());
    state.load_recent();
    assert!(state.recent.is_empty());
    let ui = Ui::new();
    let empty = file_menu_items(&state, ui.font(), 13.0, 900.0);
    assert!(
        !empty
            .iter()
            .find(|item| item.id == mid::FILE_RECENT)
            .unwrap()
            .enabled
    );
    state.recent = [
        "/tmp/数据库/".to_string() + &"😀".repeat(60) + "/数据.sqlite",
        "/tmp/short.sqlite".into(),
        "a".into(),
        "b".into(),
        "c".into(),
        "d".into(),
    ]
    .into();
    let paths = state.recent.clone();
    let items = file_menu_items(&state, ui.font(), 13.0, 900.0);
    let recent = &items
        .iter()
        .find(|item| item.id == mid::FILE_RECENT)
        .unwrap()
        .children;
    assert_eq!(recent.len(), 5);
    assert_eq!(recent[0].label.chars().count(), 51);
    assert!(recent[0].label.starts_with("1. …"));
    assert!(recent[0].label.ends_with("/数据.sqlite"));
    assert_eq!(recent[1].label, "2. /tmp/short.sqlite");
    assert_eq!(state.recent, paths);
    let narrow = file_menu_items(&state, ui.font(), 24.0, 400.0);
    let label = &narrow
        .iter()
        .find(|item| item.id == mid::FILE_RECENT)
        .unwrap()
        .children[0]
        .label;
    assert!(ui.font().measure_line(label, 24.0).0 <= 400.0);
    assert!(label.ends_with("/数据.sqlite"));
    for (parent, expected) in [
        (
            mid::FILE_IMPORT,
            vec![
                mid::FILE_IMPORT_SQL,
                mid::FILE_IMPORT_CSV,
                mid::FILE_IMPORT_CLIP,
            ],
        ),
        (
            mid::FILE_EXPORT,
            vec![
                mid::FILE_EXPORT_SQL,
                mid::FILE_EXPORT_CSV,
                mid::FILE_EXPORT_JSON,
            ],
        ),
        (
            mid::FILE_RECENT,
            (mid::FILE_RECENT_BASE..mid::FILE_RECENT_BASE + 5).collect(),
        ),
    ] {
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == parent)
                .unwrap()
                .children
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            expected
        );
    }
    let leaves: Vec<_> = items
        .iter()
        .filter(|item| item.enabled && item.children.is_empty())
        .map(|item| item.id)
        .collect();
    assert_eq!(
        leaves,
        vec![
            mid::FILE_NEW,
            mid::FILE_NEW_MEM,
            mid::FILE_OPEN,
            mid::FILE_OPEN_RO,
            mid::FILE_ATTACH,
            mid::FILE_SAVE_AS,
            mid::FILE_CLOSE,
            mid::FILE_WRITE,
            mid::FILE_REVERT,
            mid::FILE_VACUUM,
            mid::FILE_PROJECT_OPEN,
            mid::FILE_PROJECT_SAVE,
            mid::FILE_PROJECT_SAVE_AS,
            mid::FILE_PROJECT_CLOSE,
            mid::FILE_SAVE_ALL,
            mid::FILE_PRINT_SCHEMA,
            mid::FILE_EXIT
        ]
    );
}

fn open_group(demo: &mut Demo, parent: u64) {
    demo.ui.close_popup();
    demo.ui.set_focus(Some(demo.ids.menu_file));
    assert!(demo.ui.key_input(UiKey::Down));
    let items = file_menu_items(
        &demo.state,
        demo.ui.font(),
        demo.theme.typography.body,
        demo.win_w as f32 - 48.0,
    );
    for item in items.iter().filter(|item| item.enabled) {
        if item.id == parent {
            break;
        }
        assert!(demo.ui.key_input(UiKey::Down));
    }
    assert!(demo.ui.key_input(UiKey::Right));
    demo.ui.sync(&mut demo.scene);
}

fn capture(demo: &mut Demo, folder: &std::path::Path, name: &str) {
    demo.scene
        .mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
    let dirty = demo.scene.take_dirty();
    let commands = encode_scene(&demo.scene, &dirty);
    let clear = clear_color(&demo.theme);
    let mut soft = next_gui_render_soft::SoftRenderer::new(demo.win_w, demo.win_h, clear);
    soft.render(&commands, &dirty);
    let pixels = soft.frame_pixels().unwrap().0;
    crate::write_ppm(
        folder.join(format!("{name}-soft.ppm")).to_str().unwrap(),
        pixels,
        demo.win_w,
        demo.win_h,
    )
    .unwrap();
    let mut gpu = next_gui_render_gpu::GpuRenderer::try_new(demo.win_w, demo.win_h, clear)
        .expect("Menu visual QA requires an available GPU");
    gpu.render(&commands, &dirty);
    let rendered = gpu.frame_pixels().unwrap().0;
    crate::write_ppm(
        folder.join(format!("{name}-metal.ppm")).to_str().unwrap(),
        rendered,
        demo.win_w,
        demo.win_h,
    )
    .unwrap();
    let delta = pixels
        .iter()
        .zip(rendered)
        .flat_map(|(a, b)| {
            [0, 8, 16]
                .map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())
        })
        .max()
        .unwrap();
    assert!(delta <= 2, "{name}: full-frame delta={delta}");
    eprintln!("Menu QA {name}: full-frame delta={delta}");
}

#[test]
fn file_submenus_dispatch_existing_actions_and_keep_sources_in_wide_and_narrow_layouts() {
    let folder = std::env::var_os("NEXT_GUI_MENUS_QA_OUT").map(std::path::PathBuf::from);
    if let Some(folder) = &folder {
        std::fs::create_dir_all(folder).unwrap();
    }
    for (name, theme, width, height, font_size) in [
        ("dark-wide", Theme::dark(), 1400, 860, 13.0),
        ("light-wide", Theme::light(), 1400, 860, 18.0),
        ("dark-narrow", Theme::dark(), 800, 600, 24.0),
        ("light-narrow", Theme::light(), 800, 600, 18.0),
    ] {
        let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.recent = (0..5)
            .map(|index| format!("/tmp/{}/数据库-{index}.sqlite", "😀".repeat(60)))
            .collect();
        *demo.state.sql_editor_mut() = "SELECT 'preserve my SQL draft';".into();
        demo.state.prefs.font_size = font_size;
        demo.theme = browser_theme(theme, font_size);
        demo.do_rebuild();
        for (group, first, count, label) in [
            (mid::FILE_RECENT, mid::FILE_RECENT_BASE, 5, "recent"),
            (mid::FILE_IMPORT, mid::FILE_IMPORT_SQL, 3, "import"),
            (mid::FILE_EXPORT, mid::FILE_EXPORT_SQL, 3, "export"),
        ] {
            let expected: Vec<_> =
                file_menu_items(&demo.state, demo.ui.font(), font_size, width as f32 - 48.0)
                    .iter()
                    .find(|item| item.id == group)
                    .unwrap()
                    .children
                    .iter()
                    .map(|item| item.id)
                    .collect();
            assert_eq!(expected[0], first);
            assert_eq!(expected.len(), count);
            for (row, &action) in expected.iter().enumerate() {
                open_group(&mut demo, group);
                for _ in 0..row {
                    assert!(demo.ui.key_input(UiKey::Down));
                }
                demo.ui.sync(&mut demo.scene);
                if row == 0 {
                    if let Some(folder) = &folder {
                        capture(&mut demo, folder, &format!("{name}-{label}"));
                    }
                }
                assert!(demo.ui.key_input(UiKey::Enter));
                assert_eq!(
                    demo.ui.take_menu_action(),
                    Some((demo.ids.menu_file, action))
                );
                assert!(!demo.ui.has_open_popup());
                assert_eq!(demo.ui.focused(), Some(demo.ids.menu_file));
            }
        }
        demo.ui.key_input(UiKey::Down);
        demo.ui.key_input(UiKey::End);
        demo.ui.sync(&mut demo.scene);
        if let Some(folder) = &folder {
            capture(&mut demo, folder, &format!("{name}-end"));
        }
        demo.ui.key_input(UiKey::Escape);
        assert!(!demo.ui.has_open_popup());
        assert!(demo.ui.take_menu_action().is_none());
        assert_eq!(demo.state.sql_editor(), "SELECT 'preserve my SQL draft';");
        assert_eq!(demo.state.path, ":memory:");
        assert!(demo.state.overlay.is_none());
    }
}
