use super::*;

#[test]
fn disabled_buttons_skip_focus_and_cancel_pending_keyboard_activation() {
    let mut ui = Ui::new();
    let (mut first, mut middle, mut last) = (WidgetId(0), WidgetId(0), WidgetId(0));
    ui.column(ColumnStyle::default(), |col| {
        first = col.button("First");
        middle = col.tool_button("Middle");
        last = col.button("Last");
    });
    assert_eq!(ui.button_enabled(middle), Some(true));
    ui.set_focus(Some(middle));
    assert!(ui.key_input(UiKey::Enter));
    assert!(ui.set_button_enabled(middle, false));
    assert!(!ui.set_button_enabled(middle, false));
    assert_eq!(ui.take_button_activate(), None);
    assert_eq!(ui.focused(), None);
    ui.set_focus(Some(middle));
    assert_eq!(ui.focused(), None);
    assert!(!ui.key_input(UiKey::Space));
    assert_eq!(ui.focusable_widgets(), vec![first, last]);
    assert_eq!(ui.focus_next(), Some(first));
    assert_eq!(ui.focus_next(), Some(last));
    assert_eq!(ui.focus_prev(), Some(first));
    assert!(ui.set_button_enabled(middle, true));
    assert_eq!(ui.focus_next(), Some(middle));
    assert!(ui.key_input(UiKey::Space));
    assert_eq!(ui.take_button_activate(), Some(middle));
    assert_eq!(ui.button_enabled(WidgetId(0)), None);
    assert!(!ui.set_button_enabled(WidgetId(0), false));
}

#[test]
fn disabled_button_consumes_pointer_without_stealing_focus() {
    let mut ui = Ui::new();
    let mut scene = Scene::new();
    let (mut input, mut button) = (WidgetId(0), WidgetId(0));
    ui.column(
        ColumnStyle {
            width: 300.0,
            height: 150.0,
            ..ColumnStyle::default()
        },
        |col| {
            input = col.text_input("Keep focus");
            button = col.button("Unavailable");
        },
    );
    ui.set_button_enabled(button, false);
    ui.sync(&mut scene);
    let bounds = scene.get(ui.widgets[&button].node.unwrap()).unwrap().bounds;
    let (x, y) = (bounds.x + 8.0, bounds.y + 8.0);
    ui.set_focus(Some(input));
    assert_eq!(ui.hit_widget(&scene, x, y), Some(button));
    assert!(ui.is_button(button));
    assert!(ui.pointer_pressed(&mut scene, x, y));
    assert_eq!(ui.focused(), Some(input));
    assert_eq!(ui.take_button_activate(), None);
    ui.set_button_enabled(button, true);
    assert!(!ui.pointer_pressed(&mut scene, x, y));
    assert_eq!(ui.focused(), Some(button));
}

#[test]
fn disabled_button_theme_colors_restore_authored_colors_and_geometry() {
    for theme in [Theme::light(), Theme::dark()] {
        let mut ui = Ui::new();
        ui.set_theme(theme);
        let mut scene = Scene::new();
        let (mut regular, mut tool) = (WidgetId(0), WidgetId(0));
        let bg = Color::rgb(11, 22, 33);
        let fg = Color::rgb(220, 230, 240);
        ui.column(
            ColumnStyle {
                width: 300.0,
                height: 160.0,
                ..ColumnStyle::default()
            },
            |col| {
                regular = col.button_styled("Action", bg, fg);
                tool = col.tool_button_styled("Tool action", bg, fg);
            },
        );
        ui.sync(&mut scene);
        for id in [regular, tool] {
            let node = ui.widgets[&id].node.unwrap();
            let label = ui.widgets[&id].label_node.unwrap();
            let bounds = scene.get(node).unwrap().bounds;
            assert_eq!(scene.get(node).unwrap().fill, bg);
            assert_eq!(scene.get(label).unwrap().fill, fg);
            ui.set_button_enabled(id, false);
            ui.sync(&mut scene);
            assert_eq!(scene.get(node).unwrap().fill, theme.colors.muted);
            assert_eq!(scene.get(label).unwrap().fill, theme.colors.disabled);
            assert_eq!(scene.get(node).unwrap().bounds, bounds);
            ui.set_button_enabled(id, true);
            ui.sync(&mut scene);
            assert_eq!(scene.get(node).unwrap().fill, bg);
            assert_eq!(scene.get(label).unwrap().fill, fg);
            assert_eq!(scene.get(node).unwrap().bounds, bounds);
        }
    }
}


#[test]
fn icon_buttons_keep_live_help_focus_activation_disabled_colors_and_reversible_text_sizing() {
    for theme in [Theme::light(), Theme::dark()] {
        for density in [UiDensity::Compact, UiDensity::Default, UiDensity::Comfortable] {
            let mut ui = Ui::new(); ui.set_theme(theme); ui.set_config(UiConfig { density, ..UiConfig::default() });
            ui.set_viewport(360.0, 220.0); let mut scene = Scene::new();
            let (mut icon, mut text) = (WidgetId(0), WidgetId(0));
            ui.row(RowStyle { width: 360.0, ..RowStyle::default() }, |row| {
                icon = row.tool_icon_button(IconName::Save, "Save complete database");
                text = row.tool_button("Text action");
            });
            ui.sync(&mut scene);
            let node = ui.widgets[&icon].node.unwrap(); let label = ui.widgets[&icon].label_node.unwrap();
            let bounds = scene.get(node).unwrap().bounds; let glyph = scene.get(label).unwrap();
            assert_eq!(bounds.width, bounds.height); assert!(bounds.width >= 24.0 && bounds.width < 40.0);
            assert!((glyph.bounds.x - bounds.x - (bounds.width - glyph.bounds.width) * 0.5).abs() < 0.001);
            assert!((glyph.bounds.y - bounds.y - (bounds.height - glyph.bounds.height) * 0.5).abs() < 0.001);
            assert!(glyph.mask.as_ref().unwrap().pixels.iter().any(|&p| p > 0));
            assert_eq!(ui.accessible_name(icon).as_deref(), Some("Save complete database"));
            assert_eq!(ui.focus_next(), Some(icon)); assert!(ui.key_input(UiKey::Enter)); assert_eq!(ui.take_button_activate(), Some(icon));
            assert_eq!(ui.focus_next(), Some(text));
            let (x, y) = (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5);
            assert_eq!(ui.hit_widget(&scene, x, y), Some(icon));
            assert!(!ui.pointer_pressed(&mut scene, x, y)); assert_eq!(ui.focused(), Some(icon));
            let tip = ui.find_tooltip_ancestor(icon).unwrap();
            ui.set_button_label(icon, "Save changes"); ui.pointer_hover(&scene, x, y); ui.sync(&mut scene);
            let tip_label = scene.get(ui.widgets[&tip].aux[1]).unwrap();
            let expected = ui.font.rasterize_text("Save changes", ui.tooltip_style().size, None).1;
            assert_eq!(tip_label.mask.as_ref().unwrap().pixels, expected.pixels);
            ui.set_button_enabled(icon, false); ui.sync(&mut scene);
            assert_eq!(scene.get(label).unwrap().fill, theme.colors.disabled); assert_eq!(scene.get(node).unwrap().bounds, bounds);
            assert!(!ui.focusable_widgets().contains(&icon)); assert!(ui.pointer_pressed(&mut scene, x, y));
            assert_eq!(ui.take_button_activate(), None);
            ui.set_button_enabled(icon, true); ui.sync(&mut scene);
            assert_eq!(scene.get(label).unwrap().fill, theme.colors.button_fg);
            assert!(ui.set_button_icon(icon, None)); ui.sync(&mut scene);
            assert!(scene.get(node).unwrap().bounds.width > bounds.width * 2.0);
            assert!(!ui.set_button_icon(icon, None)); assert!(!ui.set_button_icon(WidgetId(0), Some(IconName::Save)));
            assert!(ui.set_button_icon(icon, Some(IconName::Open))); ui.sync(&mut scene);
            assert_eq!(scene.get(node).unwrap().bounds, bounds);
        }
    }
}

#[test]
fn icon_label_layouts_fit_cjk_large_fonts_keep_live_names_and_restore_plain_buttons() {
    let font = Ui::new().font().clone();
    for mut theme in [Theme::light(), Theme::dark()] {
        for size in [13.0, 28.0] {
            theme.typography.button_sm = size;
            for density in [UiDensity::Compact, UiDensity::Default, UiDensity::Comfortable] {
                let mut ui = Ui::with_font(font.clone()); ui.set_theme(theme); ui.set_config(UiConfig { density, ..UiConfig::default() });
                ui.set_viewport(900.0, 400.0); let mut scene = Scene::new(); let mut id = WidgetId(0);
                ui.column(ColumnStyle { width: 880.0, height: 380.0, ..ColumnStyle::default() }, |col| {
                    id = col.tool_icon_button(IconName::Save, "Save 数据库");
                });
                ui.sync(&mut scene);
                let (node, label) = (ui.widgets[&id].node.unwrap(), ui.widgets[&id].label_node.unwrap());
                let square = scene.get(node).unwrap().bounds;
                let mut dimensions = Vec::new();
                for layout in [ButtonIconLayout::IconOnly, ButtonIconLayout::TextOnly, ButtonIconLayout::TextBesideIcon, ButtonIconLayout::TextUnderIcon] {
                    ui.set_button_icon_layout(id, layout); ui.sync(&mut scene);
                    let bounds = scene.get(node).unwrap().bounds; let content = scene.get(label).unwrap();
                    assert!(content.bounds.x >= bounds.x && content.bounds.y >= bounds.y);
                    assert!(content.bounds.x+content.bounds.width <= bounds.x+bounds.width);
                    assert!(content.bounds.y+content.bounds.height <= bounds.y+bounds.height);
                    assert!(content.mask.as_ref().unwrap().pixels.iter().any(|&p| p != 0));
                    assert_eq!(ui.hit_widget(&scene, bounds.x+bounds.width*0.5, bounds.y+bounds.height*0.5), Some(id));
                    assert_eq!(ui.accessible_name(id).as_deref(), Some("Save 数据库"));
                    ui.set_focus(Some(id)); assert!(ui.key_input(UiKey::Enter)); assert_eq!(ui.take_button_activate(), Some(id));
                    ui.set_button_enabled(id, false); ui.sync(&mut scene);
                    assert_eq!(scene.get(label).unwrap().fill, theme.colors.disabled);
                    assert_eq!(scene.get(node).unwrap().bounds, bounds);
                    ui.set_button_enabled(id, true); ui.sync(&mut scene);
                    dimensions.push(bounds);
                }
                assert_eq!(dimensions[0], square);
                assert!(dimensions[2].width > dimensions[1].width);
                assert!(dimensions[3].height > dimensions[2].height);
                ui.set_button_label(id, "Save a longer database name"); ui.sync(&mut scene);
                assert!(scene.get(node).unwrap().bounds.width > dimensions[3].width);
                assert_eq!(ui.accessible_name(id).as_deref(), Some("Save a longer database name"));
                ui.set_button_label(id, ""); ui.sync(&mut scene);
                assert!(scene.get(label).unwrap().mask.as_ref().unwrap().pixels.iter().any(|&p| p != 0));
                ui.set_button_label(id, "Save 数据库"); ui.set_button_icon(id, None); ui.sync(&mut scene);
                assert_eq!(scene.get(node).unwrap().bounds, dimensions[1]);
                assert!(!ui.set_button_icon_layout(id, ButtonIconLayout::TextUnderIcon));
                assert!(!ui.set_button_icon_layout(WidgetId(0), ButtonIconLayout::TextOnly));
            }
        }
    }
}
