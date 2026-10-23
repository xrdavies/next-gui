//! Build DB4S-style chrome + Phase 2 overlays / browse / pragmas / SQL.

use next_gui_layout::{Axis, CrossAlign};
use next_gui_scene::{Color, Scene};
use next_gui_text::Font;
use next_gui_ui::{
    AlertTone, ChartStyle, ChartTile, ColumnStyle, ListStyle, MenuItem, MenuStyle, ModalStyle,
    PanelStyle, RowStyle, SelectStyle, SplitterStyle, StatusBarStyle, SwitchStyle, TableColumn,
    TableStyle, TabsStyle, TextAreaStyle, TextInputStyle, Theme, ThemeMode, ToolbarStyle, TreeNode,
    TreeStyle, TrendStyle, Ui, WidgetId,
};

use crate::db::ObjectKind;
use crate::state::{AppState, ConfirmKind, DockKind, MainTab, OverlayKind};

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
    pub const FILE_RECENT_BASE: u64 = 100; // 100..104

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
    pub browse_tabs: WidgetId,
    pub btn_browse_new_tab: WidgetId,
    pub browse_select: WidgetId,
    pub browse_filter: WidgetId,
    pub btn_browse_go: WidgetId,
    pub btn_add_row: WidgetId,
    pub btn_del_row: WidgetId,
    pub browse_table: WidgetId,
    pub browse_col_filters: Vec<WidgetId>,
    pub browse_tab_btns: Vec<WidgetId>,
    pub sql_tab_btns: Vec<WidgetId>,
    pub pragma_inputs: Vec<WidgetId>,
    pub pragma_switches: Vec<WidgetId>,
    pub pragma_selects: Vec<WidgetId>,
    pub btn_pragma_apply: WidgetId,
    pub sql_tabs: WidgetId,
    pub sql_area: WidgetId,
    pub btn_sql_run: WidgetId,
    pub btn_sql_new_tab: WidgetId,
    pub btn_sql_open: WidgetId,
    pub btn_sql_save: WidgetId,
    pub btn_sql_find: WidgetId,
    pub btn_sql_stop: WidgetId,
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
    // Overlay widgets
    pub ov_name: WidgetId,
    pub ov_list: WidgetId,
    pub ov_col_name: WidgetId,
    pub ov_col_type: WidgetId,
    pub ov_pk: WidgetId,
    pub ov_nn: WidgetId,
    pub ov_unique: WidgetId,
    pub ov_default: WidgetId,
    pub ov_btn_add: WidgetId,
    pub ov_btn_del: WidgetId,
    pub ov_btn_up: WidgetId,
    pub ov_btn_down: WidgetId,
    pub ov_btn_ok: WidgetId,
    pub ov_btn_cancel: WidgetId,
    pub ov_idx_table: WidgetId,
    pub ov_idx_cols: WidgetId,
    pub ov_idx_unique: WidgetId,
    pub ov_csv_table: WidgetId,
    pub ov_csv_header: WidgetId,
    pub ov_csv_delim: WidgetId,
    pub ov_csv_quote: WidgetId,
    pub ov_csv_paste: WidgetId,
    pub ov_attach_path: WidgetId,
    pub ov_attach_schema: WidgetId,
    pub ov_find: WidgetId,
    pub ov_replace: WidgetId,
    pub ov_btn_find: WidgetId,
    pub ov_btn_repl: WidgetId,
    pub ov_btn_repl_all: WidgetId,
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

fn decorate_browse_cols(state: &AppState, cols: &[String]) -> Vec<TableColumn> {
    let n = cols.len().max(1);
    let w = (720.0 / n as f32).clamp(72.0, 200.0);
    if cols.is_empty() {
        return vec![TableColumn::new("—", 120.0)];
    }
    let order = state.browse().order_col.as_deref();
    let desc = state.browse().order_desc;
    cols.iter()
        .map(|c| {
            let title = if order == Some(c.as_str()) {
                if desc {
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

fn result_columns(cols: &[String], fallback: &[&str]) -> Vec<TableColumn> {
    if cols.is_empty() {
        return fallback
            .iter()
            .map(|t| TableColumn::new(*t, 120.0))
            .collect();
    }
    let n = cols.len().max(1);
    let w = (720.0 / n as f32).clamp(72.0, 200.0);
    cols.iter().map(|c| TableColumn::new(c.clone(), w)).collect()
}

pub fn build_ui(font: Font, theme: Theme, width: u32, height: u32, state: &AppState) -> BuiltUi {
    let mut scene = Scene::new();
    let mut ui = Ui::with_font(font);
    ui.set_theme(theme);
    ui.set_viewport(width as f32, height as f32);

    let z = WidgetId(0);
    let mut menu_file = z;
    let mut menu_edit = z;
    let mut menu_view = z;
    let mut menu_tools = z;
    let mut menu_help = z;
    let mut btn_new = z;
    let mut btn_open = z;
    let mut btn_write = z;
    let mut btn_revert = z;
    let mut btn_theme = z;
    let mut tabs = z;
    let mut tree = z;
    let mut ddl_area = z;
    let mut btn_struct_new_table = z;
    let mut btn_struct_new_index = z;
    let mut btn_struct_modify = z;
    let mut btn_struct_delete = z;
    let mut btn_struct_refresh = z;
    let mut btn_run_ddl = z;
    let mut browse_tabs = z;
    let mut btn_browse_new_tab = z;
    let mut browse_select = z;
    let mut browse_filter = z;
    let mut btn_browse_go = z;
    let mut btn_add_row = z;
    let mut btn_del_row = z;
    let mut browse_table = z;
    let mut browse_col_filters: Vec<WidgetId> = Vec::new();
    let mut browse_tab_btns: Vec<WidgetId> = Vec::new();
    let mut sql_tab_btns: Vec<WidgetId> = Vec::new();
    let mut pragma_inputs: Vec<WidgetId> = Vec::new();
    let mut pragma_switches: Vec<WidgetId> = Vec::new();
    let mut pragma_selects: Vec<WidgetId> = Vec::new();
    let mut btn_pragma_apply = z;
    let mut sql_tabs_w = z;
    let mut sql_area = z;
    let mut btn_sql_run = z;
    let mut btn_sql_new_tab = z;
    let mut btn_sql_open = z;
    let mut btn_sql_save = z;
    let mut btn_sql_find = z;
    let mut btn_sql_stop = z;
    let mut sql_table = z;
    let mut sql_alert = z;
    let mut dock_tabs = z;
    let mut edit_col = z;
    let mut edit_value = z;
    let mut btn_edit_cell = z;
    let mut schema_list = z;
    let mut log_list = z;
    let mut chart_select = z;
    let mut chart_col = z;
    let mut btn_chart = z;
    let mut chart = z;
    let mut status = z;
    let mut ov_name = z;
    let mut ov_list = z;
    let mut ov_col_name = z;
    let mut ov_col_type = z;
    let mut ov_pk = z;
    let mut ov_nn = z;
    let mut ov_unique = z;
    let mut ov_default = z;
    let mut ov_btn_add = z;
    let mut ov_btn_del = z;
    let mut ov_btn_up = z;
    let mut ov_btn_down = z;
    let mut ov_btn_ok = z;
    let mut ov_btn_cancel = z;
    let mut ov_idx_table = z;
    let mut ov_idx_cols = z;
    let mut ov_idx_unique = z;
    let mut ov_csv_table = z;
    let mut ov_csv_header = z;
    let mut ov_csv_delim = z;
    let mut ov_csv_quote = z;
    let mut ov_csv_paste = z;
    let mut ov_attach_path = z;
    let mut ov_attach_schema = z;
    let mut ov_find = z;
    let mut ov_replace = z;
    let mut ov_btn_find = z;
    let mut ov_btn_repl = z;
    let mut ov_btn_repl_all = z;

    let win_w = width as f32;
    let win_h = height as f32;
    const STATUS_H: f32 = 40.0;
    let shell_w = (win_w - 12.0).max(1100.0);
    let shell_h = (win_h - STATUS_H - 4.0).max(640.0);
    let content_h = (shell_h - 100.0).max(420.0);
    let docks_on = state.any_dock_visible() && state.overlay.is_none();
    let main_frac = if docks_on { 0.72 } else { 1.0 };
    let main_w = (shell_w - 20.0) * main_frac;
    let dock_w = (shell_w - 20.0) * (1.0 - main_frac) - 8.0;
    let tab_page_h = (content_h - 36.0).max(360.0);

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

    // Recent menu items
    let mut file_items = vec![
        MenuItem::new(mid::FILE_NEW, "New Database…"),
        MenuItem::new(mid::FILE_NEW_MEM, "New In-Memory Database"),
        MenuItem::new(mid::FILE_OPEN, "Open Database…"),
        MenuItem::new(mid::FILE_OPEN_RO, "Open Database Read Only…"),
        MenuItem::new(mid::FILE_ATTACH, "Attach Database…"),
        MenuItem::new(mid::FILE_CLOSE, "Close Database"),
        MenuItem::new(mid::FILE_SAVE_AS, "Save Database As…"),
        MenuItem::new(mid::FILE_WRITE, "Write Changes"),
        MenuItem::new(mid::FILE_REVERT, "Revert Changes"),
        MenuItem::new(mid::FILE_VACUUM, "Compact Database"),
        MenuItem::new(mid::FILE_IMPORT_CSV, "Import Table from CSV…"),
        MenuItem::new(mid::FILE_IMPORT_SQL, "Import Database from SQL…"),
        MenuItem::new(mid::FILE_IMPORT_CLIP, "Import CSV from Clipboard…"),
        MenuItem::new(mid::FILE_EXPORT_CSV, "Export Table as CSV…"),
        MenuItem::new(mid::FILE_EXPORT_SQL, "Export Database to SQL…"),
        MenuItem::new(mid::FILE_EXPORT_JSON, "Export Table(s) to JSON…"),
    ];
    for (i, p) in state.recent.iter().take(5).enumerate() {
        let short = if p.len() > 48 {
            format!("…{}", &p[p.len() - 47..])
        } else {
            p.clone()
        };
        file_items.push(MenuItem::new(mid::FILE_RECENT_BASE + i as u64, format!("Recent: {short}")));
    }
    file_items.push(MenuItem::disabled(mid::FILE_PROJECT_OPEN, "Open Project…"));
    file_items.push(MenuItem::disabled(mid::FILE_PROJECT_SAVE, "Save Project"));
    file_items.push(MenuItem::new(mid::FILE_EXIT, "Exit"));

    ui.column(col_style, |col| {
        col.toolbar(menu_tb, |tb| {
            menu_file = tb.menu("File", file_items.clone(), menu_style);
            menu_edit = tb.menu(
                "Edit",
                vec![
                    MenuItem::new(mid::EDIT_CREATE_TABLE, "Create Table…"),
                    MenuItem::new(mid::EDIT_CREATE_INDEX, "Create Index…"),
                    MenuItem::new(mid::EDIT_MODIFY, "Modify Object…"),
                    MenuItem::new(mid::EDIT_DELETE, "Delete Object…"),
                    MenuItem::disabled(mid::EDIT_PREFS, "Preferences…"),
                ],
                menu_style,
            );
            menu_view = tb.menu(
                "View",
                vec![
                    MenuItem::new(mid::VIEW_DOCK_EDIT, format!("{}Edit Database Cell", check(state.dock_edit))),
                    MenuItem::new(mid::VIEW_DOCK_SCHEMA, format!("{}DB Schema", check(state.dock_schema))),
                    MenuItem::new(mid::VIEW_DOCK_LOG, format!("{}SQL Log", check(state.dock_log))),
                    MenuItem::new(mid::VIEW_DOCK_PLOT, format!("{}Plot", check(state.dock_plot))),
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

        col.toolbar(main_tb, |tb| {
            btn_new = tb.tool_button("New");
            btn_open = tb.tool_button("Open");
            btn_write = tb.tool_button("Write Changes");
            btn_revert = tb.tool_button("Revert");
            btn_theme = tb.tool_button("Theme");
            tb.spacer(12.0, 1.0);
            let dirty = if state.dirty { "● modified" } else { "○ clean" };
            let ro = if state.read_only { " · read-only" } else { "" };
            let busy = if state.sql_busy { " · busy" } else { "" };
            tb.text_styled(
                format!(
                    "{}{ro} · {dirty}{busy}",
                    if state.path.is_empty() {
                        "(no database)"
                    } else {
                        &state.path
                    }
                ),
                theme.colors.text_secondary,
                12.0,
            );
        });

        if let Some(kind) = state.overlay {
            // —— Overlay wizard panel ——
            let mut panel = PanelStyle::from_theme(&theme);
            panel.width = shell_w - 16.0;
            panel.height = content_h;
            let title = match kind {
                OverlayKind::EditTable => {
                    if state.wizard_table_original.is_some() {
                        "Modify Table"
                    } else {
                        "Create Table"
                    }
                }
                OverlayKind::EditIndex => {
                    if state.wizard_index_original.is_some() {
                        "Modify Index"
                    } else {
                        "Create Index"
                    }
                }
                OverlayKind::ImportCsv => "Import CSV",
                OverlayKind::PasteCsv => "Import CSV from Clipboard",
                OverlayKind::FindReplace => "Find / Replace",
                OverlayKind::Attach => "Attach Database",
            };
            let mut ov_col = ColumnStyle::from_theme(&theme);
            ov_col.width = shell_w - 40.0;
            ov_col.height = content_h - 40.0;
            ov_col.padding = 8.0;
            ov_col.gap = 6.0;
            ov_col.cross_align = CrossAlign::Start;

            let mut row = RowStyle::from_theme(&theme);
            row.width = shell_w - 60.0;
            row.height = 32.0;
            row.gap = 8.0;
            row.padding = 0.0;
            row.cross_align = CrossAlign::Center;

            let mut name_s = TextInputStyle::from_theme(&theme);
            name_s.width = 220.0;
            let mut wide_s = TextInputStyle::from_theme(&theme);
            wide_s.width = 360.0;
            let mut type_sel = SelectStyle::from_theme(&theme);
            type_sel.width = 140.0;
            let mut list_s = ListStyle::from_theme(&theme);
            list_s.width = 280.0;
            list_s.height = 180.0;
            let mut area_s = TextAreaStyle::from_theme(&theme);
            area_s.width = shell_w - 80.0;
            area_s.height = 160.0;
            let sw = SwitchStyle::from_theme(&theme);

            col.panel(title, panel, |p| {
                p.column(ov_col, |c| {
                    match kind {
                        OverlayKind::EditTable => {
                            c.row(row, |r| {
                                r.text("Table name");
                                ov_name = r.text_input_styled(&state.wizard_table_name, "name", name_s);
                            });
                            let items: Vec<String> = state
                                .wizard_cols
                                .iter()
                                .enumerate()
                                .map(|(i, col)| {
                                    format!(
                                        "{}. {} {}{}",
                                        i + 1,
                                        col.name,
                                        col.type_name,
                                        if col.pk { " PK" } else { "" }
                                    )
                                })
                                .collect();
                            c.text("Columns");
                            ov_list = c.list(items, list_s);
                            c.row(row, |r| {
                                ov_btn_add = r.tool_button("Add");
                                ov_btn_del = r.tool_button("Remove");
                                ov_btn_up = r.tool_button("Up");
                                ov_btn_down = r.tool_button("Down");
                            });
                            let sel = state.wizard_col_sel.min(state.wizard_cols.len().saturating_sub(1));
                            let coldef = state.wizard_cols.get(sel);
                            c.text(format!("Selected column #{}", sel + 1));
                            c.row(row, |r| {
                                r.text("Name");
                                ov_col_name = r.text_input_styled(
                                    coldef.map(|c| c.name.as_str()).unwrap_or(""),
                                    "col",
                                    name_s,
                                );
                                r.text("Type");
                                let types = AppState::column_type_names();
                                let ti = coldef
                                    .and_then(|c| {
                                        types.iter().position(|t| t == &c.type_name)
                                    })
                                    .unwrap_or(1);
                                ov_col_type = r.select(types, Some(ti), "type", type_sel);
                            });
                            c.row(row, |r| {
                                ov_pk = r.switch_styled("PK", coldef.map(|c| c.pk).unwrap_or(false), sw);
                                ov_nn = r.switch_styled("Not Null", coldef.map(|c| c.notnull).unwrap_or(false), sw);
                                ov_unique = r.switch_styled("Unique", coldef.map(|c| c.unique).unwrap_or(false), sw);
                            });
                            c.row(row, |r| {
                                r.text("Default");
                                ov_default = r.text_input_styled(
                                    coldef.map(|c| c.default_value.as_str()).unwrap_or(""),
                                    "default",
                                    wide_s,
                                );
                            });
                        }
                        OverlayKind::EditIndex => {
                            c.row(row, |r| {
                                r.text("Index name");
                                ov_name = r.text_input_styled(&state.wizard_index_name, "name", name_s);
                            });
                            let tables = state.table_names();
                            let tsel = tables.iter().position(|t| t == &state.wizard_index_table);
                            c.row(row, |r| {
                                r.text("Table");
                                ov_idx_table = r.select(
                                    if tables.is_empty() {
                                        vec!["(none)".into()]
                                    } else {
                                        tables
                                    },
                                    tsel,
                                    "table",
                                    type_sel,
                                );
                            });
                            c.row(row, |r| {
                                r.text("Columns");
                                ov_idx_cols = r.text_input_styled(
                                    &state.wizard_index_cols,
                                    "col1, col2",
                                    wide_s,
                                );
                            });
                            ov_idx_unique = c.switch_styled("Unique", state.wizard_index_unique, sw);
                        }
                        OverlayKind::ImportCsv | OverlayKind::PasteCsv => {
                            if kind == OverlayKind::ImportCsv {
                                c.text(format!("File: {}", state.csv_path));
                            } else {
                                c.text("Paste CSV below");
                                ov_csv_paste = c.text_area(&state.csv_paste, "csv…", area_s);
                            }
                            c.row(row, |r| {
                                r.text("Table");
                                ov_csv_table = r.text_input_styled(&state.csv_table, "table", name_s);
                            });
                            ov_csv_header = c.switch_styled("First row is header", state.csv_has_header, sw);
                            c.row(row, |r| {
                                r.text("Delimiter");
                                ov_csv_delim = r.select(
                                    vec![String::from(","), String::from(";"), String::from("Tab"), String::from("|")],
                                    Some(state.csv_delim_idx),
                                    "delim",
                                    type_sel,
                                );
                                r.text("Quote");
                                ov_csv_quote = r.select(
                                    vec![String::from("\""), String::from("'"), String::from("None")],
                                    Some(state.csv_quote_idx),
                                    "quote",
                                    type_sel,
                                );
                            });
                            c.text("Encoding: UTF-8");
                        }
                        OverlayKind::FindReplace => {
                            c.row(row, |r| {
                                r.text("Find");
                                ov_find = r.text_input_styled(&state.find_text, "find", wide_s);
                            });
                            c.row(row, |r| {
                                r.text("Replace");
                                ov_replace = r.text_input_styled(&state.replace_text, "replace", wide_s);
                            });
                            c.row(row, |r| {
                                ov_btn_find = r.tool_button("Find Next");
                                ov_btn_repl = r.tool_button("Replace");
                                ov_btn_repl_all = r.tool_button("Replace All");
                            });
                        }
                        OverlayKind::Attach => {
                            c.row(row, |r| {
                                r.text("Path");
                                ov_attach_path = r.text_input_styled(&state.attach_path, "db path", wide_s);
                            });
                            c.row(row, |r| {
                                r.text("Schema name");
                                ov_attach_schema =
                                    r.text_input_styled(&state.attach_schema, "schema", name_s);
                            });
                        }
                    }
                    c.row(row, |r| {
                        ov_btn_ok = r.tool_button(match kind {
                            OverlayKind::FindReplace => "Close",
                            OverlayKind::ImportCsv | OverlayKind::PasteCsv => "Import",
                            OverlayKind::Attach => "Attach",
                            _ => "Save",
                        });
                        ov_btn_cancel = r.tool_button("Cancel");
                    });
                });
            });
        } else {
            // —— Main + docks ——
            let table_names = state.table_names();
            let browse_opts: Vec<String> = if table_names.is_empty() {
                vec!["(no tables)".into()]
            } else {
                table_names.clone()
            };
            let browse_sel = state
                .browse()
                .table
                .as_ref()
                .and_then(|t| browse_opts.iter().position(|x| x == t));
            let chart_sel = state
                .chart_table
                .as_ref()
                .and_then(|t| browse_opts.iter().position(|x| x == t));
            let browse_cols = state
                .browse()
                .result
                .as_ref()
                .map(|r| decorate_browse_cols(state, &r.columns))
                .unwrap_or_else(|| vec![TableColumn::new("—", 120.0)]);
            let browse_rows = state
                .browse()
                .result
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
            let chart_tiles = vec![ChartTile::trend(
                format!(
                    "{} · {} ({} samples)",
                    state.chart_table.as_deref().unwrap_or("—"),
                    state.chart_column,
                    state.chart_points.len()
                ),
                chart_points,
                TrendStyle::from_theme(&theme),
            )];

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
            ddl_style.height = 180.0;

            let mut tool_row = RowStyle::from_theme(&theme);
            tool_row.width = main_w - 24.0;
            tool_row.height = 32.0;
            tool_row.gap = 6.0;
            tool_row.padding = 0.0;
            tool_row.cross_align = CrossAlign::Center;

            let mut filt_row = tool_row;
            filt_row.height = 28.0;

            let mut sel_style = SelectStyle::from_theme(&theme);
            sel_style.width = 160.0;
            let mut filter_style = TextInputStyle::from_theme(&theme);
            filter_style.width = 200.0;
            let mut col_filt_style = TextInputStyle::from_theme(&theme);
            col_filt_style.width = 90.0;
            col_filt_style.height = 26.0;

            let mut browse_table_style = TableStyle::from_theme(&theme);
            browse_table_style.width = main_w - 24.0;
            browse_table_style.height = (tab_page_h - 140.0).max(160.0);

            let mut sql_tb = ToolbarStyle::from_theme(&theme);
            sql_tb.width = main_w - 24.0;
            sql_tb.height = 34.0;
            let mut sql_area_style = TextAreaStyle::from_theme(&theme);
            sql_area_style.width = main_w - 24.0;
            sql_area_style.height = 120.0;
            let mut sql_table_style = TableStyle::from_theme(&theme);
            sql_table_style.width = main_w - 24.0;
            sql_table_style.height = (tab_page_h - 250.0).max(120.0);

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
            let bt = state.browse();
            let col_filter_vals = bt.col_filters.clone();
            let col_names = bt
                .result
                .as_ref()
                .map(|r| r.columns.clone())
                .unwrap_or_default();

            let mut pragma_col = ColumnStyle::from_theme(&theme);
            pragma_col.width = main_w - 24.0;
            pragma_col.height = (tab_page_h - 60.0).max(200.0);
            pragma_col.padding = 4.0;
            pragma_col.gap = 4.0;
            pragma_col.cross_align = CrossAlign::Start;

            let sw = SwitchStyle::from_theme(&theme);
            let mut prag_sel = SelectStyle::from_theme(&theme);
            prag_sel.width = 160.0;
            let mut prag_inp = TextInputStyle::from_theme(&theme);
            prag_inp.width = 180.0;

            col.splitter(
                body_split,
                |left| {
                    tabs = left.tabs(tabs_style, |tb| {
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
                                        ddl_area = c.text_area(&state.ddl_draft, "CREATE / ALTER…", ddl_style);
                                        c.row(tool_row, |row| {
                                            btn_run_ddl = row.tool_button("Run DDL");
                                        });
                                        if let Some(sel) = &state.selected {
                                            c.text(format!("Selected: {:?} {}", sel.kind, sel.name));
                                        } else {
                                            c.text("Selected: (none) — use New Table / New Index wizards");
                                        }
                                    });
                                },
                            );
                        });

                        tb.page(MainTab::Browse.title(), |p| {
                            // Browse multi-tabs strip
                            let titles: Vec<String> =
                                state.browse_tabs.iter().map(|t| t.title.clone()).collect();
                            p.row(tool_row, |row| {
                                btn_browse_new_tab = row.tool_button("+ Tab");
                                for (i, title) in titles.iter().enumerate() {
                                    let label = if i == state.browse_tab_idx {
                                        format!("[{title}]")
                                    } else {
                                        title.clone()
                                    };
                                    browse_tab_btns.push(row.tool_button(label));
                                }
                            });
                            browse_tabs = WidgetId(0);
                            p.row(tool_row, |row| {
                                row.text("Table");
                                browse_select =
                                    row.select(browse_opts_a.clone(), browse_sel, "table…", sel_style);
                                browse_filter = row.text_input_styled(
                                    &state.browse().filter,
                                    "WHERE …",
                                    filter_style,
                                );
                                btn_browse_go = row.tool_button("Apply");
                                btn_add_row = row.tool_button("New Record");
                                btn_del_row = row.tool_button("Delete Record");
                            });
                            // Per-column filters
                            if !col_names.is_empty() {
                                p.row(filt_row, |row| {
                                    row.text_styled("Filters", theme.colors.text_secondary, 12.0);
                                    for (i, name) in col_names.iter().enumerate() {
                                        let val = col_filter_vals.get(i).cloned().unwrap_or_default();
                                        let id = row.text_input_styled(
                                            &val,
                                            name.as_str(),
                                            col_filt_style,
                                        );
                                        browse_col_filters.push(id);
                                    }
                                });
                            }
                            browse_table =
                                p.table(browse_cols.clone(), browse_rows.clone(), browse_table_style);
                            p.text_styled(
                                "Tip: header click sorts · column filters → LIKE · + Tab opens another table",
                                theme.colors.text_secondary,
                                12.0,
                            );
                        });

                        tb.page(MainTab::Pragmas.title(), |p| {
                            p.text("Database preferences (PRAGMA). Edit values then Save Pragmas.");
                            p.column(pragma_col, |c| {
                                for (i, (key, _val)) in state.pragmas.iter().enumerate() {
                                    let draft = state
                                        .pragma_drafts
                                        .get(i)
                                        .cloned()
                                        .unwrap_or_default();
                                    c.row(tool_row, |row| {
                                        row.text_styled(key, theme.colors.text_primary, 13.0);
                                        match key.as_str() {
                                            "foreign_keys" | "recursive_triggers" => {
                                                let on = draft == "1"
                                                    || draft.eq_ignore_ascii_case("on")
                                                    || draft == "true";
                                                let id = row.switch_styled(key, on, sw);
                                                pragma_switches.push(id);
                                                pragma_inputs.push(WidgetId(0));
                                                pragma_selects.push(WidgetId(0));
                                            }
                                            "journal_mode" => {
                                                let opts = vec![
                                                    String::from("delete"),
                                                    String::from("truncate"),
                                                    String::from("persist"),
                                                    String::from("memory"),
                                                    String::from("wal"),
                                                    String::from("off"),
                                                ];
                                                let sel = opts
                                                    .iter()
                                                    .position(|x: &String| x.eq_ignore_ascii_case(&draft))
                                                    .unwrap_or(0);
                                                let id = row.select(opts, Some(sel), key, prag_sel);
                                                pragma_selects.push(id);
                                                pragma_inputs.push(WidgetId(0));
                                                pragma_switches.push(WidgetId(0));
                                            }
                                            "synchronous" => {
                                                let opts = vec![
                                                    String::from("OFF"),
                                                    String::from("NORMAL"),
                                                    String::from("FULL"),
                                                    String::from("EXTRA"),
                                                ];
                                                let sel = match draft.as_str() {
                                                    "0" | "OFF" | "off" => 0,
                                                    "1" | "NORMAL" | "normal" => 1,
                                                    "3" | "EXTRA" | "extra" => 3,
                                                    _ => 2,
                                                };
                                                let id = row.select(opts, Some(sel), key, prag_sel);
                                                pragma_selects.push(id);
                                                pragma_inputs.push(WidgetId(0));
                                                pragma_switches.push(WidgetId(0));
                                            }
                                            _ => {
                                                let id = row.text_input_styled(&draft, key, prag_inp);
                                                pragma_inputs.push(id);
                                                pragma_switches.push(WidgetId(0));
                                                pragma_selects.push(WidgetId(0));
                                            }
                                        }
                                    });
                                }
                            });
                            p.row(tool_row, |row| {
                                btn_pragma_apply = row.tool_button("Save Pragmas");
                            });
                        });

                        tb.page(MainTab::ExecuteSql.title(), |p| {
                            p.toolbar(sql_tb, |tb| {
                                btn_sql_new_tab = tb.tool_button("New Tab");
                                btn_sql_open = tb.tool_button("Open SQL");
                                btn_sql_save = tb.tool_button("Save SQL");
                                btn_sql_run = tb.tool_button("Execute");
                                btn_sql_stop = tb.tool_button("Stop");
                                btn_sql_find = tb.tool_button("Find");
                                tb.spacer(8.0, 1.0);
                                tb.text_styled(
                                    format!("SQL {}", state.sql_tab_idx + 1),
                                    theme.colors.text_secondary,
                                    12.0,
                                );
                            });
                            // SQL editor tabs as buttons
                            p.row(tool_row, |row| {
                                for i in 0..state.sql_tabs.len() {
                                    let label = if i == state.sql_tab_idx {
                                        format!("[SQL {}]", i + 1)
                                    } else {
                                        format!("SQL {}", i + 1)
                                    };
                                    sql_tab_btns.push(row.tool_button(label));
                                }
                            });
                            sql_tabs_w = WidgetId(0);
                            sql_area = p.text_area(&sql_editor, "SQL…", sql_area_style);
                            let tone = if state.error_msg.is_some() {
                                AlertTone::Error
                            } else if state.sql_busy {
                                AlertTone::Warning
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
                        right.text_styled("(docks hidden)", theme.colors.text_secondary, 12.0);
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
                                            if let Some(row) = state.browse().sel_row {
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
        }
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
        "{}{}{}{}",
        if state.read_only { "RO" } else { "RW" },
        if state.dirty { " · modified" } else { "" },
        if state.sql_busy { " · busy" } else { "" },
        " · UTF-8 · not encrypted"
    );
    let status_msg = state.status_msg.clone();
    let status_fg = theme.colors.text_secondary;
    ui.status_bar(status_style, |bar| {
        status = bar.text_styled(format!("{path} · {flags} · {status_msg}"), status_fg, 12.0);
    });

    if state.overlay.is_none() {
        let _ = ui.set_tabs_selected(tabs, state.tab.index());
        if docks_on && dock_tabs.0 != 0 {
            let focus_idx = visible_docks_index(state);
            let _ = ui.set_tabs_selected(dock_tabs, focus_idx);
        }
        if tree.0 != 0 {
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
        }
        if let Some(row) = state.browse().sel_row {
            if browse_table.0 != 0 {
                let _ = ui.set_table_selected_row(browse_table, Some(row));
            }
        }

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
        browse_tabs,
        btn_browse_new_tab,
        browse_select,
        browse_filter,
        btn_browse_go,
        btn_add_row,
        btn_del_row,
        browse_table,
        browse_col_filters,
        browse_tab_btns,
        sql_tab_btns,
        pragma_inputs,
        pragma_switches,
        pragma_selects,
        btn_pragma_apply,
        sql_tabs: sql_tabs_w,
        sql_area,
        btn_sql_run,
        btn_sql_new_tab,
        btn_sql_open,
        btn_sql_save,
        btn_sql_find,
        btn_sql_stop,
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
        ov_name,
        ov_list,
        ov_col_name,
        ov_col_type,
        ov_pk,
        ov_nn,
        ov_unique,
        ov_default,
        ov_btn_add,
        ov_btn_del,
        ov_btn_up,
        ov_btn_down,
        ov_btn_ok,
        ov_btn_cancel,
        ov_idx_table,
        ov_idx_cols,
        ov_idx_unique,
        ov_csv_table,
        ov_csv_header,
        ov_csv_delim,
        ov_csv_quote,
        ov_csv_paste,
        ov_attach_path,
        ov_attach_schema,
        ov_find,
        ov_replace,
        ov_btn_find,
        ov_btn_repl,
        ov_btn_repl_all,
    }
}

fn visible_docks_index(state: &AppState) -> usize {
    state
        .visible_docks()
        .iter()
        .position(|d| *d == state.dock_focus)
        .unwrap_or(0)
}
