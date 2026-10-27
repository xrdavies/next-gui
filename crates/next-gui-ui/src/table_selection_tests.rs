use super::*;

fn grid() -> (Ui, WidgetId) {
    let mut ui = Ui::new();
    let mut id = WidgetId(0);
    ui.column(ColumnStyle::default(), |c| {
        id = c.table(
            (0..6)
                .map(|i| TableColumn::new(i.to_string(), 100.0))
                .collect(),
            (0..6)
                .map(|r| (0..6).map(|c| format!("{r}:{c}")).collect())
                .collect(),
            TableStyle {
                width: 320.0,
                height: 140.0,
                padding: 0.0,
                ..TableStyle::default()
            },
        );
    });
    ui.set_focus(Some(id));
    (ui, id)
}

#[test]
fn shift_selection_keeps_its_anchor_when_extending_and_reversing() {
    let (mut ui, id) = grid();
    ui.set_table_selected_cells(id, Some((3, 3)), Some((3, 3)));
    for _ in 0..3 {
        assert!(ui.key_input(UiKey::SelectUp));
        assert!(ui.key_input(UiKey::SelectLeft));
    }
    assert_eq!(ui.table_selection_anchor(id), Some((3, Some(3))));
    assert_eq!(ui.table_selected_rows(id), Some((0, 3)));
    assert_eq!(ui.table_selected_cols(id), Some((0, 3)));
    assert_eq!(
        ui.table_selected_text(id).unwrap(),
        "0:0\t0:1\t0:2\t0:3\n1:0\t1:1\t1:2\t1:3\n2:0\t2:1\t2:2\t2:3\n3:0\t3:1\t3:2\t3:3"
    );
    for _ in 0..4 {
        ui.key_input(UiKey::SelectDown);
        ui.key_input(UiKey::SelectRight);
    }
    assert_eq!(ui.table_selection_anchor(id), Some((3, Some(3))));
    assert_eq!(ui.table_selected_rows(id), Some((3, 4)));
    assert_eq!(ui.table_selected_cols(id), Some((3, 4)));
    ui.key_input(UiKey::Left);
    assert_eq!(ui.table_selected_rows(id), Some((4, 4)));
    assert_eq!(ui.table_selected_cols(id), Some((3, 3)));
}

#[test]
fn replacing_rows_clamps_both_endpoints_and_clears_empty_selection() {
    let (mut ui, id) = grid();
    ui.set_table_selected_cells(id, Some((5, 1)), Some((5, 1)));
    ui.set_table_rows(id, vec![vec!["a".into(); 6]; 3]);
    assert_eq!(ui.table_selection_anchor(id), Some((2, Some(5))));
    assert_eq!(ui.table_selected_rows(id), Some((1, 2)));
    assert_eq!(
        ui.table_selected_text(id).as_deref(),
        Some("a\ta\ta\ta\ta\na\ta\ta\ta\ta")
    );
    ui.set_table_rows(id, Vec::new());
    assert_eq!(ui.table_selected_rows(id), None);
    assert_eq!(ui.table_selected_cols(id), None);
    assert_eq!(ui.table_selected_text(id), None);
    ui.set_table_rows(id, vec![vec!["new".into(); 6]]);
    assert_eq!(ui.table_selected_text(id), None);
}

#[test]
fn keyboard_reveals_the_active_column_without_resetting_horizontal_scroll() {
    let (mut ui, id) = grid();
    ui.set_table_frozen_cols(id, 1);
    ui.set_table_selected_cells(id, Some((0, 0)), Some((1, 1)));
    for _ in 0..3 {
        ui.key_input(UiKey::Right);
    }
    assert_eq!(ui.table_scroll_offset(id), Some((180.0, 0.0)));
    ui.key_input(UiKey::Down);
    assert_eq!(ui.table_scroll_offset(id), Some((180.0, 0.0)));
    for _ in 0..3 {
        ui.key_input(UiKey::Left);
    }
    assert_eq!(ui.table_scroll_offset(id), Some((0.0, 0.0)));
    ui.set_table_scroll_offset(id, 150.0, 0.0);
    ui.set_table_selected_col(id, Some(0));
    ui.key_input(UiKey::Down);
    assert_eq!(ui.table_scroll_offset(id), Some((150.0, 0.0)));
}

#[test]
fn programmatic_reveal_keeps_the_range_anchor_focus_and_frozen_column() {
    let (mut ui, id) = grid();
    ui.set_table_frozen_cols(id, 1);
    ui.set_table_selected_cells(id, Some((2, 5)), Some((1, 5)));
    ui.reveal_table_selection(id);
    assert_eq!(ui.table_scroll_offset(id), Some((280.0, 44.0)));
    assert_eq!(ui.table_selection_anchor(id), Some((2, Some(1))));
    assert_eq!(ui.table_selected_rows(id), Some((2, 5)));
    assert_eq!(ui.table_selected_cols(id), Some((1, 5)));
    assert_eq!(ui.focused(), Some(id));
    ui.set_table_selected_cells(id, Some((0, 0)), Some((0, 0)));
    ui.reveal_table_selection(id);
    assert_eq!(ui.table_scroll_offset(id), Some((280.0, 0.0)));
    ui.set_table_rows(id, Vec::new());
    ui.reveal_table_selection(id);
    assert_eq!(ui.table_selected_row(id), None);
    assert_eq!(ui.table_scroll_offset(id).unwrap().1, 0.0);
    ui.reveal_table_selection(WidgetId(u64::MAX));
}

#[test]
fn visible_rows_counts_half_rows_and_uses_the_current_viewport() {
    let (mut ui, id) = grid();
    assert_eq!(ui.table_visible_rows(id), Some(4));
    ui.set_table_scroll_offset(id, 0.0, 13.0);
    assert_eq!(ui.table_visible_rows(id), Some(5));
    ui.set_table_scroll_offset(id, 0.0, 14.0);
    assert_eq!(ui.table_visible_rows(id), Some(4));
    let Kind::Table { style, .. } = &mut ui.widgets.get_mut(&id).unwrap().kind else { unreachable!() };
    style.height = 40.0;
    assert_eq!(ui.table_visible_rows(id), Some(0));
    let Kind::Table { style, .. } = &mut ui.widgets.get_mut(&id).unwrap().kind else { unreachable!() };
    style.height = 1000.0;
    ui.set_table_scroll_offset(id, 0.0, 0.0);
    assert_eq!(ui.table_visible_rows(id), Some(6));
    ui.set_table_rows(id, Vec::new());
    assert_eq!(ui.table_visible_rows(id), Some(0));
    assert_eq!(ui.table_visible_rows(WidgetId(u64::MAX)), None);
}

#[test]
fn copy_quotes_tabs_line_breaks_and_quotes() {
    let (mut ui, id) = grid();
    ui.set_table_rows(
        id,
        vec![vec!["a\tb".into(), "x\ny".into(), "say \"hi\"".into()]],
    );
    ui.set_table_selected_cells(id, Some((0, 0)), Some((2, 0)));
    assert_eq!(
        ui.table_selected_text(id).as_deref(),
        Some("\"a\tb\"\t\"x\ny\"\t\"say \"\"hi\"\"\"")
    );
    ui.set_table_selected_rows(id, None);
    assert_eq!(ui.table_selected_text(id), None);
}

#[test]
fn drag_selection_respects_scrolled_frozen_columns_and_keeps_its_anchor() {
    let (mut ui, id) = grid();
    let mut scene = Scene::new();
    if let Kind::Table { style, .. } = &mut ui.widgets.get_mut(&id).unwrap().kind {
        style.drag_select = true;
    }
    ui.set_table_frozen_cols(id, 1);
    ui.set_table_scroll_offset(id, 150.0, 26.0);
    ui.sync(&mut scene);
    let bounds = scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
    let y = bounds.y + ui.table_style().header_height + 5.0;
    assert!(ui.pointer_pressed(&mut scene, bounds.x + 120.0, y));
    assert!(ui.is_dragging());
    assert_eq!(ui.table_selection_anchor(id), Some((1, Some(2))));
    assert!(ui.pointer_moved(bounds.x + 50.0, y + 52.0));
    assert_eq!(ui.table_selected_cols(id), Some((0, 2)));
    assert_eq!(ui.table_selected_rows(id), Some((1, 3)));
    assert_eq!(ui.table_selection_anchor(id), Some((1, Some(2))));
    assert!(!ui.pointer_moved(f32::NAN, y));
    assert!(ui.pointer_moved(bounds.x + 1000.0, bounds.y + 1000.0));
    assert_eq!(ui.table_selected_col(id), Some(5));
    assert_eq!(ui.table_selected_row(id), Some(5));
    ui.pointer_released();
    assert!(!ui.is_dragging());
    let selection = ui.table_selected_text(id);
    ui.pointer_moved(0.0, 0.0);
    assert_eq!(ui.table_selected_text(id), selection);
}

#[test]
fn default_table_body_drag_still_pans_without_extending_the_selection() {
    let (mut ui, id) = grid();
    let mut scene = Scene::new();
    ui.sync(&mut scene);
    let bounds = scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
    let x = bounds.x + 120.0;
    let y = bounds.y + 60.0;
    ui.pointer_pressed(&mut scene, x, y);
    assert!(ui.pointer_moved(x - 80.0, y - 30.0));
    assert_eq!(ui.table_scroll_offset(id), Some((80.0, 30.0)));
    assert_eq!(ui.table_selected_rows(id), Some((1, 1)));
    assert_eq!(ui.table_selected_cols(id), Some((1, 1)));
}
