use super::*;

fn grid() -> (Ui, WidgetId) {
    let mut ui = Ui::new();
    let mut id = WidgetId(0);
    ui.column(ColumnStyle::default(), |c| {
        id = c.table(
            (0..6).map(|i| TableColumn::new(i.to_string(), 100.0)).collect(),
            (0..6).map(|r| (0..6).map(|c| format!("{r}:{c}")).collect()).collect(),
            TableStyle { width: 320.0, height: 140.0, padding: 0.0, ..TableStyle::default() },
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
    assert_eq!(ui.table_selected_text(id).unwrap(),
        "0:0\t0:1\t0:2\t0:3\n1:0\t1:1\t1:2\t1:3\n2:0\t2:1\t2:2\t2:3\n3:0\t3:1\t3:2\t3:3");
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
    assert_eq!(ui.table_selected_text(id).as_deref(), Some("a\ta\ta\ta\ta\na\ta\ta\ta\ta"));
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
    for _ in 0..3 { ui.key_input(UiKey::Right); }
    assert_eq!(ui.table_scroll_offset(id), Some((180.0, 0.0)));
    ui.key_input(UiKey::Down);
    assert_eq!(ui.table_scroll_offset(id), Some((180.0, 0.0)));
    for _ in 0..3 { ui.key_input(UiKey::Left); }
    assert_eq!(ui.table_scroll_offset(id), Some((0.0, 0.0)));
    ui.set_table_scroll_offset(id, 150.0, 0.0);
    ui.set_table_selected_col(id, Some(0));
    ui.key_input(UiKey::Down);
    assert_eq!(ui.table_scroll_offset(id), Some((150.0, 0.0)));
}

#[test]
fn copy_quotes_tabs_line_breaks_and_quotes() {
    let (mut ui, id) = grid();
    ui.set_table_rows(id, vec![vec!["a\tb".into(), "x\ny".into(), "say \"hi\"".into()]]);
    ui.set_table_selected_cells(id, Some((0, 0)), Some((2, 0)));
    assert_eq!(ui.table_selected_text(id).as_deref(), Some("\"a\tb\"\t\"x\ny\"\t\"say \"\"hi\"\"\""));
    ui.set_table_selected_rows(id, None);
    assert_eq!(ui.table_selected_text(id), None);
}
