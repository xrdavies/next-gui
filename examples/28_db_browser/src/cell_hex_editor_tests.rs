use super::*;
use crate::state::EditCellMode;
use rusqlite::types::Value;

#[test]
fn cell_binary_editor_preserves_bytes_sessions_types_and_explicit_apply() {
    let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch(CURRENT_PLOT_QA_SQL)
        .unwrap();
    demo.state.prefs.auto_switch_mode = false;
    demo.state.refresh_schema().unwrap();
    demo.state
        .switch_browse_table_in("main", "quarterly_revenue")
        .unwrap();
    demo.state.dock_ratio = 0.62;
    demo.state.body_ratio = 0.45;
    demo.state.dock_focus = DockKind::Plot;
    demo.state.sync_current_plot().unwrap();
    demo.state.plot.check(1, 1, true);
    demo.state.plot.check(2, 2, true);
    let folder = std::env::var("NEXT_GUI_HEX_EDITOR_QA_OUT")
        .ok()
        .map(PathBuf::from);
    if let Some(folder) = &folder {
        std::fs::create_dir_all(folder).unwrap();
    }
    let original = (0..259)
        .map(|index| (index % 256) as u8)
        .collect::<Vec<_>>();
    let stored = |demo: &Demo| {
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .query_row(
                "SELECT region FROM quarterly_revenue WHERE observed='2026-10-01'",
                [],
                |row| row.get::<_, Value>(0),
            )
            .unwrap()
    };
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        for (width, height, size) in [(1400, 860, 13.0), (800, 600, 18.0)] {
            demo.state.discard_cell_draft();
            demo.state.tab = MainTab::Browse;
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .execute(
                    "UPDATE quarterly_revenue SET region=?1 WHERE observed='2026-10-01'",
                    [Value::Blob(original.clone())],
                )
                .unwrap();
            demo.state.write_changes().unwrap();
            demo.state.refresh_browse().unwrap();
            demo.state.goto_browse_record(1).unwrap();
            demo.state.browse_mut().sel_col = Some(3);
            demo.state.sync_edit_cell_from_selection();
            demo.state.set_edit_mode(EditCellMode::Hex).unwrap();
            demo.win_w = width;
            demo.win_h = height;
            demo.state.prefs.font_size = size;
            demo.theme = ui_build::browser_theme(theme, size);
            demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.edit_value));
            demo.ui.sync(&mut demo.scene);
            assert!(demo.ui.text_area_value(demo.ids.edit_value).is_none());
            assert_eq!(
                demo.ui.hex_editor_caret(demo.ids.edit_value),
                Some((0, false))
            );
            assert!(!demo.ui.text_input_wants_ime());
            assert!(!demo.ui.ime_commit("中文"));
            assert!(!demo.state.has_cell_draft());
            assert!(demo
                .ui
                .accessible_name(demo.ids.edit_value)
                .unwrap()
                .contains("Hex and ASCII"));
            demo.ui.select_hex_editor_range(demo.ids.edit_value, 32..51);
            demo.ui.sync(&mut demo.scene);
            if let Some(folder) = &folder {
                crate::plot_export::capture(
                    &mut demo,
                    folder,
                    &format!("hex-selected-{name}-{width}"),
                );
            }
            let (editor, revision) = (demo.ids.edit_value, demo.state.edit_revision);
            assert!(!demo.ui.paste_into_focused_text("00 ff Z"));
            demo.snapshot_inputs();
            assert_eq!(demo.state.edit_bytes.as_slice(), original);
            assert!(demo
                .state
                .error_msg
                .as_ref()
                .unwrap()
                .contains("hexadecimal"));
            demo.clear_error();
            assert_eq!(
                (demo.ids.edit_value, demo.state.edit_revision),
                (editor, revision)
            );
            demo.ui.select_hex_editor_range(editor, 0..1);
            assert!(demo.ui.paste_into_focused_text("41 42"));
            demo.snapshot_inputs();
            let changed = demo.state.edit_bytes.clone();
            assert_eq!(&changed[..3], b"AB\x01");
            assert!(demo.state.has_cell_draft());
            assert!(!demo.state.dirty);
            assert_eq!(stored(&demo), Value::Blob(original.clone()));
            demo.state.tab = MainTab::Pragmas;
            demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.edit_value));
            assert!(!demo.ui.key_input(UiKey::Undo));
            assert!(!demo.ui.key_input(UiKey::Delete));
            assert!(!demo.ui.paste_into_focused_text("ff"));
            if let Some(folder) = &folder {
                demo.ui.sync(&mut demo.scene);
                crate::plot_export::capture(
                    &mut demo,
                    folder,
                    &format!("hex-readonly-{name}-{width}"),
                );
            }
            demo.state.tab = MainTab::Browse;
            demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.edit_value));
            assert!(demo.ui.key_input(UiKey::Undo));
            demo.snapshot_inputs();
            assert_eq!(demo.state.edit_bytes.as_slice(), original);
            assert!(!demo.state.has_cell_draft());
            assert!(demo.ui.key_input(UiKey::Redo));
            demo.snapshot_inputs();
            assert_eq!(*demo.state.edit_bytes, *changed);
            demo.ui.select_hex_editor_range(demo.ids.edit_value, 0..2);
            let readable = demo
                .ui
                .hex_editor_readable(demo.ids.edit_value, 0..2)
                .unwrap();
            let dir = tempfile::tempdir().unwrap();
            let output = dir.path().join("hex.txt");
            demo.state.export_readable_cell(&output, &readable).unwrap();
            assert_eq!(std::fs::read_to_string(&output).unwrap(), readable);
            assert!(readable.contains("41 42"));
            assert!(readable.contains("AB"));
            demo.handle_button(demo.ids.hex_insert);
            assert_eq!(
                demo.ui.hex_editor_overwrite(demo.ids.edit_value),
                Some(true)
            );
            assert!(demo.ui.key_input(UiKey::Delete));
            demo.snapshot_inputs();
            assert_eq!(demo.state.edit_bytes.len(), changed.len());
            assert_eq!(&demo.state.edit_bytes[..2], &[0, 0]);
            assert!(demo.ui.key_input(UiKey::Undo));
            demo.snapshot_inputs();
            assert_eq!(*demo.state.edit_bytes, *changed);
            demo.ui.select_hex_editor_range(demo.ids.edit_value, 0..1);
            assert!(demo.ui.key_input(UiKey::Tab));
            assert!(demo.ui.key_input(UiKey::Char('Z')));
            demo.snapshot_inputs();
            assert_eq!(demo.state.edit_bytes[0], b'Z');
            assert!(demo.ui.key_input(UiKey::Undo));
            demo.snapshot_inputs();
            assert_eq!(*demo.state.edit_bytes, *changed);
            assert!(demo.ui.key_input(UiKey::DocumentEnd));
            demo.ui.sync(&mut demo.scene);
            if let Some(folder) = &folder {
                crate::plot_export::capture(&mut demo, folder, &format!("hex-end-{name}-{width}"));
            }
            assert!(demo.state.has_cell_draft());
            let position = demo.state.browse().sel_col;
            assert!(demo.handle_cell_apply_key(
                KeyCode::Enter,
                Modifiers {
                    meta: true,
                    ..Default::default()
                }
            ));
            demo.do_rebuild();
            assert_eq!(stored(&demo), Value::Blob(changed.as_ref().clone()));
            assert!(demo.state.dirty);
            assert!(!demo.state.has_cell_draft());
            assert_eq!(demo.state.browse().sel_col, position);
            assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
            assert!(
                !demo.ui.key_input(UiKey::Undo),
                "Apply starts a fresh source-scoped session"
            );
            demo.state.write_changes().unwrap();
        }
    }
    demo.state.edit_type = 3;
    demo.state.edit_bytes = std::sync::Arc::new(vec![255]);
    assert!(demo.state.edited_cell_value().is_err());
    demo.state.edit_type = 4;
    demo.state.discard_cell_draft();
    demo.state.set_edit_mode(EditCellMode::Hex).unwrap();
    demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.key_input(UiKey::SelectAll);
    assert!(demo.ui.delete_focused_text_selection());
    demo.snapshot_inputs();
    assert!(demo.state.edit_bytes.is_empty());
    assert_eq!(demo.state.edited_cell_value().unwrap(), Value::Blob(vec![]));
    assert!(demo.ui.key_input(UiKey::Undo));
    demo.snapshot_inputs();
    assert!(!demo.state.has_cell_draft());
    assert_eq!(
        map_key(KeyCode::Insert, Modifiers::default()),
        Some(UiKey::Insert)
    );
    assert_eq!(
        map_key(
            KeyCode::End,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            }
        ),
        Some(UiKey::SelectDocumentEnd)
    );
}
