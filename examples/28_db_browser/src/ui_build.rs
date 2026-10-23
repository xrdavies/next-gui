//! Build DB4S-style chrome: menubar, main toolbar, 4 tabs, right docks.

use next_gui_layout::{Axis, CrossAlign};
use next_gui_scene::{Color, Scene};
use next_gui_text::Font;
use next_gui_ui::{
    AlertTone, ChartStyle, ChartTile, ColumnStyle, ListStyle, MenuItem, MenuStyle, ModalStyle,
    PanelStyle, PropertyRow, PropertyValue, RowStyle, SelectStyle, SplitterStyle, StatusBarStyle,
    TableColumn, TableStyle, TabsStyle, TextAreaStyle, TextInputStyle, Theme, ThemeMode,
    ToolbarStyle, TreeNode, TreeStyle, TrendStyle, Ui, WidgetId,
};

use crate::db::ObjectKind;
use crate::state::{AppState, ConfirmKind, DockKind, MainTab};

/// Menu action ids (File / Edit / View / Tools / Help).
pub mod mid {
    pub const FILE_NEW: u64 = 1;
    pub const FILE_NEW_MEM: u64 = 2;
    pub const FILE_OPEN: u64 = 3;
    pub const FILE_OPEN_RO: u64 = 4;
    pub const FILE_ATTACH: u64 = 5;
    pub const FILE_CLOSE: u64 = 6;
    pub const FILE_SAVE_AS: u64 = 7;
    pub const FILE_WRITE: u64 = 8;
    pub const FILE_REVERT: u64 = 9;
    pub const FILE_VACUUM: u64 = 10;
    pub const FILE_IMPORT_CSV: u64 = 11;
    pub const FILE_IMPORT_SQL: u64 = 12;
    pub const FILE_IMPORT_CLIP: u64 = 13;
    pub const FILE_EXPORT_CSV: u64 = 14;
    pub const FILE_EXPORT_SQL: u64 = 15;
    pub const FILE_EXPORT_JSON: u64 = 16;
    pub const FILE_PROJECT_OPEN: u64 = 17;
    pub const FILE_PROJECT_SAVE: u64 = 18;
    pub const FILE_EXIT: u64 = 19;

    pub const EDIT_CREATE_TABLE: u64 = 30;
    pub const EDIT_CREATE_INDEX: u64 = 31;
    pub const EDIT_MODIFY: u64 = 32;
    pub const EDIT_DELETE: u64 = 33;
    pub const EDIT_PREFS: u64 = 34;

    pub const VIEW_DOCK_EDIT: u64 = 40;
    pub const VIEW_DOCK_SCHEMA: u64 = 41;
    pub const VIEW_DOCK_LOG: u64 = 42;
    pub const VIEW_DOCK_PLOT: u64 = 43;
    pub const VIEW_THEME: u64 = 44;

    pub const TOOLS_ENCRYPT: u64 = 50;
    pub const TOOLS_EXT: u64 = 51;
    pub const TOOLS_INTEGRITY: u64 = 52;
    pub const TOOLS_QUICK: u64 = 53;
    pub const TOOLS_FK: u64 = 54;
    pub const TOOLS_OPTIMIZE: u64 = 55;
    pub const TOOLS_ROWCOUNTS: u64 = 56;

    pub const HELP_ABOUT: u64 = 60;
    pub const HELP_WIKI: u64 = 61;
}

#[allow(dead_code)]
pub struct BuiltUi {
    pub ui: Ui,
    pub scene: Scene,
    pub menu_file: WidgetId,
    pub menu_edit: WidgetId,
    pub menu_view: WidgetId,
    pub menu_tools: WidgetId,
    pub menu_help: WidgetId,
    pub btn_new: WidgetId,
    pub btn_open: WidgetId,
    pub btn_write: WidgetId,
    pub btn_revert: WidgetId,
    pub btn_theme: WidgetId,
    pub tabs: WidgetId,
    pub tree: WidgetId,
    pub ddl_area: WidgetId,
    pub btn_struct_new_table: WidgetId,
    pub btn_struct_new_index: WidgetId,
    pub btn_struct_modify: WidgetId,
    pub btn_struct_delete: WidgetId,
    pub btn_struct_refresh: WidgetId,
    pub btn_run_ddl: WidgetId,
    pub new_table_name: WidgetId,
    pub new_table_cols: WidgetId,
    pub new_index_name: WidgetId,
    pub new_index_on: WidgetId,
    pub browse_select: WidgetId,
    pub browse_filter: WidgetId,
    pub btn_browse_go: WidgetId,
    pub btn_add_row: WidgetId,
    pub btn_del_row: WidgetId,
    pub browse_table: WidgetId,
    pub pragma_grid: WidgetId,
    pub pragma_value: WidgetId,
    pub btn_pragma_apply: WidgetId,
    pub sql_area: WidgetId,
    pub btn_sql_run: WidgetId,
    pub btn_sql_new_tab: WidgetId,
    pub btn_sql_open: WidgetId,
    pub btn_sql_save: WidgetId,
    pub sql_table: WidgetId,
    pub sql_alert: WidgetId,
    pub dock_tabs: WidgetId,
    pub edit_col: WidgetId,
    pub edit_value: WidgetId,
    pub btn_edit_cell: WidgetId,
    pub schema_list: WidgetId,
    pub log_list: WidgetId,
    pub chart_select: WidgetId,
    pub chart_col: WidgetId,
    pub btn_chart: WidgetId,
    pub chart: WidgetId,
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
    let w = (720.0 / n as f32).clamp(72.0, 200.0);
    cols.iter()
        .map(|c| {
            let title = if state_order_mark(c) {
                // placeholder — caller may decorate
                c.clone()
            } else {
                c.clone()
            };
            TableColumn::new(title, w)
        })
        .collect()
}

fn state_order_mark(_: &str) -> bool {
    false
}

fn decorate_browse_cols(state: &AppState, cols: &[String]) -> Vec<TableColumn> {
    let n = cols.len().max(1);
    let w = (720.0 / n as f32).clamp(72.0, 200.0);
    if cols.is_empty() {
        return vec![TableColumn::new("—", 120.0)];
    }
    cols.iter()
        .map(|c| {
            let title = if state.browse_order_col.as_deref() == Some(c.as_str()) {
                if state.browse_order_desc {
                    format!("{c} ▼")
                } else {
                    format!("{c} ▲")
                }
            } else {
                c.clone()
            };
            TableColumn::new(title, w)
        })
        .collect()
}

pub fn build_ui(font: Font, theme: Theme, width: u32, height: u32, state: &AppState) -> BuiltUi {
    let mut scene = Scene::new();
    let mut ui = Ui::with_font(font);
    ui.set_theme(theme);
    ui.set_viewport(width as f32, height as f32);

    let mut menu_file = WidgetId(0);
    let mut menu_edit = WidgetId(0);
    let mut menu_view = WidgetId(0);
    let mut menu_tools = WidgetId(0);
    let mut menu_help = WidgetId(0);
    let mut btn_new = WidgetId(0);
    let mut btn_open = WidgetId(0);
    let mut btn_write = WidgetId(0);
    let mut btn_revert = WidgetId(0);
    let mut btn_theme = WidgetId(0);
    let mut tabs = WidgetId(0);
    let mut tree = WidgetId(0);
    let mut ddl_area = WidgetId(0);
    let mut btn_struct_new_table = WidgetId(0);
    let mut btn_struct_new_index = WidgetId(0);
    let mut btn_struct_modify = WidgetId(0);
    let mut btn_struct_delete = WidgetId(0);
    let mut btn_struct_refresh = WidgetId(0);
    let mut btn_run_ddl = WidgetId(0);
    let mut new_table_name = WidgetId(0);
    let mut new_table_cols = WidgetId(0);
    let mut new_index_name = WidgetId(0);
    let mut new_index_on = WidgetId(0);
    let mut browse_select = WidgetId(0);
    let mut browse_filter = WidgetId(0);
    let mut btn_browse_go = WidgetId(0);
    let mut btn_add_row = WidgetId(0);
    let mut btn_del_row = WidgetId(0);
    let mut browse_table = WidgetId(0);
    let mut pragma_grid = WidgetId(0);
    let mut pragma_value = WidgetId(0);
    let mut btn_pragma_apply = WidgetId(0);
    let mut sql_area = WidgetId(0);
    let mut btn_sql_run = WidgetId(0);
    let mut btn_sql_new_tab = WidgetId(0);
    let mut btn_sql_open = WidgetId(0);
    let mut btn_sql_save = WidgetId(0);
    let mut sql_table = WidgetId(0);
    let mut sql_alert = WidgetId(0);
    let mut dock_tabs = WidgetId(0);
    let mut edit_col = WidgetId(0);
    let mut edit_value = WidgetId(0);
    let mut btn_edit_cell = WidgetId(0);
    let mut schema_list = WidgetId(0);
    let mut log_list = WidgetId(0);
    let mut chart_select = WidgetId(0);
    let mut chart_col = WidgetId(0);
    let mut btn_chart = WidgetId(0);
    let mut chart = WidgetId(0);
    let mut status = WidgetId(0);

    let win_w = width as f32;
    let win_h = height as f32;
    const STATUS_H: f32 = 40.0;
    let shell_w = (win_w - 12.0).max(1100.0);
    let shell_h = (win_h - STATUS_H - 4.0).max(640.0);
    // menubar 36 + main tb 40 + pad/gaps ≈ 100
    let content_h = (shell_h - 100.0).max(420.0);
    let docks_on = state.any_dock_visible();
    let main_frac = if docks_on { 0.72 } else { 1.0 };
    let main_w = (shell_w - 20.0) * main_frac;
    let dock_w = (shell_w - 20.0) * (1.0 - main_frac) - 8.0;
    let tab_page_h = (content_h - 36.0).max(360.0);

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
        .map(|r| decorate_browse_cols(state, &r.columns))
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
        state.sql_log.iter().rev().take(100).cloned().collect()
    };
    let schema_items: Vec<String> = if state.schema.is_empty() {
        vec!["(no objects)".into()]
    } else {
        state
            .schema
            .iter()
            .map(|o| format!("{:?} · {}", o.kind, o.name))
            .collect()
    };
    let chart_points = if state.chart_points.is_empty() {
        vec![0.0]
    } else {
        state.chart_points.clone()
    };
    let n_pts = state.chart_points.len();
    let chart_tiles = vec![ChartTile::trend(
        format!(
            "{} · {} ({} samples)",
            state.chart_table.as_deref().unwrap_or("—"),
            state.chart_column,
            n_pts
        ),
        chart_points,
        TrendStyle::from_theme(&theme),
    )];

    let menu_style = MenuStyle::from_theme(&theme);
    let check = |on: bool| if on { "✓ " } else { "  " };

    let mut col_style = ui.column_style();
    col_style.x = 6.0;
    col_style.y = 2.0;
    col_style.width = shell_w;
    col_style.height = shell_h;
    col_style.padding = 4.0;
    col_style.gap = 4.0;
    col_style.cross_align = CrossAlign::Start;

    let mut menu_tb = ToolbarStyle::from_theme(&theme);
    menu_tb.width = shell_w - 8.0;
    menu_tb.height = 34.0;
    menu_tb.padding = 2.0;
    menu_tb.gap = 4.0;

    let mut main_tb = ToolbarStyle::from_theme(&theme);
    main_tb.width = shell_w - 8.0;
    main_tb.height = 38.0;
    main_tb.padding = 4.0;

    let mut body_split = SplitterStyle::from_theme(&theme);
    body_split.width = shell_w - 8.0;
    body_split.height = content_h;
    body_split.axis = Axis::Vertical;
    body_split.ratio = if docks_on { 0.72 } else { 0.98 };

    let mut tabs_style = TabsStyle::from_theme(&theme);
    tabs_style.width = main_w - 4.0;
    tabs_style.height = content_h - 4.0;

    let mut struct_tb = ToolbarStyle::from_theme(&theme);
    struct_tb.width = main_w - 24.0;
    struct_tb.height = 34.0;

    let mut struct_split = SplitterStyle::from_theme(&theme);
    struct_split.width = main_w - 24.0;
    struct_split.height = (tab_page_h - 48.0).max(280.0);
    struct_split.axis = Axis::Vertical;
    struct_split.ratio = 0.34;

    let tree_w = (main_w - 24.0) * 0.34 - 8.0;
    let right_w = (main_w - 24.0) * 0.66 - 8.0;
    let mut tree_style = TreeStyle::from_theme(&theme);
    tree_style.width = tree_w;
    tree_style.height = (tab_page_h - 56.0).max(240.0);

    let right_col = ColumnStyle {
        width: right_w,
        height: (tab_page_h - 56.0).max(240.0),
        padding: 4.0,
        gap: 4.0,
        cross_align: CrossAlign::Start,
        ..ColumnStyle::from_theme(&theme)
    };
    let mut ddl_style = TextAreaStyle::from_theme(&theme);
    ddl_style.width = right_w - 12.0;
    ddl_style.height = 140.0;

    let mut name_style = TextInputStyle::from_theme(&theme);
    name_style.width = 120.0;
    let mut cols_style = TextInputStyle::from_theme(&theme);
    cols_style.width = 280.0;
    let mut idx_style = TextInputStyle::from_theme(&theme);
    idx_style.width = 160.0;

    let mut tool_row = RowStyle::from_theme(&theme);
    tool_row.width = main_w - 24.0;
    tool_row.height = 32.0;
    tool_row.gap = 6.0;
    tool_row.padding = 0.0;
    tool_row.cross_align = CrossAlign::Center;

    let mut sel_style = SelectStyle::from_theme(&theme);
    sel_style.width = 160.0;
    let mut filter_style = TextInputStyle::from_theme(&theme);
    filter_style.width = 200.0;

    let mut browse_table_style = TableStyle::from_theme(&theme);
    browse_table_style.width = main_w - 24.0;
    browse_table_style.height = (tab_page_h - 80.0).max(200.0);

    let mut pragma_style = ui.property_grid_style();
    pragma_style.width = main_w - 24.0;
    let pragma_rows_h = (state.pragmas.len() as f32 + 1.0) * pragma_style.row_height + 24.0;
    pragma_style.height = pragma_rows_h.clamp(140.0, (tab_page_h - 100.0).max(140.0));
    pragma_style.key_width = 180.0;

    let mut sql_tb = ToolbarStyle::from_theme(&theme);
    sql_tb.width = main_w - 24.0;
    sql_tb.height = 34.0;
    let mut sql_area_style = TextAreaStyle::from_theme(&theme);
    sql_area_style.width = main_w - 24.0;
    sql_area_style.height = 130.0;
    let mut sql_table_style = TableStyle::from_theme(&theme);
    sql_table_style.width = main_w - 24.0;
    sql_table_style.height = (tab_page_h - 230.0).max(140.0);

    let mut dock_tabs_style = TabsStyle::from_theme(&theme);
    dock_tabs_style.width = dock_w.max(220.0);
    dock_tabs_style.height = content_h - 4.0;

    let mut dock_col = ColumnStyle::from_theme(&theme);
    dock_col.width = dock_w.max(200.0) - 16.0;
    dock_col.height = tab_page_h - 20.0;
    dock_col.padding = 4.0;
    dock_col.gap = 4.0;
    dock_col.cross_align = CrossAlign::Start;

    let mut edit_col_style = TextInputStyle::from_theme(&theme);
    edit_col_style.width = (dock_w - 40.0).max(120.0);
    let mut edit_val_style = TextInputStyle::from_theme(&theme);
    edit_val_style.width = (dock_w - 40.0).max(120.0);
    edit_val_style.height = 56.0;

    let mut list_style = ListStyle::from_theme(&theme);
    list_style.width = (dock_w - 28.0).max(180.0);
    list_style.height = (tab_page_h - 48.0).max(180.0);

    let mut chart_col_style = TextInputStyle::from_theme(&theme);
    chart_col_style.width = 100.0;
    let mut chart_style = ChartStyle::from_theme(&theme);
    chart_style.width = (dock_w - 28.0).max(180.0);
    chart_style.height = (tab_page_h - 100.0).max(160.0);
    chart_style.columns = 1;
    chart_style.padding = 6.0;

    let mut dock_row = RowStyle::from_theme(&theme);
    dock_row.width = (dock_w - 28.0).max(180.0);
    dock_row.height = 30.0;
    dock_row.gap = 4.0;
    dock_row.padding = 0.0;

    let browse_opts_a = browse_opts.clone();
    let browse_opts_b = browse_opts;
    let roots_for_tree = roots;
    let visible_docks = state.visible_docks();
    let sql_editor = state.sql_editor().to_string();
    let sql_tab_label = format!("SQL {}", state.sql_tab_idx + 1);

    ui.column(col_style, |col| {
        // —— Menubar ——
        col.toolbar(menu_tb, |tb| {
            menu_file = tb.menu(
                "File",
                vec![
                    MenuItem::new(mid::FILE_NEW, "New Database…"),
                    MenuItem::new(mid::FILE_NEW_MEM, "New In-Memory Database"),
                    MenuItem::new(mid::FILE_OPEN, "Open Database…"),
                    MenuItem::new(mid::FILE_OPEN_RO, "Open Database Read Only…"),
                    MenuItem::disabled(mid::FILE_ATTACH, "Attach Database…"),
                    MenuItem::new(mid::FILE_CLOSE, "Close Database"),
                    MenuItem::new(mid::FILE_SAVE_AS, "Save Database As…"),
                    MenuItem::new(mid::FILE_WRITE, "Write Changes"),
                    MenuItem::new(mid::FILE_REVERT, "Revert Changes"),
                    MenuItem::new(mid::FILE_VACUUM, "Compact Database"),
                    MenuItem::new(mid::FILE_IMPORT_CSV, "Import Table from CSV…"),
                    MenuItem::new(mid::FILE_IMPORT_SQL, "Import Database from SQL…"),
                    MenuItem::disabled(mid::FILE_IMPORT_CLIP, "Import CSV from Clipboard…"),
                    MenuItem::new(mid::FILE_EXPORT_CSV, "Export Table as CSV…"),
                    MenuItem::new(mid::FILE_EXPORT_SQL, "Export Database to SQL…"),
                    MenuItem::disabled(mid::FILE_EXPORT_JSON, "Export Table(s) to JSON…"),
                    MenuItem::disabled(mid::FILE_PROJECT_OPEN, "Open Project…"),
                    MenuItem::disabled(mid::FILE_PROJECT_SAVE, "Save Project"),
                    MenuItem::new(mid::FILE_EXIT, "Exit"),
                ],
                menu_style,
            );
            menu_edit = tb.menu(
                "Edit",
                vec![
                    MenuItem::new(mid::EDIT_CREATE_TABLE, "Create Table…"),
                    MenuItem::new(mid::EDIT_CREATE_INDEX, "Create Index…"),
                    MenuItem::disabled(mid::EDIT_MODIFY, "Modify Object…"),
                    MenuItem::new(mid::EDIT_DELETE, "Delete Object…"),
                    MenuItem::disabled(mid::EDIT_PREFS, "Preferences…"),
                ],
                menu_style,
            );
            menu_view = tb.menu(
                "View",
                vec![
                    MenuItem::new(
                        mid::VIEW_DOCK_EDIT,
                        format!("{}Edit Database Cell", check(state.dock_edit)),
                    ),
                    MenuItem::new(
                        mid::VIEW_DOCK_SCHEMA,
                        format!("{}DB Schema", check(state.dock_schema)),
                    ),
                    MenuItem::new(
                        mid::VIEW_DOCK_LOG,
                        format!("{}SQL Log", check(state.dock_log)),
                    ),
                    MenuItem::new(
                        mid::VIEW_DOCK_PLOT,
                        format!("{}Plot", check(state.dock_plot)),
                    ),
                    MenuItem::new(mid::VIEW_THEME, "Toggle Theme Light/Dark"),
                ],
                menu_style,
            );
            menu_tools = tb.menu(
                "Tools",
                vec![
                    MenuItem::disabled(mid::TOOLS_ENCRYPT, "Set Encryption…"),
                    MenuItem::disabled(mid::TOOLS_EXT, "Load Extension…"),
                    MenuItem::new(mid::TOOLS_INTEGRITY, "Integrity Check"),
                    MenuItem::new(mid::TOOLS_QUICK, "Quick Check"),
                    MenuItem::new(mid::TOOLS_FK, "Foreign Key Check"),
                    MenuItem::new(mid::TOOLS_OPTIMIZE, "Optimize"),
                    MenuItem::new(mid::TOOLS_ROWCOUNTS, "Row Counts"),
                ],
                menu_style,
            );
            menu_help = tb.menu(
                "Help",
                vec![
                    MenuItem::new(mid::HELP_ABOUT, "About"),
                    MenuItem::new(mid::HELP_WIKI, "Wiki (stub)"),
                ],
                menu_style,
            );
        });

        // —— Main DB toolbar ——
        col.toolbar(main_tb, |tb| {
            btn_new = tb.tool_button("New");
            btn_open = tb.tool_button("Open");
            btn_write = tb.tool_button("Write Changes");
            btn_revert = tb.tool_button("Revert");
            btn_theme = tb.tool_button("Theme");
            tb.spacer(12.0, 1.0);
            let dirty = if state.dirty { "● modified" } else { "○ clean" };
            let ro = if state.read_only { " · read-only" } else { "" };
            tb.text_styled(
                format!("{}{ro} · {dirty}", if state.path.is_empty() { "(no database)" } else { &state.path }),
                theme.colors.text_secondary,
                12.0,
            );
        });

        // —— Main + docks ——
        col.splitter(
            body_split,
            |left| {
                tabs = left.tabs(tabs_style, |tb| {
                    // Structure
                    tb.page(MainTab::Structure.title(), |p| {
                        p.toolbar(struct_tb, |tb| {
                            btn_struct_new_table = tb.tool_button("New Table");
                            btn_struct_new_index = tb.tool_button("New Index");
                            btn_struct_modify = tb.tool_button("Modify");
                            btn_struct_delete = tb.tool_button("Delete");
                            btn_struct_refresh = tb.tool_button("Refresh");
                        });
                        p.splitter(
                            struct_split,
                            |l| {
                                tree = l.tree(roots_for_tree.clone(), tree_style);
                            },
                            |r| {
                                r.column(right_col, |c| {
                                    c.text("Object SQL / DDL");
                                    ddl_area =
                                        c.text_area(&state.ddl_draft, "CREATE / ALTER…", ddl_style);
                                    c.row(tool_row, |row| {
                                        btn_run_ddl = row.tool_button("Run DDL");
                                    });
                                    c.text("Quick create table");
                                    c.row(tool_row, |row| {
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
                                    });
                                    c.text("Quick create index");
                                    c.row(tool_row, |row| {
                                        new_index_name = row.text_input_styled(
                                            &state.new_index_name,
                                            "index name",
                                            name_style,
                                        );
                                        new_index_on = row.text_input_styled(
                                            &state.new_index_on,
                                            "table(cols)",
                                            idx_style,
                                        );
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

                    // Browse
                    tb.page(MainTab::Browse.title(), |p| {
                        p.row(tool_row, |row| {
                            row.text("Table");
                            browse_select =
                                row.select(browse_opts_a.clone(), browse_sel, "table…", sel_style);
                            browse_filter = row.text_input_styled(
                                &state.browse_filter,
                                "WHERE …",
                                filter_style,
                            );
                            btn_browse_go = row.tool_button("Apply");
                            btn_add_row = row.tool_button("New Record");
                            btn_del_row = row.tool_button("Delete Record");
                        });
                        browse_table =
                            p.table(browse_cols.clone(), browse_rows.clone(), browse_table_style);
                        p.text_styled(
                            "Tip: click a column header to sort · edit via Edit Database Cell dock",
                            theme.colors.text_secondary,
                            12.0,
                        );
                    });

                    // Pragmas
                    tb.page(MainTab::Pragmas.title(), |p| {
                        p.text("Database preferences (PRAGMA). Select a row, set Value, Apply.");
                        pragma_grid = p.property_grid(pragma_rows.clone(), pragma_style);
                        p.row(tool_row, |row| {
                            row.text("Value");
                            pragma_value = row.text_input_styled(
                                &state.pragma_value_draft,
                                "e.g. WAL / ON / -2000",
                                filter_style,
                            );
                            btn_pragma_apply = row.tool_button("Save Pragmas");
                        });
                    });

                    // Execute SQL
                    tb.page(MainTab::ExecuteSql.title(), |p| {
                        p.toolbar(sql_tb, |tb| {
                            btn_sql_new_tab = tb.tool_button("New Tab");
                            btn_sql_open = tb.tool_button("Open SQL");
                            btn_sql_save = tb.tool_button("Save SQL");
                            btn_sql_run = tb.tool_button("Execute");
                            tb.spacer(8.0, 1.0);
                            tb.text_styled(&sql_tab_label, theme.colors.text_secondary, 12.0);
                        });
                        sql_area = p.text_area(&sql_editor, "SQL…", sql_area_style);
                        let tone = if state.error_msg.is_some() {
                            AlertTone::Error
                        } else {
                            AlertTone::Info
                        };
                        let msg = state
                            .error_msg
                            .clone()
                            .unwrap_or_else(|| state.sql_message.clone());
                        sql_alert = p.alert(tone, msg);
                        sql_table = p.table(sql_cols.clone(), sql_rows.clone(), sql_table_style);
                    });
                });
            },
            |right| {
                if !docks_on {
                    right.text_styled("(docks hidden — View menu)", theme.colors.text_secondary, 12.0);
                    return;
                }
                dock_tabs = right.tabs(dock_tabs_style, |tb| {
                    for dock in &visible_docks {
                        match *dock {
                            DockKind::EditCell => {
                                tb.page(DockKind::EditCell.title(), |p| {
                                    p.column(dock_col, |c| {
                                        c.text("Column");
                                        edit_col = c.text_input_styled(
                                            &state.edit_col,
                                            "column",
                                            edit_col_style,
                                        );
                                        c.text("Value");
                                        edit_value = c.text_input_styled(
                                            &state.edit_value,
                                            "value",
                                            edit_val_style,
                                        );
                                        btn_edit_cell = c.tool_button("Apply");
                                        if let Some(row) = state.browse_sel_row {
                                            c.text(format!("Browse row #{row}"));
                                        } else {
                                            c.text("Select a browse row");
                                        }
                                    });
                                });
                            }
                            DockKind::Schema => {
                                tb.page(DockKind::Schema.title(), |p| {
                                    schema_list = p.list(schema_items.clone(), list_style);
                                });
                            }
                            DockKind::Log => {
                                tb.page(DockKind::Log.title(), |p| {
                                    log_list = p.list(log_items.clone(), list_style);
                                });
                            }
                            DockKind::Plot => {
                                tb.page(DockKind::Plot.title(), |p| {
                                    p.column(dock_col, |c| {
                                        c.row(dock_row, |row| {
                                            chart_select = row.select(
                                                browse_opts_b.clone(),
                                                chart_sel,
                                                "table…",
                                                sel_style,
                                            );
                                            chart_col = row.text_input_styled(
                                                &state.chart_column,
                                                "col",
                                                chart_col_style,
                                            );
                                            btn_chart = row.tool_button("Plot");
                                        });
                                        chart = c.chart(chart_tiles.clone(), chart_style);
                                    });
                                });
                            }
                        }
                    }
                });
            },
        );
    });

    // Status bar
    let mut status_style = StatusBarStyle::from_theme(&theme);
    status_style.x = 0.0;
    status_style.y = win_h - STATUS_H;
    status_style.width = win_w;
    status_style.height = STATUS_H;
    status_style.padding = 6.0;
    status_style.gap = 8.0;
    status_style.cross_align = CrossAlign::Start;
    let path = if state.path.is_empty() {
        "(no database)".to_string()
    } else {
        state.path.clone()
    };
    let flags = format!(
        "{}{}{}",
        if state.read_only { "RO" } else { "RW" },
        if state.dirty { " · modified" } else { "" },
        " · UTF-8 · not encrypted"
    );
    let status_msg = state.status_msg.clone();
    let status_fg = theme.colors.text_secondary;
    ui.status_bar(status_style, |bar| {
        status = bar.text_styled(format!("{path} · {flags} · {status_msg}"), status_fg, 12.0);
    });

    let _ = ui.set_tabs_selected(tabs, state.tab.index());
    if docks_on && dock_tabs.0 != 0 {
        let focus_idx = visible_docks
            .iter()
            .position(|d| *d == state.dock_focus)
            .unwrap_or(0);
        let _ = ui.set_tabs_selected(dock_tabs, focus_idx);
    }
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
    if let Some(row) = state.browse_sel_row {
        let _ = ui.set_table_selected_row(browse_table, Some(row));
    }

    let (modal_title, modal_body, modal_primary) = match state.confirm_kind {
        ConfirmKind::DropObject => (
            "Delete Object",
            state
                .selected
                .as_ref()
                .map(|s| format!("Delete {:?} `{}`?", s.kind, s.name))
                .unwrap_or_else(|| "Delete selected object?".into()),
            "Delete",
        ),
        ConfirmKind::DeleteRow => (
            "Delete Record",
            "Delete the selected browse row (by rowid)?".into(),
            "Delete",
        ),
        ConfirmKind::CloseDb => (
            "Close Database",
            if state.dirty {
                "There are unsaved changes. Close anyway (revert)?"
            } else {
                "Close the current database connection?"
            }
            .into(),
            "Close",
        ),
        ConfirmKind::Revert => (
            "Revert Changes",
            "Discard all unwritten changes and reload?".into(),
            "Revert",
        ),
        ConfirmKind::ExitDirty => (
            "Exit",
            "There are unsaved changes. Exit anyway?".into(),
            "Exit",
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

    // silence unused PanelStyle import path if any
    let _ = PanelStyle::from_theme(&theme);

    ui.sync(&mut scene);
    scene.mark_full_dirty(win_w, win_h);

    BuiltUi {
        ui,
        scene,
        menu_file,
        menu_edit,
        menu_view,
        menu_tools,
        menu_help,
        btn_new,
        btn_open,
        btn_write,
        btn_revert,
        btn_theme,
        tabs,
        tree,
        ddl_area,
        btn_struct_new_table,
        btn_struct_new_index,
        btn_struct_modify,
        btn_struct_delete,
        btn_struct_refresh,
        btn_run_ddl,
        new_table_name,
        new_table_cols,
        new_index_name,
        new_index_on,
        browse_select,
        browse_filter,
        btn_browse_go,
        btn_add_row,
        btn_del_row,
        browse_table,
        pragma_grid,
        pragma_value,
        btn_pragma_apply,
        sql_area,
        btn_sql_run,
        btn_sql_new_tab,
        btn_sql_open,
        btn_sql_save,
        sql_table,
        sql_alert,
        dock_tabs,
        edit_col,
        edit_value,
        btn_edit_cell,
        schema_list,
        log_list,
        chart_select,
        chart_col,
        btn_chart,
        chart,
        status,
        modal,
    }
}
