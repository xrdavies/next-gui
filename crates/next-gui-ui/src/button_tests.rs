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
