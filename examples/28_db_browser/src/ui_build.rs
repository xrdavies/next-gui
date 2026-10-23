//! Build the DB Browser chrome from [`crate::state::AppState`].

use next_gui_layout::{Axis, CrossAlign};
use next_gui_scene::{Color, Scene};
use next_gui_text::Font;
use next_gui_ui::{
    AlertTone, ChartStyle, ChartTile, ColumnStyle, ListStyle, ModalStyle, PropertyRow, PropertyValue,
    RowStyle, SelectStyle, SplitterStyle, TableColumn, TableStyle, TabsStyle, TextAreaStyle,
    TextInputStyle, Theme, ThemeMode, ToolbarStyle, TreeNode, TreeStyle, TrendStyle, Ui, WidgetId,
};

use crate::db::ObjectKind;
use crate::state::{AppState, ConfirmKind, MainTab};

#[allow(dead_code)]
pub struct BuiltUi {
    pub ui: Ui,
    pub scene: Scene,
    pub btn_new: WidgetId,
    pub btn_demo: WidgetId,
    pub btn_open: WidgetId,
    pub btn_close: WidgetId,
    pub btn_vacuum: WidgetId,
    pub btn_refresh: WidgetId,
    pub btn_theme: WidgetId,
    pub path_input: WidgetId,
    pub tabs: WidgetId,
    pub tree: WidgetId,
    pub ddl_area: WidgetId,
    pub btn_run_ddl: WidgetId,
    pub btn_drop: WidgetId,
    pub new_table_name: WidgetId,
    pub new_table_cols: WidgetId,
    pub btn_create_table: WidgetId,
    pub browse_select: WidgetId,
    pub browse_filter: WidgetId,
    pub btn_browse_go: WidgetId,
    pub btn_add_row: WidgetId,
    pub btn_del_row: WidgetId,
    pub btn_edit_cell: WidgetId,
    pub edit_col: WidgetId,
    pub edit_value: WidgetId,
    pub browse_table: WidgetId,
    pub pragma_grid: WidgetId,
    pub pragma_value: WidgetId,
    pub btn_pragma_apply: WidgetId,
    pub sql_area: WidgetId,
    pub btn_run_sql: WidgetId,
    pub sql_table: WidgetId,
    pub sql_alert: WidgetId,
    pub chart_select: WidgetId,
    pub chart_col: WidgetId,
    pub btn_chart: WidgetId,
    pub chart: WidgetId,
    pub log_list: WidgetId,
    pub io_path: WidgetId,
    pub btn_import_csv: WidgetId,
    pub btn_export_csv: WidgetId,
    pub btn_export_sql: WidgetId,
    pub btn_import_sql: WidgetId,
    pub status: WidgetId,
    pub modal: WidgetId,
}

pub fn clear_color(theme: &Theme) -> Color {
    match theme.mode {
        ThemeMode::Dark => Color::rgb(0x1a, 0x1a, 0x1c),
        ThemeMode::Light => theme.colors.content_bg,
    }
}

fn schema_tree(state: &AppState) -> Vec<TreeNode> {
    let mut tables = Vec::new();
    let mut views = Vec::new();
    let mut indexes = Vec::new();
    let mut triggers = Vec::new();
    for (i, obj) in state.schema.iter().enumerate() {
        let id = 10_000 + i as u64;
        let node = TreeNode::leaf(id, obj.name.clone());
        match obj.kind {
            ObjectKind::Table => tables.push(node),
            ObjectKind::View => views.push(node),
            ObjectKind::Index => indexes.push(node),
            ObjectKind::Trigger => triggers.push(node),
        }
    }
    vec![
        TreeNode::new(1, format!("Tables ({})", tables.len()), tables),
        TreeNode::new(2, format!("Views ({})", views.len()), views),
        TreeNode::new(3, format!("Indexes ({})", indexes.len()), indexes),
        TreeNode::new(4, format!("Triggers ({})", triggers.len()), triggers),
    ]
}

fn result_columns(cols: &[String], fallback: &[&str]) -> Vec<TableColumn> {
    if cols.is_empty() {
        return fallback
            .iter()
            .map(|t| TableColumn::new(*t, 120.0))
            .collect();
    }
    let n = cols.len().max(1);
    let w = (900.0 / n as f32).clamp(80.0, 220.0);
    cols.iter().map(|c| TableColumn::new(c.clone(), w)).collect()
}

pub fn build_ui(font: Font, theme: Theme, width: u32, height: u32, state: &AppState) -> BuiltUi {
    let mut scene = Scene::new();
    let mut ui = Ui::with_font(font);
    ui.set_theme(theme);

    let mut btn_new = WidgetId(0);
    let mut btn_demo = WidgetId(0);
    let mut btn_open = WidgetId(0);
    let mut btn_close = WidgetId(0);
    let mut btn_vacuum = WidgetId(0);
    let mut btn_refresh = WidgetId(0);
    let mut btn_theme = WidgetId(0);
    let mut path_input = WidgetId(0);
    let mut tabs = WidgetId(0);
    let mut tree = WidgetId(0);
    let mut ddl_area = WidgetId(0);
    let mut btn_run_ddl = WidgetId(0);
    let mut btn_drop = WidgetId(0);
    let mut new_table_name = WidgetId(0);
    let mut new_table_cols = WidgetId(0);
    let mut btn_create_table = WidgetId(0);
    let mut browse_select = WidgetId(0);
    let mut browse_filter = WidgetId(0);
    let mut btn_browse_go = WidgetId(0);
    let mut btn_add_row = WidgetId(0);
    let mut btn_del_row = WidgetId(0);
    let mut btn_edit_cell = WidgetId(0);
    let mut edit_col = WidgetId(0);
    let mut edit_value = WidgetId(0);
    let mut browse_table = WidgetId(0);
    let mut pragma_grid = WidgetId(0);
    let mut pragma_value = WidgetId(0);
    let mut btn_pragma_apply = WidgetId(0);
    let mut sql_area = WidgetId(0);
    let mut btn_run_sql = WidgetId(0);
    let mut sql_table = WidgetId(0);
    let mut sql_alert = WidgetId(0);
    let mut chart_select = WidgetId(0);
    let mut chart_col = WidgetId(0);
    let mut btn_chart = WidgetId(0);
    let mut chart = WidgetId(0);
    let mut log_list = WidgetId(0);
    let mut io_path = WidgetId(0);
    let mut btn_import_csv = WidgetId(0);
    let mut btn_export_csv = WidgetId(0);
    let mut btn_export_sql = WidgetId(0);
    let mut btn_import_sql = WidgetId(0);
    let mut status = WidgetId(0);

    let win_w = width as f32;
    let win_h = height as f32;
    let shell_w = (win_w - 16.0).max(960.0);
    let shell_h = (win_h - 40.0).max(640.0);
    let content_h = shell_h - 110.0;
    let inner_w = shell_w - 40.0;

    let table_names = state.table_names();
    let browse_opts: Vec<String> = if table_names.is_empty() {
        vec!["(no tables)".into()]
    } else {
        table_names
    };
    let browse_sel = state
        .browse_table
        .as_ref()
        .and_then(|t| browse_opts.iter().position(|x| x == t));
    let chart_sel = state
        .chart_table
        .as_ref()
        .and_then(|t| browse_opts.iter().position(|x| x == t));

    let browse_cols = state
        .browse_result
        .as_ref()
        .map(|r| result_columns(&r.columns, &["col"]))
        .unwrap_or_else(|| vec![TableColumn::new("—", 120.0)]);
    let browse_rows = state
        .browse_result
        .as_ref()
        .map(|r| r.rows.clone())
        .unwrap_or_default();
    let sql_cols = state
        .sql_result
        .as_ref()
        .map(|r| result_columns(&r.columns, &["result"]))
        .unwrap_or_else(|| vec![TableColumn::new("result", 200.0)]);
    let sql_rows = state
        .sql_result
        .as_ref()
        .map(|r| r.rows.clone())
        .unwrap_or_default();
    let pragma_rows: Vec<PropertyRow> = state
        .pragmas
        .iter()
        .map(|(k, v)| {
            let value = match k.as_str() {
                "foreign_keys" | "recursive_triggers" => {
                    PropertyValue::Bool(v == "1" || v.eq_ignore_ascii_case("on") || v == "true")
                }
                "journal_mode" => PropertyValue::Select {
                    options: vec![
                        "delete".into(),
                        "truncate".into(),
                        "persist".into(),
                        "memory".into(),
                        "wal".into(),
                        "off".into(),
                    ],
                    selected: ["delete", "truncate", "persist", "memory", "wal", "off"]
                        .iter()
                        .position(|x| x.eq_ignore_ascii_case(v))
                        .unwrap_or(0),
                },
                "synchronous" => PropertyValue::Select {
                    options: vec!["OFF".into(), "NORMAL".into(), "FULL".into(), "EXTRA".into()],
                    selected: match v.as_str() {
                        "0" | "OFF" | "off" => 0,
                        "1" | "NORMAL" | "normal" => 1,
                        "3" | "EXTRA" | "extra" => 3,
                        _ => 2,
                    },
                },
                _ => PropertyValue::Text(v.clone()),
            };
            PropertyRow::new(k.clone(), value)
        })
        .collect();
    let roots = schema_tree(state);
    let log_items: Vec<String> = if state.sql_log.is_empty() {
        vec!["(empty)".into()]
    } else {
        state.sql_log.iter().rev().take(80).cloned().collect()
    };
    let chart_points = if state.chart_points.is_empty() {
        vec![0.0]
    } else {
        state.chart_points.clone()
    };
    let chart_tiles = vec![
        ChartTile::trend(
            format!(
                "{} · {}",
                state.chart_table.as_deref().unwrap_or("—"),
                state.chart_column
            ),
            chart_points,
            TrendStyle::from_theme(&theme),
        ),
        ChartTile::gauge(
            "points",
            state.chart_points.len() as f32,
            0.0,
            64.0,
            "",
            "samples",
            ui.gauge_style(),
        ),
    ];
    let theme_label = match theme.mode {
        ThemeMode::Dark => "Theme: Light",
        ThemeMode::Light => "Theme: Dark",
    };

    let mut col_style = ui.column_style();
    col_style.x = 8.0;
    col_style.y = 6.0;
    col_style.width = shell_w;
    col_style.height = shell_h;
    col_style.padding = 6.0;
    col_style.gap = 6.0;
    col_style.cross_align = CrossAlign::Start;

    let mut tb_style = ToolbarStyle::from_theme(&theme);
    tb_style.width = shell_w - 12.0;
    tb_style.height = 40.0;

    let mut path_style = TextInputStyle::from_theme(&theme);
    path_style.width = 320.0;

    let mut io_row = RowStyle::from_theme(&theme);
    io_row.width = shell_w - 12.0;
    io_row.height = 34.0;
    io_row.gap = 8.0;
    io_row.padding = 0.0;
    io_row.cross_align = CrossAlign::Center;

    let mut io_path_style = TextInputStyle::from_theme(&theme);
    io_path_style.width = 280.0;

    let mut tabs_style = TabsStyle::from_theme(&theme);
    tabs_style.width = shell_w - 12.0;
    tabs_style.height = content_h;

    let mut split_style = SplitterStyle::from_theme(&theme);
    split_style.width = inner_w;
    split_style.height = content_h - 48.0;
    split_style.axis = Axis::Vertical;
    split_style.ratio = 0.32;

    let tree_w = inner_w * 0.32 - 8.0;
    let right_w = inner_w * 0.68 - 8.0;
    let mut tree_style = TreeStyle::from_theme(&theme);
    tree_style.width = tree_w;
    tree_style.height = content_h - 64.0;

    let right_col = ColumnStyle {
        width: right_w,
        height: content_h - 64.0,
        padding: 4.0,
        gap: 6.0,
        cross_align: CrossAlign::Start,
        ..ColumnStyle::from_theme(&theme)
    };
    let mut ddl_style = TextAreaStyle::from_theme(&theme);
    ddl_style.width = right_w - 16.0;
    ddl_style.height = 160.0;

    let mut struct_row = RowStyle::from_theme(&theme);
    struct_row.width = right_w - 16.0;
    struct_row.height = 32.0;
    struct_row.gap = 8.0;
    struct_row.padding = 0.0;

    let mut name_style = TextInputStyle::from_theme(&theme);
    name_style.width = 140.0;
    let mut cols_style = TextInputStyle::from_theme(&theme);
    cols_style.width = 360.0;

    let mut browse_row = RowStyle::from_theme(&theme);
    browse_row.width = inner_w;
    browse_row.height = 34.0;
    browse_row.gap = 8.0;
    browse_row.padding = 0.0;
    browse_row.cross_align = CrossAlign::Center;

    let mut sel_style = SelectStyle::from_theme(&theme);
    sel_style.width = 180.0;
    let mut filter_style = TextInputStyle::from_theme(&theme);
    filter_style.width = 220.0;
    let mut edit_col_style = TextInputStyle::from_theme(&theme);
    edit_col_style.width = 120.0;
    let mut edit_val_style = TextInputStyle::from_theme(&theme);
    edit_val_style.width = 220.0;

    let mut browse_table_style = TableStyle::from_theme(&theme);
    browse_table_style.width = inner_w;
    browse_table_style.height = content_h - 120.0;

    let mut pragma_style = ui.property_grid_style();
    pragma_style.width = inner_w;
    pragma_style.height = content_h - 100.0;
    pragma_style.key_width = 180.0;

    let mut sql_area_style = TextAreaStyle::from_theme(&theme);
    sql_area_style.width = inner_w;
    sql_area_style.height = 150.0;
    let mut sql_table_style = TableStyle::from_theme(&theme);
    sql_table_style.width = inner_w;
    sql_table_style.height = content_h - 260.0;

    let mut chart_row = browse_row;
    let mut chart_col_style = TextInputStyle::from_theme(&theme);
    chart_col_style.width = 140.0;
    let mut chart_style = ChartStyle::from_theme(&theme);
    chart_style.width = inner_w;
    chart_style.height = content_h - 90.0;
    chart_style.columns = 2;

    let mut list_style = ListStyle::from_theme(&theme);
    list_style.width = inner_w;
    list_style.height = content_h - 60.0;

    let roots_for_tree = roots;
    let browse_opts_a = browse_opts.clone();
    let browse_opts_b = browse_opts;

    ui.column(col_style, |col| {
        col.toolbar(tb_style, |tb| {
            tb.text("DB Browser");
            btn_new = tb.tool_button("New");
            btn_demo = tb.tool_button("Demo");
            btn_open = tb.tool_button("Open");
            btn_close = tb.tool_button("Close");
            btn_vacuum = tb.tool_button("Vacuum");
            btn_refresh = tb.tool_button("Refresh");
            btn_theme = tb.tool_button(theme_label);
            tb.spacer(8.0, 1.0);
            path_input = tb.text_input_styled(&state.path_draft, "db path…", path_style);
        });

        col.row(io_row, |row| {
            row.text("Import/Export");
            io_path = row.text_input_styled(&state.io_path, "csv/sql path…", io_path_style);
            btn_import_csv = row.tool_button("Import CSV");
            btn_export_csv = row.tool_button("Export CSV");
            btn_import_sql = row.tool_button("Import SQL");
            btn_export_sql = row.tool_button("Export SQL");
        });

        tabs = col.tabs(tabs_style, |tb| {
            tb.page(MainTab::Structure.title(), |p| {
                p.splitter(
                    split_style,
                    |left| {
                        tree = left.tree(roots_for_tree.clone(), tree_style);
                    },
                    |right| {
                        right.column(right_col, |c| {
                            c.text("Object SQL / DDL");
                            ddl_area = c.text_area(&state.ddl_draft, "CREATE / ALTER…", ddl_style);
                            c.row(struct_row, |row| {
                                btn_run_ddl = row.tool_button("Run DDL");
                                btn_drop = row.tool_button("Drop selected");
                            });
                            c.text("Create table");
                            c.row(struct_row, |row| {
                                new_table_name = row.text_input_styled(
                                    &state.new_table_name,
                                    "name",
                                    name_style,
                                );
                                new_table_cols = row.text_input_styled(
                                    &state.new_table_cols,
                                    "columns DDL",
                                    cols_style,
                                );
                                btn_create_table = row.tool_button("Create");
                            });
                            if let Some(sel) = &state.selected {
                                c.text(format!("Selected: {:?} {}", sel.kind, sel.name));
                            } else {
                                c.text("Selected: (none)");
                            }
                        });
                    },
                );
            });

            tb.page(MainTab::Browse.title(), |p| {
                p.row(browse_row, |row| {
                    row.text("Table");
                    browse_select =
                        row.select(browse_opts_a.clone(), browse_sel, "table…", sel_style);
                    browse_filter =
                        row.text_input_styled(&state.browse_filter, "WHERE …", filter_style);
                    btn_browse_go = row.tool_button("Apply");
                    btn_add_row = row.tool_button("Add row");
                    btn_del_row = row.tool_button("Delete row");
                });
                p.row(browse_row, |row| {
                    row.text("Edit cell");
                    edit_col = row.text_input_styled(&state.edit_col, "column", edit_col_style);
                    edit_value =
                        row.text_input_styled(&state.edit_value, "value", edit_val_style);
                    btn_edit_cell = row.tool_button("Write cell");
                });
                browse_table = p.table(browse_cols.clone(), browse_rows.clone(), browse_table_style);
            });

            tb.page(MainTab::Pragmas.title(), |p| {
                p.text("Select a pragma row, type the new value, then Apply (Bool click toggles).");
                pragma_grid = p.property_grid(pragma_rows.clone(), pragma_style);
                p.row(browse_row, |row| {
                    row.text("Value");
                    pragma_value = row.text_input_styled(
                        &state.pragma_value_draft,
                        "e.g. WAL / ON / -2000",
                        filter_style,
                    );
                    btn_pragma_apply = row.tool_button("Apply selected");
                });
            });

            tb.page(MainTab::ExecuteSql.title(), |p| {
                sql_area = p.text_area(&state.sql_editor, "SQL…", sql_area_style);
                p.row(browse_row, |row| {
                    btn_run_sql = row.tool_button("Run SQL");
                });
                let tone = if state.error_msg.is_some() {
                    AlertTone::Error
                } else {
                    AlertTone::Info
                };
                let msg = state
                    .error_msg
                    .clone()
                    .unwrap_or_else(|| "Ready — Run SQL to see results.".into());
                sql_alert = p.alert(tone, msg);
                sql_table = p.table(sql_cols.clone(), sql_rows.clone(), sql_table_style);
            });

            tb.page(MainTab::Charts.title(), |p| {
                p.row(chart_row, |row| {
                    row.text("Source");
                    chart_select =
                        row.select(browse_opts_b.clone(), chart_sel, "table…", sel_style);
                    chart_col =
                        row.text_input_styled(&state.chart_column, "numeric col", chart_col_style);
                    btn_chart = row.tool_button("Plot");
                });
                chart = p.chart(chart_tiles.clone(), chart_style);
            });

            tb.page(MainTab::SqlLog.title(), |p| {
                log_list = p.list(log_items.clone(), list_style);
            });
        });
    });

    let _ = ui.set_tabs_selected(tabs, state.tab.index());
    let _ = ui.set_tree_expanded(tree, 1, true);
    let _ = ui.set_tree_expanded(tree, 2, true);
    if let Some(sel) = &state.selected {
        if let Some(idx) = state
            .schema
            .iter()
            .position(|o| o.name == sel.name && o.kind == sel.kind)
        {
            let _ = ui.set_tree_selected(tree, Some(10_000 + idx as u64));
        }
    }

    let mut status_style = ui.status_bar_style();
    status_style.x = 0.0;
    status_style.y = win_h - 28.0;
    status_style.width = win_w;
    status_style.height = 28.0;
    ui.status_bar(status_style, |bar| {
        let path = if state.path.is_empty() {
            "(no database)"
        } else {
            state.path.as_str()
        };
        status = bar.text(format!(
            "{} · {} · {}",
            path,
            if state.conn.is_some() {
                "sqlite"
            } else {
                "closed"
            },
            state.status_msg
        ));
    });

    let (modal_title, modal_body, modal_primary) = match state.confirm_kind {
        ConfirmKind::DropObject => (
            "Drop object",
            state
                .selected
                .as_ref()
                .map(|s| format!("Drop {:?} `{}`?", s.kind, s.name))
                .unwrap_or_else(|| "Drop selected object?".into()),
            "Drop",
        ),
        ConfirmKind::DeleteRow => (
            "Delete row",
            "Delete the selected browse row (by rowid)?".into(),
            "Delete",
        ),
        ConfirmKind::CloseDb => (
            "Close database",
            "Close the current database connection?".into(),
            "Close",
        ),
        ConfirmKind::None => ("Confirm", "Confirm action?".into(), "OK"),
    };
    let modal = ui.modal(
        modal_title,
        modal_body,
        modal_primary,
        Some("Cancel".into()),
        ModalStyle::from_theme(&theme),
    );
    if state.confirm_open {
        let _ = ui.set_modal_open(modal, true);
    }

    ui.sync(&mut scene);
    scene.mark_full_dirty(win_w, win_h);

    BuiltUi {
        ui,
        scene,
        btn_new,
        btn_demo,
        btn_open,
        btn_close,
        btn_vacuum,
        btn_refresh,
        btn_theme,
        path_input,
        tabs,
        tree,
        ddl_area,
        btn_run_ddl,
        btn_drop,
        new_table_name,
        new_table_cols,
        btn_create_table,
        browse_select,
        browse_filter,
        btn_browse_go,
        btn_add_row,
        btn_del_row,
        btn_edit_cell,
        edit_col,
        edit_value,
        browse_table,
        pragma_grid,
        pragma_value,
        btn_pragma_apply,
        sql_area,
        btn_run_sql,
        sql_table,
        sql_alert,
        chart_select,
        chart_col,
        btn_chart,
        chart,
        log_list,
        io_path,
        btn_import_csv,
        btn_export_csv,
        btn_export_sql,
        btn_import_sql,
        status,
        modal,
    }
}
