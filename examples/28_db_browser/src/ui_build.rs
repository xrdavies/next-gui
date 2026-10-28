//! Build DB4S-style chrome + Phase 2 overlays / browse / pragmas / SQL.

use next_gui_layout::{Axis, CrossAlign};
use next_gui_scene::{Color, Scene};
use next_gui_text::Font;
use next_gui_ui::{
    AlertStyle, AlertTone, ChartStyle, ChartTile, ColumnStyle, ImageStyle, ListStyle, MenuItem,
    MenuStyle, ModalStyle, PanelStyle, RowStyle, SelectStyle, SplitterStyle, StatusBarStyle,
    SwitchStyle, TableColumn, TableStyle, TabsStyle, TextAreaStyle, TextInputStyle, Theme,
    ToolbarStyle, TreeNode, TreeStyle, TrendStyle, Ui, WidgetId,
};

use crate::db::{self, ObjectKind};
use crate::state::{AppState, ConfirmKind, DockKind, MainTab, OverlayKind};

const TABLE_PANE_MINIMUMS:(f32,f32)=(76.0,48.0);

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
    pub const FILE_PROJECT_SAVE_AS: u64 = 20;
    pub const FILE_PROJECT_CLOSE: u64 = 21;
    pub const FILE_SAVE_ALL: u64 = 22;
    pub const FILE_PRINT_SCHEMA: u64 = 23;
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
    pub const VIEW_TB_DB: u64 = 45;
    pub const VIEW_TB_EXTRA: u64 = 46;
    pub const VIEW_TB_PROJECT: u64 = 47;
    pub const VIEW_TB_SQL: u64 = 48;
    pub const VIEW_DOCK_REMOTE: u64 = 49;

    pub const TOOLS_ENCRYPT: u64 = 50;
    pub const TOOLS_EXT: u64 = 51;
    pub const TOOLS_INTEGRITY: u64 = 52;
    pub const TOOLS_QUICK: u64 = 53;
    pub const TOOLS_FK: u64 = 54;
    pub const TOOLS_OPTIMIZE: u64 = 55;
    pub const TOOLS_ROWCOUNTS: u64 = 56;

    pub const BROWSE_FREEZE: u64 = 70;
    pub const BROWSE_SELECT_COLUMN: u64 = 71;
    pub const BROWSE_COPY_COLUMN: u64 = 72;

    pub const HELP_ABOUT: u64 = 60;
    pub const HELP_WIKI: u64 = 61;
    pub const HELP_WEBSITE: u64 = 62;
    pub const HELP_SQLCIPHER: u64 = 63;
    pub const HELP_BUG: u64 = 64;
    pub const HELP_FEATURE: u64 = 65;
    pub const HELP_DONATE: u64 = 66;
    pub const HELP_UPDATES: u64 = 67;
}

#[allow(dead_code)]
pub struct BuiltUi {
    pub ui: Ui,
    pub csv_files: crate::csv_files::Controls,
    pub column_picker: crate::column_picker::Controls,
    pub reference_editor: crate::reference_editor::Controls,
    pub body_splitter: WidgetId,
    pub structure_splitter: WidgetId,
    pub shell_scroll: WidgetId,
    pub page_scroll: WidgetId,
    pub dock_scroll: WidgetId,
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
    pub btn_browse_close_tab: WidgetId,
    pub browse_select: WidgetId,
    pub browse_filter: WidgetId,
    pub browse_global_filter: WidgetId,
    pub btn_browse_clear_filters: WidgetId,
    pub btn_browse_clear_sort: WidgetId,
    pub btn_browse_go: WidgetId,
    pub btn_record_prev: WidgetId,
    pub btn_record_next: WidgetId,
    pub btn_record_first: WidgetId,
    pub btn_record_last: WidgetId,
    pub record_number: WidgetId,
    pub btn_record_go: WidgetId,
    pub btn_add_row: WidgetId,
    pub btn_del_row: WidgetId,
    pub browse_table: WidgetId,
    pub browse_header_menu: WidgetId,
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
    pub btn_sql_save_as: WidgetId,
    pub btn_sql_close: WidgetId,
    pub btn_sql_left: WidgetId,
    pub btn_sql_right: WidgetId,
    pub btn_sql_save: WidgetId,
    pub btn_sql_find: WidgetId,
    pub btn_sql_stop: WidgetId,
    pub btn_sql_csv: WidgetId,
    pub btn_sql_json: WidgetId,
    pub sql_table: WidgetId,
    pub sql_alert: WidgetId,
    pub dock_tabs: WidgetId,
    pub edit_col: WidgetId,
    pub edit_value: WidgetId,
    pub edit_type: WidgetId,
    pub btn_edit_discard: WidgetId,
    pub btn_edit_cell: WidgetId,
    pub schema_list: WidgetId,
    pub log_area: WidgetId,
    pub log_filter: WidgetId,
    pub log_clear: WidgetId,
    pub log_export: WidgetId,
    pub chart_select: WidgetId,
    pub chart_col: WidgetId,
    pub btn_chart: WidgetId,
    pub chart: WidgetId,
    pub status: WidgetId,
    pub modal: WidgetId,
    // Overlay widgets
    pub ov_name: WidgetId,
    pub ov_schema: WidgetId,
    pub ov_scroll: WidgetId,
    pub insert_values: Vec<WidgetId>,
    pub insert_modes: Vec<WidgetId>,
    pub ov_list: WidgetId,
    pub ov_fields_table: WidgetId,
    pub ov_field_editor: WidgetId,
    pub ov_constraint_editor: WidgetId,
    pub ov_field_top: WidgetId,
    pub ov_field_bottom: WidgetId,
    pub ov_col_name: WidgetId,
    pub ov_col_type: WidgetId,
    pub ov_pk: WidgetId,
    pub ov_ai: WidgetId,
    pub ov_collation: WidgetId,
    pub ov_check: WidgetId,
    pub ov_foreign_key: WidgetId,
    pub ov_generated: WidgetId,
    pub ov_generation: WidgetId,
    pub ov_table_preview: WidgetId,
    pub ov_table_splitter: WidgetId,
    pub ov_table_tabs: WidgetId,
    pub ov_table_advanced: WidgetId,
    pub ov_checks_scroll: WidgetId,
    pub ov_checks_table: WidgetId,
    pub ov_check_name: WidgetId,
    pub ov_check_expr: WidgetId,
    pub ov_check_add: WidgetId,
    pub ov_check_add_column: WidgetId,
    pub ov_check_del: WidgetId,
    pub ov_check_up: WidgetId,
    pub ov_check_down: WidgetId,
    pub ov_keys_scroll: WidgetId,
    pub ov_keys_table: WidgetId,
    pub ov_key_name: WidgetId,
    pub ov_key_columns: WidgetId,
    pub ov_key_conflict: WidgetId,
    pub ov_key_add_pk: WidgetId,
    pub ov_key_add_unique: WidgetId,
    pub ov_key_del: WidgetId,
    pub ov_key_up: WidgetId,
    pub ov_key_down: WidgetId,
    pub ov_fks_scroll: WidgetId,
    pub ov_fks_table: WidgetId,
    pub ov_fk_name: WidgetId,
    pub ov_fk_columns: WidgetId,
    pub ov_fk_clause: WidgetId,
    pub ov_fk_add: WidgetId,
    pub ov_fk_del: WidgetId,
    pub ov_fk_up: WidgetId,
    pub ov_fk_down: WidgetId,
    pub ov_without_rowid: WidgetId,
    pub ov_strict: WidgetId,
    pub ov_pk_conflict: WidgetId,
    pub ov_nn: WidgetId,
    pub ov_unique: WidgetId,
    pub ov_default: WidgetId,
    pub ov_btn_add: WidgetId,
    pub ov_btn_del: WidgetId,
    pub ov_btn_up: WidgetId,
    pub ov_btn_down: WidgetId,
    pub ov_btn_ok: WidgetId,
    pub ov_btn_cancel: WidgetId,
    pub ov_idx_available: WidgetId,
    pub ov_idx_expression: WidgetId,
    pub ov_idx_edit_key: WidgetId,
    pub ov_idx_preview: WidgetId,
    pub ov_idx_order: WidgetId,
    pub ov_idx_table: WidgetId,
    pub ov_idx_cols: WidgetId,
    pub ov_idx_where: WidgetId,
    pub ov_idx_unique: WidgetId,
    pub ov_csv_table: WidgetId,
    pub ov_csv_header: WidgetId,
    pub ov_csv_delim: WidgetId,
    pub ov_csv_quote: WidgetId,
    pub ov_csv_encoding: WidgetId,
    pub ov_csv_custom_encoding: WidgetId,
    pub ov_csv_custom_delimiter: WidgetId,
    pub ov_csv_custom_quote: WidgetId,
    pub ov_csv_trim: WidgetId,
    pub ov_csv_local: WidgetId,
    pub ov_csv_types: WidgetId,
    pub ov_csv_defaults: WidgetId,
    pub ov_csv_missing: WidgetId,
    pub ov_csv_conflict: WidgetId,
    pub ov_csv_advanced: WidgetId,
    pub ov_csv_refresh: WidgetId,
    pub ov_csv_preview: WidgetId,
    pub ov_csv_preview_error: WidgetId,
    pub ov_csv_scroll: WidgetId,
    pub ov_csv_paste: WidgetId,
    pub ov_attach_path: WidgetId,
    pub ov_attach_schema: WidgetId,
    pub ov_find: WidgetId,
    pub ov_replace: WidgetId,
    pub ov_find_options: [WidgetId; 6],
    pub ov_btn_find_all: WidgetId,
    pub ov_btn_find: WidgetId,
    pub ov_btn_repl: WidgetId,
    pub ov_btn_repl_all: WidgetId,
    pub ov_cipher_pass: WidgetId,
    pub ov_cipher_page: WidgetId,
    pub ov_cipher_kdf: WidgetId,
    pub ov_prefs_font: WidgetId,
    pub ov_prefs_theme: WidgetId,
    pub ov_prefs_confirm: WidgetId,
    pub ov_prefs_completion: WidgetId,
    pub ov_prefs_upper_keywords: WidgetId,
    pub ov_prefs_delim: WidgetId,
    pub ov_frozen: WidgetId,
    pub ov_export_struct: WidgetId,
    pub ov_export_data: WidgetId,
    pub ov_export_drop: WidgetId,
    pub ov_export_fk: WidgetId,
    pub ov_cond_col: WidgetId,
    pub ov_cond_op: WidgetId,
    pub ov_cond_val: WidgetId,
    pub ov_cond_color: WidgetId,
    pub btn_sql_line: WidgetId,
    pub btn_sql_comment: WidgetId,
    pub btn_sql_view: WidgetId,
    pub sql_view_name: WidgetId,
    pub btn_edit_text: WidgetId,
    pub btn_edit_hex: WidgetId,
    pub btn_edit_json: WidgetId,
    pub btn_edit_image: WidgetId,
    pub btn_fk_nav: WidgetId,
    pub btn_freeze: WidgetId,
    pub btn_cond_fmt: WidgetId,
    pub edit_meta: WidgetId,
    pub toolbar_extra: WidgetId,
    pub toolbar_project: WidgetId,
    pub btn_project_save: WidgetId,
    pub btn_integrity: WidgetId,
    pub ov_ext_path: WidgetId,
    pub ov_ext_entry: WidgetId,
    pub btn_form_view: WidgetId,
    pub btn_form_prev: WidgetId,
    pub btn_form_next: WidgetId,
    pub form_fields: Vec<WidgetId>,
    pub image_fit: WidgetId,
    pub image_actual: WidgetId,
    pub image_in: WidgetId,
    pub image_out: WidgetId,
    pub image_export: WidgetId,
    pub edit_image: WidgetId,
    pub remote_list: WidgetId,
    pub remote_draft: WidgetId,
    pub btn_remote_add: WidgetId,
    pub btn_remote_open: WidgetId,
}

pub fn clear_color(theme: &Theme) -> Color {
    // Match tab/shell fills so gaps never show a stale dark clear.
    theme.colors.content_bg
}

fn schema_tree(state: &AppState) -> Vec<TreeNode> {
    state.schema_names.iter().enumerate().map(|(database, name)| {
        let root = database as u64 * 5 + 1;
        let groups = [ObjectKind::Table, ObjectKind::View, ObjectKind::Index, ObjectKind::Trigger]
            .into_iter().enumerate().map(|(group, kind)| {
                let objects: Vec<_> = state.schema.iter().enumerate()
                    .filter(|(_, o)| o.schema == *name && o.kind == kind)
                    .map(|(i, o)| TreeNode::leaf(10_000 + i as u64, o.name.clone())).collect();
                TreeNode::new(root + group as u64 + 1, format!("{} ({})", kind.label(), objects.len()), objects)
            }).collect();
        TreeNode::new(root, name.clone(), groups)
    }).collect()
}

pub fn status_text(state: &AppState) -> String {
    let path = if state.path.is_empty() {
        "(no database)".to_string()
    } else {
        state.path.clone()
    };
    let flags = format!(
        "{}{}{}{} · {} · {}",
        if state.read_only { "RO" } else { "RW" },
        if state.dirty { " · modified" } else { "" },
        if state.sql_busy { " · busy" } else { "" },
        if state.cipher_linked {
            " · SQLCipher"
        } else {
            ""
        },
        if state.database_encoding.is_empty() { "—" } else { &state.database_encoding },
        if state.cipher_active {
            "encrypted"
        } else {
            "not encrypted"
        },
    );
    format!("{path} · {flags} · {}",state.status_msg)
}

fn constraint_cell_editor(c: &mut next_gui_ui::FlexBuilder<'_>, table: WidgetId, state: &AppState, theme: &Theme) -> WidgetId {
    let Some(edit) = &state.wizard_constraint_edit else { return WidgetId(0); };
    let Some(value) = state.constraint_cell_value(edit.page, edit.row, edit.column) else { return WidgetId(0); };
    c.table_cell_editor(table, edit.row, edit.column, |cell| {
        if (edit.page,edit.column)==(0,1) {
            cell.select(["PRIMARY KEY","UNIQUE"],Some(usize::from(!state.wizard_index_constraints[edit.row].primary)),"",SelectStyle::from_theme(theme))
        } else {cell.text_input_styled(value,"",TextInputStyle::from_theme(theme))}
    }).unwrap_or(WidgetId(0))
}

pub fn table_index_rows(state: &AppState) -> Vec<Vec<String>> {
    state.wizard_index_constraints.iter().map(|key|vec![key.columns.clone(),
        format!("{}{}",if key.primary {"PRIMARY KEY"} else {"UNIQUE"},if key.inline {" · column"} else {""}),
        key.name.clone().unwrap_or_default(),crate::table_sql::index_constraint_sql(key).unwrap_or_else(|error|format!("-- {error}"))]).collect()
}

pub fn table_foreign_key_rows(state: &AppState) -> Vec<Vec<String>> {
    state.wizard_foreign_keys.iter().map(|key|vec![format!("{}{}",key.columns,if key.inline {" · column"} else {""}),
        key.name.clone().unwrap_or_default(),key.clause.clone(),
        crate::table_sql::foreign_key_sql(key).unwrap_or_else(|error|format!("-- {error}"))]).collect()
}

pub fn table_check_rows(state: &AppState) -> Vec<Vec<String>> {
    state.wizard_checks.iter().map(|check|vec![check.column.as_ref().map_or_else(||check.expression.clone(),
        |owner|format!("Column {} · {}",db::quote_ident(owner),check.expression)),check.name.clone().unwrap_or_default(),
        crate::table_sql::check_sql(check).unwrap_or_else(|error|format!("-- {error}"))]).collect()
}

pub fn table_field_rows(state: &AppState) -> Vec<Vec<String>> {
    let flag = |value| if value { "Yes".to_string() } else { String::new() };
    state.wizard_cols.iter().map(|column|vec![column.name.clone(),
        format!("{}{}",column.type_name,column.generated.as_ref().map_or("",|generated|if generated.stored { " · STORED" } else { " · VIRTUAL" })),
        flag(column.notnull),flag(column.pk),flag(column.auto_increment),flag(column.unique),column.default_value.clone(),
        column.check_expression.clone(),column.collation.clone(),column.foreign_key_clause.clone()]).collect()
}

pub fn table_field_checks(state: &AppState) -> Vec<Vec<Option<bool>>> {
    state.wizard_cols.iter().map(|column|vec![None,None,Some(column.notnull),Some(column.pk),
        Some(column.auto_increment),Some(column.unique)]).collect()
}

fn decorate_browse_cols(state: &AppState, cols: &[String]) -> Vec<TableColumn> {
    let n = cols.len().max(1);
    let w = (720.0 / n as f32).clamp(72.0, 200.0);
    if cols.is_empty() {
        return vec![TableColumn::new("—", 120.0)];
    }
    let ordering = state.browse().ordering();
    cols.iter()
        .map(|c| {
            let title = if let Some((priority, order)) = ordering.iter().enumerate().find(|(_, o)| o.column == *c) {
                let arrow = if order.descending { "▼" } else { "▲" };
                if ordering.len() > 1 { format!("{c} {arrow}{}", priority + 1) } else { format!("{c} {arrow}") }
            } else { c.clone() };
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
    cols.iter()
        .map(|c| TableColumn::new(c.clone(), w))
        .collect()
}

/// Desktop typography and restrained surfaces, using the same widget token system.
pub fn browser_theme(mut theme: Theme, font_size: f32) -> Theme {
    theme.typography.body = font_size;
    theme.typography.input = font_size;
    theme.typography.button = font_size;
    theme.typography.button_sm = font_size;
    theme.typography.title = font_size + 2.0;
    theme.typography.caption = (font_size - 2.0).max(10.0);
    theme.radii.control = 5.0;
    theme.radii.panel = 8.0;
    let dark = theme.mode == next_gui_ui::ThemeMode::Dark;
    let c = &mut theme.colors;
    c.accent = Color::rgb(57, 113, 220);
    c.on_accent = Color::rgb(255, 255, 255);
    c.surface = if dark { Color::rgb(23, 26, 32) } else { Color::rgb(245, 247, 251) };
    c.surface_elevated = if dark { Color::rgb(30, 34, 42) } else { Color::rgb(235, 239, 246) };
    c.content_bg = c.surface;
    c.panel = c.surface;
    c.list_bg = c.surface;
    c.scroll_bg = c.surface;
    c.strip_bg = c.surface_elevated;
    c.title_bar = c.surface_elevated;
    c.header_bg = if dark { Color::rgb(36, 42, 53) } else { Color::rgb(229, 235, 244) };
    c.button_bg = if dark { Color::rgb(43, 50, 63) } else { Color::rgb(225, 231, 240) };
    c.button_fg = if dark { Color::rgb(225, 231, 241) } else { Color::rgb(40, 52, 72) };
    c.text_primary = if dark { Color::rgb(225, 231, 241) } else { Color::rgb(32, 43, 61) };
    c.text_secondary = if dark { Color::rgb(152, 164, 185) } else { Color::rgb(91, 106, 128) };
    c.input_bg = if dark { Color::rgb(18, 21, 27) } else { Color::rgb(255, 255, 255) };
    c.grid = if dark { Color::rgb(41, 47, 59) } else { Color::rgb(216, 225, 237) };
    c.border = c.grid;
    c.sash = c.grid;
    c.chrome = if dark { Color::rgb(38, 53, 78) } else { Color::rgb(216, 230, 253) };
    c.selected_bg = c.accent;
    c.muted = c.surface_elevated;
    theme
}

pub fn build_ui(
    font: Font,
    mut theme: Theme,
    width: u32,
    height: u32,
    state: &AppState,
) -> BuiltUi {
    let generated_cell = state.edit_cell_generated();
    theme = browser_theme(theme, state.prefs.font_size);
    let mut scene = Scene::new();
    let mut ui = Ui::with_font(font);
    ui.set_theme(theme);
    ui.set_viewport(width as f32, height as f32);
    let reference_reset_width=ui.font().measure_line("Reset",theme.typography.button_sm.round().max(1.0)).0.max(0.0)+32.0;

    let z = WidgetId(0);
    let mut body_splitter = z;
    let mut structure_splitter = z;
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
    let mut btn_browse_close_tab = z;
    let mut browse_select = z;
    let mut browse_filter = z;
    let mut browse_global_filter = z;
    let mut btn_browse_clear_filters = z;
    let mut btn_browse_clear_sort = z;
    let mut btn_browse_go = z;
    let mut btn_record_prev = z;
    let mut btn_record_next = z;
    let mut btn_record_first = z;
    let mut btn_record_last = z;
    let mut record_number = z;
    let mut btn_record_go = z;
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
    let mut btn_sql_save_as = z;
    let mut btn_sql_close = z;
    let mut btn_sql_left = z;
    let mut btn_sql_right = z;
    let mut btn_sql_save = z;
    let mut btn_sql_find = z;
    let mut btn_sql_stop = z;
    let mut btn_sql_csv = z;
    let mut btn_sql_json = z;
    let mut sql_table = z;
    let mut sql_alert = z;
    let mut dock_tabs = z;
    let mut edit_col = z;
    let mut edit_value = z;
    let mut edit_type = z;
    let mut btn_edit_discard = z;
    let mut btn_edit_cell = z;
    let mut schema_list = z;
    let mut log_area = z;
    let mut log_filter = z;
    let mut log_clear = z;
    let mut log_export = z;
    let mut chart_select = z;
    let mut chart_col = z;
    let mut btn_chart = z;
    let mut chart = z;
    let mut status = z;
    let mut ov_name = z;
    let mut ov_schema = z;
    let mut ov_scroll = z;
    let mut insert_values = Vec::new();
    let mut insert_modes = Vec::new();
    let mut ov_list = z;
    let mut ov_fields_table = z;
    let mut ov_field_editor = z;
    let mut ov_constraint_editor = z;
    let mut reference_editor = crate::reference_editor::Controls::default();
    let mut ov_field_top = z;
    let mut ov_field_bottom = z;
    let mut ov_col_name = z;
    let mut ov_col_type = z;
    let mut ov_pk = z;
    let mut ov_ai = z;
    let mut ov_collation = z;
    let mut ov_check = z;
    let mut ov_foreign_key = z;
    let mut ov_generated = z;
    let mut ov_generation = z;
    let mut ov_table_preview = z;
    let mut ov_table_splitter = z;
    let mut ov_table_tabs = z;
    let mut ov_table_advanced = z;
    let mut ov_checks_scroll = z;
    let mut ov_checks_table = z;
    let mut ov_check_name = z;
    let mut ov_check_expr = z;
    let mut ov_check_add = z;
    let mut ov_check_add_column = z;
    let mut ov_check_del = z;
    let mut ov_check_up = z;
    let mut ov_check_down = z;
    let mut ov_keys_scroll = z;
    let mut ov_keys_table = z;
    let mut ov_key_name = z;
    let mut ov_key_columns = z;
    let mut ov_key_conflict = z;
    let mut ov_key_add_pk = z;
    let mut ov_key_add_unique = z;
    let mut ov_key_del = z;
    let mut ov_key_up = z;
    let mut ov_key_down = z;
    let mut ov_fks_scroll = z;
    let mut ov_fks_table = z;
    let mut ov_fk_name = z;
    let mut ov_fk_columns = z;
    let mut ov_fk_clause = z;
    let mut ov_fk_add = z;
    let mut ov_fk_del = z;
    let mut ov_fk_up = z;
    let mut ov_fk_down = z;
    let mut ov_without_rowid = z;
    let mut ov_strict = z;
    let mut ov_pk_conflict = z;
    let mut ov_nn = z;
    let mut ov_unique = z;
    let mut ov_default = z;
    let mut ov_btn_add = z;
    let mut ov_btn_del = z;
    let mut ov_btn_up = z;
    let mut ov_btn_down = z;
    let mut ov_btn_ok = z;
    let mut ov_btn_cancel = z;
    let mut ov_idx_available = z;
    let mut ov_idx_expression = z;
    let mut ov_idx_edit_key = z;
    let mut ov_idx_preview = z;
    let mut ov_idx_order = z;
    let mut ov_idx_table = z;
    let mut ov_idx_cols = z;
    let mut ov_idx_where = z;
    let mut ov_idx_unique = z;
    let mut ov_csv_table = z;
    let mut ov_csv_header = z;
    let mut ov_csv_delim = z;
    let mut ov_csv_quote = z;
    let mut ov_csv_encoding = z;
    let mut ov_csv_custom_encoding = z;
    let mut ov_csv_custom_delimiter = z;
    let mut ov_csv_custom_quote = z;
    let mut ov_csv_trim = z;
    let mut ov_csv_local = z;
    let mut ov_csv_types = z;
    let mut ov_csv_defaults = z;
    let mut ov_csv_missing = z;
    let mut ov_csv_conflict = z;
    let mut ov_csv_advanced = z;
    let mut ov_csv_refresh = z;
    let mut ov_csv_preview = z;
    let mut ov_csv_preview_error = z;
    let mut ov_csv_scroll = z;
    let mut csv_files = crate::csv_files::Controls::default();
    let mut ov_csv_paste = z;
    let mut ov_attach_path = z;
    let mut ov_attach_schema = z;
    let mut ov_find = z;
    let mut ov_replace = z;
    let mut ov_find_options = [z; 6];
    let mut ov_btn_find_all = z;
    let mut ov_btn_find = z;
    let mut ov_btn_repl = z;
    let mut ov_btn_repl_all = z;
    let mut ov_cipher_pass = z;
    let mut ov_cipher_page = z;
    let mut ov_cipher_kdf = z;
    let mut ov_prefs_font = z;
    let mut ov_prefs_theme = z;
    let mut ov_prefs_confirm = z;
    let mut ov_prefs_completion = z;
    let mut ov_prefs_upper_keywords = z;
    let mut ov_prefs_delim = z;
    let mut ov_frozen = z;
    let mut ov_export_struct = z;
    let mut ov_export_data = z;
    let mut ov_export_drop = z;
    let mut ov_export_fk = z;
    let mut ov_cond_col = z;
    let mut ov_cond_op = z;
    let mut ov_cond_val = z;
    let mut ov_cond_color = z;
    let mut btn_sql_line = z;
    let mut btn_sql_comment = z;
    let mut btn_sql_view = z;
    let mut sql_view_name = z;
    let mut btn_edit_text = z;
    let mut btn_edit_hex = z;
    let mut btn_edit_json = z;
    let mut btn_edit_image = z;
    let mut btn_fk_nav = z;
    let mut btn_freeze = z;
    let mut btn_cond_fmt = z;
    let mut edit_meta = z;
    let mut toolbar_extra = z;
    let mut toolbar_project = z;
    let mut btn_project_save = z;
    let mut btn_integrity = z;
    let mut ov_ext_path = z;
    let mut ov_ext_entry = z;
    let mut btn_form_view = z;
    let mut btn_form_prev = z;
    let mut btn_form_next = z;
    let mut form_fields: Vec<WidgetId> = Vec::new();
    let mut image_fit = z;
    let mut image_actual = z;
    let mut image_in = z;
    let mut image_out = z;
    let mut image_export = z;
    let mut edit_image = z;
    let mut page_scroll = z;
    let mut dock_scroll = z;
    let mut remote_list = z;
    let mut remote_draft = z;
    let mut btn_remote_add = z;
    let mut btn_remote_open = z;

    let win_w = width as f32;
    let win_h = height as f32;
    const STATUS_H: f32 = 40.0;
    let shell_w = (win_w - 12.0).max(640.0);
    let shell_h = (win_h - STATUS_H - 4.0).max(360.0);
    let compact_find = state.overlay == Some(OverlayKind::FindReplace) && win_h < 720.0;
    let extra_visible = state.toolbar_extra && !compact_find;
    let project_visible = state.toolbar_project && !compact_find;
    let chrome_h = 50.0 + if state.toolbar_db { 42.0 } else { 0.0 }
        + if extra_visible { 40.0 } else { 0.0 }
        + if project_visible { 40.0 } else { 0.0 };
    let content_h = (shell_h - chrome_h).max(300.0);
    let docks_on = state.any_dock_visible() && state.overlay.is_none();
    let pane_width = shell_w - 20.0;
    let main_frac = if docks_on { state.body_ratio.clamp(340.0 / pane_width, 1.0 - 240.0 / pane_width) } else { 1.0 };
    let main_w = (shell_w - 20.0) * main_frac;
    let dock_w = (shell_w - 20.0) * (1.0 - main_frac) - 8.0;
    let tab_page_h = (content_h - 36.0).max(240.0);

    let mut menu_style = MenuStyle::from_theme(&theme);
    menu_style.popup_width = menu_style.popup_width.max(280.0);
    menu_style.min_width = menu_style.min_width.max(72.0);
    let check = |on: bool| if on { "✓ " } else { "  " };

    let mut menu_tb = ToolbarStyle::from_theme(&theme);
    menu_tb.width = shell_w - 8.0;
    menu_tb.height = 34.0;
    menu_tb.padding = 2.0;
    menu_tb.gap = 4.0;
    menu_tb.background = Some(theme.colors.strip_bg);

    let mut main_tb = RowStyle::from_theme(&theme);
    main_tb.width = shell_w - 8.0;
    main_tb.height = 38.0;
    main_tb.padding = 4.0;
    main_tb.background = Some(theme.colors.surface_elevated);

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
        file_items.push(MenuItem::new(
            mid::FILE_RECENT_BASE + i as u64,
            format!("Recent: {short}"),
        ));
    }
    file_items.push(MenuItem::new(mid::FILE_PROJECT_OPEN, "Open Project…"));
    file_items.push(MenuItem::new(mid::FILE_PROJECT_SAVE, "Save Project"));
    file_items.push(MenuItem::new(mid::FILE_PROJECT_SAVE_AS, "Save Project As…"));
    file_items.push(MenuItem::new(mid::FILE_PROJECT_CLOSE, "Close Project"));
    file_items.push(MenuItem::new(mid::FILE_SAVE_ALL, "Save All"));
    file_items.push(MenuItem::new(mid::FILE_PRINT_SCHEMA, "Print Schema…"));
    file_items.push(MenuItem::new(mid::FILE_EXIT, "Exit"));

    let mut shell_view = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
    shell_view.x = 6.0;
    shell_view.y = 2.0;
    shell_view.width = (win_w - 12.0).max(1.0);
    shell_view.height = (win_h - STATUS_H - 4.0).max(1.0);
    shell_view.padding = 4.0;
    shell_view.gap = 4.0;
    let shell_scroll = ui.scroll_area(shell_view, |col| {
        col.toolbar(menu_tb, |tb| {
            menu_file = tb.menu("File", file_items.clone(), menu_style);
            menu_edit = tb.menu(
                "Edit",
                vec![
                    MenuItem::new(mid::EDIT_CREATE_TABLE, "Create Table…"),
                    MenuItem::new(mid::EDIT_CREATE_INDEX, "Create Index…"),
                    MenuItem::new(mid::EDIT_MODIFY, "Modify Object…"),
                    MenuItem::new(mid::EDIT_DELETE, "Delete Object…"),
                    MenuItem::new(mid::EDIT_PREFS, "Preferences…"),
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
                    MenuItem::new(mid::VIEW_DOCK_REMOTE, format!("{}Remote", check(state.dock_remote))),
                    MenuItem::new(mid::VIEW_TB_DB, format!("{}DB Toolbar", check(state.toolbar_db))),
                    MenuItem::new(mid::VIEW_TB_EXTRA, format!("{}Extra DB Toolbar", check(state.toolbar_extra))),
                    MenuItem::new(mid::VIEW_TB_PROJECT, format!("{}Project Toolbar", check(state.toolbar_project))),
                    MenuItem::new(mid::VIEW_TB_SQL, format!("{}SQL Toolbar", check(state.toolbar_sql))),
                    MenuItem::new(mid::VIEW_THEME, "Toggle Theme Light/Dark"),
                ],
                menu_style,
            );
            menu_tools = tb.menu(
                "Tools",
                vec![
                    MenuItem::new(mid::TOOLS_ENCRYPT, "Set Encryption…"),
                    MenuItem::new(mid::TOOLS_EXT, "Load Extension…"),
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
                    MenuItem::new(mid::HELP_ABOUT, "About DB Browser"),
                    MenuItem::new(mid::HELP_WEBSITE, "Website"),
                    MenuItem::new(mid::HELP_WIKI, "Wiki"),
                    MenuItem::new(mid::HELP_SQLCIPHER, "SQLCipher FAQ"),
                    MenuItem::new(mid::HELP_BUG, "Bug Report"),
                    MenuItem::new(mid::HELP_FEATURE, "Feature Request"),
                    MenuItem::new(mid::HELP_DONATE, "Donate"),
                    MenuItem::new(mid::HELP_UPDATES, "Check for Updates"),
                ],
                menu_style,
            );
        });

        if state.toolbar_db {
            col.scroll_row(main_tb, |tb| {
                btn_new = tb.tool_button("New");
                btn_open = tb.tool_button("Open");
                btn_write = tb.tool_button_styled("Write Changes", theme.colors.accent, theme.colors.on_accent);
                btn_revert = tb.tool_button("Revert");
                btn_theme = tb.tool_button("Theme");
                tb.spacer(12.0, 1.0);
                let dirty = if state.dirty { "● modified" } else { "○ clean" };
                let ro = if state.read_only { " · read-only" } else { "" };
                let busy = if state.sql_busy { " · busy" } else { "" };
                let enc = if state.cipher_active { " · encrypted" } else { " · not encrypted" };
                tb.text_styled(
                    format!(
                        "{}{ro} · {dirty}{busy}{enc}",
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
        }
        if extra_visible {
            let mut extra_tb = main_tb;
            extra_tb.height = 36.0;
            col.scroll_row(extra_tb, |tb| {
                toolbar_extra = tb.tool_button("Vacuum");
                btn_integrity = tb.tool_button("Integrity");
                tb.text_styled("Extra DB", theme.colors.text_secondary, 11.0);
            });
        }
        if project_visible {
            let mut proj_tb = main_tb;
            proj_tb.height = 36.0;
            col.scroll_row(proj_tb, |tb| {
                toolbar_project = tb.tool_button("Open Project");
                btn_project_save = tb.tool_button("Save Project");
                let label = state
                    .project_path
                    .as_deref()
                    .unwrap_or("(no project)");
                tb.text_styled(label, theme.colors.text_secondary, 11.0);
            });
        }

        if let Some(kind) = state.overlay {
            // —— Overlay wizard panel ——
            let mut panel = PanelStyle::from_theme(&theme);
            panel.width = shell_w - 16.0;
            panel.height = content_h - if matches!(kind, OverlayKind::FindReplace | OverlayKind::EditTable | OverlayKind::ImportCsv | OverlayKind::PasteCsv) { 0.0 } else { 50.0 };
            let title = match kind {
                OverlayKind::InsertRow => "New Record",
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
                OverlayKind::Cipher => if state.cipher_open_path.is_some() { "Open Encrypted Database" } else { "Export Encrypted Copy" },
                OverlayKind::Preferences => "Preferences",
                OverlayKind::ExportSql => "Export Database to SQL",
                OverlayKind::CondFormat => "Conditional Formatting",
                OverlayKind::LoadExtension => "Load Extension",
                OverlayKind::About => "About",
            };
            let mut ov_col = ColumnStyle::from_theme(&theme);
            ov_col.width = shell_w - 40.0;
            ov_col.height = panel.height - panel.title_height - 68.0;
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

            let primary = match kind {
                OverlayKind::FindReplace | OverlayKind::About => "Close",
                OverlayKind::ImportCsv | OverlayKind::PasteCsv => "Import",
                OverlayKind::Attach => "Attach",
                OverlayKind::ExportSql => "Export…",
                OverlayKind::CondFormat => "Add Rule",
                OverlayKind::Cipher => if state.cipher_open_path.is_some() { "Unlock" } else { "Export…" },
                OverlayKind::LoadExtension => "Load",
                _ => "Save",
            };
            let (_ov_panel, ok_id, cancel_id) =
                col.dialog_panel(title, panel, primary, "Cancel", |p| {
                p.column(ov_col, |c| {
                    match kind {
                        OverlayKind::InsertRow => {
                            let mut scroll = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
                            scroll.width = shell_w - 64.0;
                            scroll.height = ov_col.height - 24.0;
                            c.scroll_area(scroll, |fields| {
                                for (i, column) in state.insert_columns.iter().enumerate() {
                                    fields.row(row, |r| {
                                        r.text(format!("{} ({})", column.name, column.type_name));
                                        insert_modes.push(r.select(["Default", "NULL", "Value"], Some(state.insert_modes[i]), "mode", type_sel));
                                        insert_values.push(r.text_input_styled(&state.insert_values[i], column.default_value.as_deref().unwrap_or("value"), wide_s));
                                    });
                                }
                            });
                        }
                        OverlayKind::EditTable => {
                            let mut scroll = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
                            scroll.width = ov_col.width - 16.0;
                            let header_height = if state.wizard_table_advanced { 70.0 } else { 32.0 };
                            let mut split=SplitterStyle::from_theme(&theme);
                            split.width=scroll.width;split.height=(ov_col.height-header_height-22.0).max(100.0);
                            split.axis=Axis::Horizontal;split.ratio=state.wizard_table_ratio;split.sash_size=6.0;
                            // SQL editor sizes must use the constrained pane before its first layout.
                            let span=split.height-split.sash_size;
                            let scale=(span/(TABLE_PANE_MINIMUMS.0+TABLE_PANE_MINIMUMS.1)).min(1.0);
                            split.ratio=split.ratio.clamp(TABLE_PANE_MINIMUMS.0*scale/span,1.0-TABLE_PANE_MINIMUMS.1*scale/span);
                            scroll.height=((split.height-split.sash_size)*split.ratio).round();
                            scroll.padding = 0.0;
                            scroll.background = None;
                            c.row(row, |r| {
                                r.text("Table"); let mut table_name=name_s; table_name.width=(scroll.width-220.0).clamp(220.0,600.0); table_name.size=name_s.size.max(16.0);
                                ov_name=r.text_input_styled(&state.wizard_table_name,"Table name",table_name);
                                ov_table_advanced=r.tool_button(if state.wizard_table_advanced { "Hide advanced" } else { "Advanced" });
                            });
                            if state.wizard_table_advanced {
                                c.row(row, |r| {
                                    r.text("Database");
                                    let schemas=state.table_schemas(); let selected=schemas.iter().position(|s|*s==state.wizard_schema);
                                    ov_schema=r.select(schemas,selected,"schema",type_sel);
                                    r.tooltip("Requires a primary key; removes the hidden rowid and forbids AUTOINCREMENT", |r| {
                                        ov_without_rowid=r.checkbox("Without Rowid",state.wizard_table_options.without_rowid);
                                    });
                                    r.tooltip("Enforce SQLite STRICT column types and stored values", |r| {
                                        ov_strict=r.checkbox("Strict",state.wizard_table_options.strict);
                                    });
                                    r.text("PK conflict"); let mut selector=type_sel; selector.width=120.0;
                                    ov_pk_conflict=r.select(crate::table_sql::PRIMARY_KEY_CONFLICTS.iter().map(|(label,_)|*label),
                                        crate::table_sql::PRIMARY_KEY_CONFLICTS.iter().position(|(_,action)|*action==state.wizard_table_options.primary_key_conflict),"Default",selector);
                                });
                            }
                            let mut table_tabs = TabsStyle::from_theme(&theme);
                            table_tabs.width = scroll.width;
                            table_tabs.height = scroll.height;
                            table_tabs.strip_height = 28.0;
                            table_tabs.padding = 0.0;
                            table_tabs.gap = 0.0;
                            table_tabs.content_bg = theme.colors.content_bg;
                            scroll.height = (table_tabs.height - table_tabs.strip_height).max(48.0);
                            ov_table_splitter=c.splitter(split,|c| {
                            ov_table_tabs = c.tabs(table_tabs, |tabs| {
                            ov_scroll=tabs.scroll_page("Fields",scroll, |c| {
                                c.row(row, |r| {
                                    ov_btn_add = r.tool_button("Add"); ov_btn_del = r.tool_button("Remove");
                                    ov_field_top = r.tool_button("Top"); ov_btn_up = r.tool_button("Up");
                                    ov_btn_down = r.tool_button("Down"); ov_field_bottom = r.tool_button("Bottom");
                                });
                                let mut fields_style = TableStyle::from_theme(&theme);
                                fields_style.width = scroll.width-12.0; fields_style.height = scroll.height.min(180.0);
                                let columns = [("Name",180.0),("Type",160.0),("NN",44.0),("PK",44.0),("AI",44.0),("U",44.0),
                                    ("Default",140.0),("Check",240.0),("Collation",140.0),("Foreign Key",280.0)].into_iter()
                                    .map(|(title,width)|TableColumn::new(title,width)).collect();
                                ov_fields_table = c.table(columns,table_field_rows(state),fields_style);
                                let sel = state.wizard_col_sel.min(state.wizard_cols.len().saturating_sub(1));
                                let coldef = state.wizard_cols.get(sel);
                                if let Some(edit)=&state.wizard_field_edit {
                                    if let Some(draft)=&edit.reference {
                                        let width=state.wizard_field_widths.get(9).copied().unwrap_or(280.0);
                                        reference_editor=crate::reference_editor::build(c,ov_fields_table,(sel,9),width,reference_reset_width.max(64.0),draft,&theme);
                                        ov_field_editor=reference_editor.root;
                                    } else if let Some(column)=coldef {
                                        let value=match edit.column {0=>column.name.as_str(),1=>column.type_name.as_str(),6=>column.default_value.as_str(),7=>column.check_expression.as_str(),8=>column.collation.as_str(),9=>column.foreign_key_clause.as_str(),_=>""};
                                        ov_field_editor=c.table_cell_editor(ov_fields_table,sel,edit.column,|cell| {
                                            if edit.column==1 || edit.column==8 {
                                                let mut items=if edit.column==1 {state.column_type_names()} else {state.column_collation_names()};
                                                if !items.iter().any(|item|item==value) {items.push(value.into());}
                                                if edit.column==8 || state.wizard_table_options.strict {
                                                    let selected=items.iter().position(|item|item==value);
                                                    cell.select(items,selected,"",next_gui_ui::SelectStyle::from_theme(&theme))
                                                } else {cell.autocomplete_placeholder(items,value,"",next_gui_ui::AutocompleteStyle::from_theme(&theme))}
                                            } else {cell.text_input_styled(value,"",name_s)}
                                        }).unwrap_or(z);
                                    }
                                }

                                c.text(format!("Selected column #{}", sel + 1));
                                c.row(row, |r| {
                                    r.text("Name");
                                    ov_col_name = r.text_input_styled(
                                        coldef.map(|c| c.name.as_str()).unwrap_or(""),
                                        "col",
                                        name_s,
                                    );
                                    r.text("Type");
                                    let types = state.column_type_names();
                                    let mut type_style = next_gui_ui::AutocompleteStyle::from_theme(&theme);
                                    type_style.width = (row.width - 340.0).clamp(140.0, 300.0);
                                    ov_col_type = r.autocomplete_placeholder(types,
                                        coldef.map(|c| c.type_name.as_str()).unwrap_or(""), "No type", type_style);
                                });
                                c.row(row, |r| {
                                    ov_pk = r.switch_styled("PK", coldef.map(|c| c.pk).unwrap_or(false), sw);
                                    r.tooltip("Autoincrement: one INTEGER primary key", |r| {
                                        ov_ai = r.switch_styled("AI", coldef.map(|c| c.auto_increment).unwrap_or(false), sw);
                                    });
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
                                c.row(row, |r| {
                                    r.text("Collation");
                                    let mut style = next_gui_ui::AutocompleteStyle::from_theme(&theme);
                                    style.width = 260.0;
                                    ov_collation = r.autocomplete_placeholder(state.column_collation_names(),
                                        coldef.map(|column| column.collation.as_str()).unwrap_or(""), "Default (BINARY)", style);
                                });
                                c.text("Check expression · SQL (optional)");
                                let mut check_style = area_s;
                                check_style.height = scroll.height.min(80.0);
                                ov_check = c.text_area(coldef.map(|column| column.check_expression.as_str()).unwrap_or(""),
                                    "e.g. length(name) > 0", check_style);
                                c.text("Foreign key · SQL (optional)");
                                ov_foreign_key = c.text_area(coldef.map(|column| column.foreign_key_clause.as_str()).unwrap_or(""),
                                    "REFERENCES parent(id) ON DELETE CASCADE", check_style);
                                c.row(row, |r| {
                                    r.text("Generation");
                                    ov_generation = r.select(["Ordinary", "VIRTUAL", "STORED"],
                                        Some(coldef.and_then(|column| column.generated.as_ref()).map_or(0, |generated| if generated.stored { 2 } else { 1 })), "Ordinary", type_sel);
                                });
                                ov_generated = c.text_area(coldef.and_then(|column| column.generated.as_ref()).map_or("", |generated| generated.expression.as_str()),
                                    "Generated expression, e.g. price * quantity", check_style);
                            });

                            ov_keys_scroll=tabs.scroll_page("Index Constraints",scroll, |c| {
                                    c.text(format!("Primary key and UNIQUE constraints · {}", db::qualified_name(&state.wizard_schema, &state.wizard_table_name)));
                                    c.row(row, |r| {
                                        ov_key_add_pk = r.tool_button("Add primary key"); ov_key_add_unique = r.tool_button("Add UNIQUE");
                                        ov_key_del = r.tool_button("Remove"); ov_key_up = r.tool_button("Up"); ov_key_down = r.tool_button("Down");
                                    });
                                    let mut grid=next_gui_ui::TableStyle::from_theme(&theme);grid.width=area_s.width;grid.height=144.0;
                                    ov_keys_table=c.table([("Columns",240.0),("Type",180.0),("Name",180.0),("SQL",620.0)].into_iter().map(|(name,width)|TableColumn::new(name,width)).collect(),table_index_rows(state),grid);
                                    if state.wizard_constraint_edit.as_ref().is_some_and(|edit| edit.page == 0) { ov_constraint_editor=constraint_cell_editor(c,ov_keys_table,state,&theme); }
                                    let key = state.wizard_index_constraints.get(state.wizard_key_sel);
                                    c.row(row, |r| {
                                        r.text("Name"); ov_key_name = r.text_input_styled(key.and_then(|key| key.name.as_deref()).unwrap_or(""), "Optional constraint name", wide_s);
                                    });
                                    c.row(row, |r| {
                                        r.text("On conflict"); let mut selector = type_sel; selector.width = 180.0;
                                        ov_key_conflict = r.select(crate::table_sql::PRIMARY_KEY_CONFLICTS.iter().map(|(label,_)| *label),
                                            crate::table_sql::PRIMARY_KEY_CONFLICTS.iter().position(|(_,action)| *action == key.and_then(|key| key.conflict)), "Default", selector);
                                    });
                                    c.text("Columns · SQL identifiers, COLLATE, ASC / DESC");
                                    let mut columns = area_s; columns.height = scroll.height.min(100.0);
                                    ov_key_columns = c.text_area(key.map_or("", |key| key.columns.as_str()), "e.g. customer_id ASC, region COLLATE NOCASE DESC", columns);
                                });

                            ov_fks_scroll=tabs.scroll_page("Foreign Keys",scroll, |c| {
                                    c.text(format!("Foreign keys · {}", db::qualified_name(&state.wizard_schema, &state.wizard_table_name)));
                                    c.row(row, |r| {
                                        ov_fk_add = r.tool_button("Add"); ov_fk_del = r.tool_button("Remove");
                                        ov_fk_up = r.tool_button("Up"); ov_fk_down = r.tool_button("Down");
                                    });
                                    let mut grid=next_gui_ui::TableStyle::from_theme(&theme);grid.width=area_s.width;grid.height=144.0;
                                    ov_fks_table=c.table([("Columns",240.0),("Name",180.0),("References",400.0),("SQL",620.0)].into_iter().map(|(name,width)|TableColumn::new(name,width)).collect(),table_foreign_key_rows(state),grid);
                                    if let Some(edit)=state.wizard_constraint_edit.as_ref().filter(|edit|edit.page==1) {
                                        if let Some(draft)=&edit.reference {
                                            let width=state.wizard_constraint_views[1].widths.get(2).copied().unwrap_or(400.0);
                                            reference_editor=crate::reference_editor::build(c,ov_fks_table,(edit.row,edit.column),width,reference_reset_width.max(64.0),draft,&theme);
                                            ov_constraint_editor=reference_editor.root;
                                        } else {ov_constraint_editor=constraint_cell_editor(c,ov_fks_table,state,&theme);}
                                    }
                                    let key = state.wizard_foreign_keys.get(state.wizard_foreign_key_sel);
                                    c.row(row, |r| {
                                        r.text("Name"); ov_fk_name = r.text_input_styled(key.and_then(|key| key.name.as_deref()).unwrap_or(""), "Optional constraint name", wide_s);
                                    });
                                    c.text("Local columns · SQL identifiers, in order");
                                    let mut columns = area_s; columns.height = scroll.height.min(56.0);
                                    ov_fk_columns = c.text_area(key.map_or("", |key| key.columns.as_str()), "e.g. customer_id, region_id", columns);
                                    c.text("References · SQL");
                                    let mut clause = area_s; clause.height = scroll.height.min(100.0);
                                    ov_fk_clause = c.text_area(key.map_or("", |key| key.clause.as_str()), "REFERENCES parent(id) ON UPDATE CASCADE ON DELETE SET NULL", clause);
                                });

                            ov_checks_scroll=tabs.scroll_page("Check Constraints",scroll, |c| {
                                    c.text(format!("CHECK constraints · {}", db::qualified_name(&state.wizard_schema, &state.wizard_table_name)));
                                    c.text(format!("Selected field: {}", state.wizard_cols.get(state.wizard_col_sel).map(|column|db::quote_ident(&column.name)).unwrap_or_else(||"(none)".into())));
                                    c.row(row, |r| {
                                        ov_check_add = r.tool_button("Add table");
                                        ov_check_add_column = r.tool_button("Add column");
                                        ov_check_del = r.tool_button("Remove");
                                        ov_check_up = r.tool_button("Up");
                                        ov_check_down = r.tool_button("Down");
                                    });
                                    let mut grid=next_gui_ui::TableStyle::from_theme(&theme);grid.width=area_s.width;grid.height=144.0;
                                    ov_checks_table=c.table([("Check",400.0),("Name",180.0),("SQL",620.0)].into_iter().map(|(name,width)|TableColumn::new(name,width)).collect(),table_check_rows(state),grid);
                                    if state.wizard_constraint_edit.as_ref().is_some_and(|edit| edit.page == 2) { ov_constraint_editor=constraint_cell_editor(c,ov_checks_table,state,&theme); }
                                    let check = state.wizard_checks.get(state.wizard_check_sel);
                                    c.row(row, |r| {
                                        r.text("Name");
                                        ov_check_name = r.text_input_styled(check.and_then(|check| check.name.as_deref()).unwrap_or(""), "Optional constraint name", wide_s);
                                    });
                                    c.text("CHECK expression · SQL");
                                    let mut expression = area_s;
                                    expression.height = scroll.height.min(100.0);
                                    ov_check_expr = c.text_area(check.map_or("", |check| check.expression.as_str()), "e.g. amount > 0 AND amount < 100", expression);
                                });

                            });
                            },|c| {
                                let mut preview_style = area_s;
                                preview_style.height=(split.height-split.sash_size)*(1.0-split.ratio);
                                ov_table_preview = c.text_area(state.table_wizard_sql().unwrap_or_else(|error| format!("-- {error}")), "SQL preview", preview_style);
                            });
                        }
                        OverlayKind::EditIndex => {
                            let mut scroll = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
                            scroll.width = ov_col.width - 16.0;
                            scroll.height = ov_col.height - 16.0;
                            scroll.padding = 0.0;
                            scroll.background = None;
                            ov_scroll = c.scroll_area(scroll, |c| {
                                c.row(row, |r| {
                                    r.text("Index name");
                                    ov_name = r.text_input_styled(&state.wizard_index_name, "name", name_s);
                                });
                                let tables = state.index_tables();
                                let tsel = tables.iter().position(|(schema, table)| *schema == state.wizard_schema && *table == state.wizard_index_table);
                                let labels: Vec<_> = tables.iter().map(|(schema, table)| db::qualified_name(schema, table)).collect();
                                c.row(row, |r| {
                                    r.text("Table");
                                    let mut selector = type_sel;
                                    selector.width = (row.width - 100.0).clamp(140.0, 440.0);
                                    ov_idx_table = r.select(
                                        if labels.is_empty() {
                                            vec!["(none)".into()]
                                        } else {
                                            labels
                                        },
                                        tsel,
                                        "table",
                                        selector,
                                    );
                                });
                                let mut key_row = row;
                                key_row.width = area_s.width;
                                key_row.height = 148.0;
                                let mut key_col = ColumnStyle::from_theme(&theme);
                                key_col.width = (area_s.width - row.gap) / 2.0;
                                key_col.height = key_row.height;
                                key_col.padding = 0.0;
                                key_col.gap = 4.0;
                                key_col.background = None;
                                let mut key_list = list_s;
                                key_list.width = key_col.width;
                                key_list.height = 120.0;
                                c.row(key_row, |r| {
                                    r.column(key_col, |c| {
                                        c.text("Available table columns");
                                        match state.index_wizard_column_details() {
                                            Ok(columns) => ov_idx_available = c.list(columns.iter().map(|(name, ty)| if ty.is_empty() { name.clone() } else { format!("{name}    {ty}") }), key_list),
                                            Err(error) => { c.text(format!("Columns unavailable: {error}")); }
                                        }
                                    });
                                    r.column(key_col, |c| {
                                        c.text("Ordered index keys");
                                        let keys = crate::index_sql::keys(&state.wizard_index_cols).unwrap_or_default();
                                        ov_list = c.list(keys.iter().enumerate().map(|(i, key)| format!("{}. {}", i + 1, key.split_whitespace().collect::<Vec<_>>().join(" "))), key_list);
                                    });
                                });
                                c.row(row, |r| {
                                    ov_btn_add = r.tool_button("Add column");
                                    ov_idx_expression = r.tool_button("Add expression");
                                    ov_btn_del = r.tool_button("Remove");
                                    ov_btn_up = r.tool_button("Up");
                                    ov_btn_down = r.tool_button("Down");
                                    ov_idx_edit_key = r.tool_button("Edit key");
                                });
                                c.row(row, |r| {
                                    r.text("Selected key order");
                                    let mut order_style = type_sel;
                                    order_style.width = 120.0;
                                    ov_idx_order = r.select(["ASC", "DESC"], state.index_wizard_key_order().ok().map(usize::from), "select key", order_style);
                                });
                                c.text("Columns / expressions · SQL");
                                let mut sql_area = area_s;
                                sql_area.height = 84.0;
                                ov_idx_cols = c.text_area(&state.wizard_index_cols, "column or expression, …", sql_area);
                                c.text("Partial clause · condition after WHERE (optional)");
                                sql_area.height = 64.0;
                                ov_idx_where = c.text_area(&state.wizard_index_where, "e.g. revenue > 0", sql_area);
                                ov_idx_unique = c.switch_styled("Unique", state.wizard_index_unique, sw);
                                c.text("SQL preview");
                                sql_area.height = 80.0;
                                ov_idx_preview = c.text_area(state.index_wizard_sql(), "", sql_area);
                            });
                        }
                        OverlayKind::ImportCsv | OverlayKind::PasteCsv => {
                            let body_w = ov_col.width - 16.0;
                            let body_h = ov_col.height - 16.0;
                            let left_w = if shell_w >= 1000.0 { 380.0 } else { 320.0 };
                            let preview_w = (body_w - left_w - 12.0).max(200.0);
                            let mut body = row; body.width = body_w; body.height = body_h; body.cross_align = CrossAlign::Start; body.gap = 12.0;
                            c.row(body, |body| {
                                let mut options = next_gui_ui::ScrollAreaStyle::from_theme(&theme); options.width = left_w; options.height = body_h; options.padding = 8.0;
                                ov_csv_scroll = body.scroll_area(options, |c| {
                                    csv_files = crate::csv_files::build_files(c,state,&theme,left_w-16.0);
                                    let mut option_row = row; option_row.width = left_w - 16.0;
                                    let mut input = name_s; input.width = left_w - 82.0;
                                    if kind == OverlayKind::ImportCsv {
                                        let filename = std::path::Path::new(&state.csv_path).file_name().and_then(|name| name.to_str()).unwrap_or(&state.csv_path);
                                        c.text_styled(filename, theme.colors.text_secondary, 12.0);
                                    } else {
                                        c.text("Clipboard text"); let mut source = area_s; source.width = left_w - 16.0; source.height = 110.0;
                                        ov_csv_paste = c.text_area(&state.csv_paste, "Paste CSV…", source);
                                    }
                                    c.row(option_row, |r| { r.text("Table"); ov_csv_table = r.text_input_styled(&state.csv_table, "table", input); });
                                    c.row(option_row, |r| {
                                        ov_csv_header = r.switch_styled("Header row", state.csv_has_header, sw);
                                        ov_csv_trim = r.switch_styled("Trim fields", state.csv_import_settings.trim_fields, sw);
                                    });
                                    ov_csv_local = c.switch_styled("Use local number conventions", state.csv_import_settings.local_conventions, sw);
                                    let mut select = type_sel; select.width = 180.0;
                                    c.row(option_row, |r| { r.text("Separator"); ov_csv_delim = r.select(["Comma ,", "Semicolon ;", "Tab", "Pipe |", "Other character", "Other code point"], Some(state.csv_delim_idx), "Separator", select); });
                                    if state.csv_delim_idx >= 4 { ov_csv_custom_delimiter = c.text_input_styled(&state.csv_custom_delimiter, if state.csv_delim_idx == 5 { "e.g. 31 or U+001F" } else { "One character" }, input); }
                                    c.row(option_row, |r| { r.text("Quote"); ov_csv_quote = r.select(["Double quote", "Single quote", "None", "Other character", "Other code point"], Some(state.csv_quote_idx), "Quote", select); });
                                    if state.csv_quote_idx >= 3 { ov_csv_custom_quote = c.text_input_styled(&state.csv_custom_quote, if state.csv_quote_idx == 4 { "e.g. 34 or U+0022" } else { "One character" }, input); }
                                    if kind == OverlayKind::ImportCsv {
                                        c.row(option_row, |r| { r.text("Encoding"); ov_csv_encoding = r.select(["UTF-8", "UTF-16", "ISO-8859-1", "Other"], Some(state.csv_encoding_idx), "Encoding", select); });
                                        if state.csv_encoding_idx == 3 {
                                            let mut custom = next_gui_ui::AutocompleteStyle::from_theme(&theme); custom.width = left_w - 16.0;
                                            ov_csv_custom_encoding = c.autocomplete_placeholder(crate::csv_encoding::NAMES.iter().copied(), &state.csv_custom_encoding, "e.g. Shift_JIS", custom);
                                        }
                                    }
                                    ov_csv_advanced = c.tool_button(if state.csv_advanced { "Hide Advanced" } else { "Advanced" });
                                    if state.csv_advanced {
                                        ov_csv_types = c.switch_styled("Disable data type detection", !state.csv_import_settings.detect_types, sw);
                                        ov_csv_defaults = c.switch_styled("Ignore default values", state.csv_import_settings.ignore_defaults, sw);
                                        ov_csv_missing = c.switch_styled("Fail on missing values", state.csv_import_settings.fail_on_missing, sw);
                                        c.text("On conflict");
                                        ov_csv_conflict = c.select(["Abort import", "Ignore row", "Replace row"], Some(match state.csv_import_settings.conflict { crate::db::CsvConflict::Abort => 0, crate::db::CsvConflict::Ignore => 1, crate::db::CsvConflict::Replace => 2 }), "On conflict", select);
                                    }
                                });
                                let mut preview = ov_col; preview.width = preview_w; preview.height = body_h; preview.padding = 0.0;
                                body.column(preview, |c| {
                                    let mut header = row; header.width = preview_w;
                                    c.row(header, |r| { r.text("Preview · first 20 records"); ov_csv_refresh = r.tool_button("Refresh"); });
                                    match state.csv_preview.as_ref() {
                                        Some(Ok(data)) => {
                                            let mut grid = TableStyle::from_theme(&theme); grid.width = preview_w; grid.height = (body_h - 82.0).max(80.0);
                                            let mut columns = vec![TableColumn::new("#", 44.0)];
                                            columns.extend(data.columns.iter().map(|name| TableColumn::new(name, ((preview_w - 44.0 - grid.padding * 2.0 - 1.0) / data.columns.len() as f32).clamp(110.0, 220.0))));
                                            let rows = data.rows.iter().enumerate().map(|(index, row)| std::iter::once((index + 1).to_string()).chain((0..data.columns.len()).map(|column| row.get(column).map(|value| value.chars().take(1024).collect::<String>()).unwrap_or_default())).collect()).collect();
                                            ov_csv_preview = c.table(columns, rows, grid);
                                            let types = data.columns.iter().zip(&data.types).map(|(name, kind)| format!("{name}: {}", if kind.is_empty() { "untyped" } else { kind })).collect::<Vec<_>>().join(" · ");
                                            c.text_styled(types, theme.colors.text_secondary, 12.0);
                                        }
                                        Some(Err(error)) => {
                                            c.text("Preview unavailable"); let mut message = area_s; message.width = preview_w; message.height = (body_h - 76.0).max(80.0);
                                            ov_csv_preview_error = c.text_area(error, "", message);
                                        }
                                        None => { c.text("Choose CSV data to preview."); }
                                    }
                                });
                            });
                        }
                        OverlayKind::FindReplace => {
                            let mut scroll = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
                            scroll.width = shell_w - 64.0;
                            scroll.height = ov_col.height - 16.0;
                            scroll.padding = 0.0;
                            c.scroll_area(scroll, |c| {
                                c.scroll_row(row, |r| {
                                    r.text("Find");
                                    ov_find = r.text_input_styled(&state.find_text, "find", wide_s);
                                });
                                c.scroll_row(row, |r| {
                                    r.text("Replace");
                                    ov_replace = r.text_input_styled(&state.replace_text, "replace", wide_s);
                                });
                                let flags = state.find_search.options.flags();
                                for indices in [[0, 1, 5], [2, 3, 4]] {
                                    c.scroll_row(row, |r| {
                                        for i in indices {
                                            ov_find_options[i] = r.switch_styled(
                                                ["Match case", "Whole words", "Wrap around", "Backwards", "Selection only", "Regular expression"][i], flags[i], sw);
                                        }
                                    });
                                }
                                c.scroll_row(row, |r| {
                                    ov_btn_find = r.tool_button("Find Next");
                                    ov_btn_find_all = r.tool_button("Find All");
                                    ov_btn_repl = r.tool_button("Replace");
                                    ov_btn_repl_all = r.tool_button("Replace All");
                                });
                                c.text(if state.find_message.is_empty() { "F3: find next · Shift+F3: reverse direction" } else { &state.find_message });
                                c.text_styled(r"Regex replacements: \0–\9 captures · \n \r \t escapes", theme.colors.text_secondary, 11.0);
                                let mut editor_style = area_s;
                                editor_style.width = shell_w - 80.0;
                                editor_style.height = (scroll.height - 242.0).max(100.0);
                                sql_area = c.text_area(state.sql_editor(), "SQL…", editor_style);
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
                        OverlayKind::Cipher => {
                            c.text("SQLCipher passphrase + page size + KDF iterations");
                            c.row(row, |r| {
                                r.text("Passphrase");
                                ov_cipher_pass = r.text_input_styled(&state.cipher_passphrase, "key", wide_s);
                            });
                            c.row(row, |r| {
                                r.text("Page size");
                                ov_cipher_page = r.text_input_styled(&state.cipher_page_size, "4096", name_s);
                                r.text("KDF iter");
                                ov_cipher_kdf = r.text_input_styled(&state.cipher_kdf_iter, "256000", name_s);
                            });
                            c.text(if state.cipher_active {
                                "Status: current database encrypted"
                            } else {
                                "Status: not encrypted / key not applied"
                            });
                            c.text(if state.cipher_linked {
                                "Open: unlock with this key. Tools: export to a new encrypted file."
                            } else {
                                "SQLCipher not linked — rebuild with bundled-sqlcipher-vendored-openssl."
                            });
                        }
                        OverlayKind::Preferences => {
                            c.row(row, |r| {
                                r.text("Font size");
                                ov_prefs_font = r.text_input_styled(&state.prefs_font_size, "13", name_s);
                            });
                            ov_prefs_theme = c.switch_styled("Default dark theme", state.prefs_theme_dark, sw);
                            ov_prefs_confirm = c.switch_styled("Confirm drops", state.prefs_confirm_drops, sw);
                            ov_prefs_completion = c.switch_styled("SQL auto completion", state.prefs_sql_completion, sw);
                            ov_prefs_upper_keywords = c.switch_styled("Uppercase SQL keywords", state.prefs_sql_upper_keywords, sw);
                            c.row(row, |r| {
                                r.text("CSV delimiter");
                                ov_prefs_delim = r.text_input_styled(&state.prefs_csv_delim, ",", name_s);
                            });
                            c.row(row, |r| {
                                r.text("Default frozen cols");
                                ov_frozen = r.text_input_styled(format!("{}", state.prefs_frozen_cols), "0", name_s);
                            });
                        }
                        OverlayKind::ExportSql => {
                            c.text("Choose what to include in the SQL dump");
                            ov_export_struct = c.switch_styled("Export structure", state.export_structure, sw);
                            ov_export_data = c.switch_styled("Export data", state.export_data, sw);
                            ov_export_drop = c.switch_styled("DROP IF EXISTS before create", state.export_drop, sw);
                            ov_export_fk = c.switch_styled("Keep/check foreign keys", state.export_keep_fk, sw);
                            c.text("OK writes via Save dialog after options are set.");
                        }
                        OverlayKind::LoadExtension => {
                            c.text("Load a SQLite extension (.so / .dylib / .dll)");
                            c.row(row, |r| {
                                r.text("Path");
                                ov_ext_path = r.text_input_styled(&state.ext_path, "/path/to/ext.so", wide_s);
                            });
                            c.row(row, |r| {
                                r.text("Entry (optional)");
                                ov_ext_entry = r.text_input_styled(&state.ext_entry, "sqlite3_extension_init", wide_s);
                            });
                        }
                        OverlayKind::About => {
                            c.text("DB Browser for SQLite — next-gui e28");
                            c.text(format!(
                                "SQLCipher: {}",
                                if state.cipher_linked { "linked" } else { "not linked" }
                            ));
                            c.text("Parity target: 100% DB4S feature + UI");
                            c.text("https://sqlitebrowser.org/");
                        }
                        OverlayKind::CondFormat => {
                            if let Some(table) = &state.browse().table {
                                c.text(format!("Table: {}", db::qualified_name(&state.browse().schema, table)));
                            }
                            c.text("Rule: when cell matches, fill cell background");
                            c.row(row, |r| {
                                r.text("Column");
                                ov_cond_col = r.text_input_styled(&state.cond_col, "col", name_s);
                            });
                            c.row(row, |r| {
                                r.text("Op");
                                ov_cond_op = r.select(
                                    vec![String::from("=="), String::from("!="), String::from("contains"), String::from(">"), String::from("<")],
                                    Some(state.cond_op_idx),
                                    "op",
                                    type_sel,
                                );
                                r.text("Value");
                                ov_cond_val = r.text_input_styled(&state.cond_value, "value", name_s);
                            });
                            c.row(row, |r| {
                                r.text("Color #RRGGBB");
                                ov_cond_color = r.text_input_styled(&state.cond_color, "E67E22", name_s);
                            });
                            c.text(format!("Rules for this table: {}", state.cond_rules.iter().filter(|rule| rule.schema == state.browse().schema && rule.table == state.browse().table).count()));
                        }
                    }
                });
            });
            ov_btn_ok = ok_id;
            ov_btn_cancel = cancel_id;
        } else {
            // —— Main + docks ——
            let browse_tables = state.browse_tables();
            let browse_opts: Vec<String> = if browse_tables.is_empty() {
                vec!["(no tables)".into()]
            } else {
                browse_tables.iter().map(|(schema, name)| if *schema == "main" { (*name).into() } else { db::qualified_name(schema, name) }).collect()
            };
            let browse_sel = state
                .browse()
                .table
                .as_ref()
                .and_then(|t| browse_tables.iter().position(|(schema, name)| *schema == state.browse().schema && *name == t));
            let chart_sel = state
                .chart_table
                .as_ref()
                .and_then(|t| browse_tables.iter().position(|(schema, name)| *schema == state.chart_schema && *name == t));
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
            let log_text = state.sql_log.text(crate::sql_log::Source::from_filter(state.sql_log_filter));
            let schema_items: Vec<String> = if state.schema.is_empty() {
                vec!["(no objects)".into()]
            } else {
                state
                    .schema
                    .iter()
                    .map(|o| format!("{:?} · {}.{}", o.kind, db::quote_ident(&o.schema), db::quote_ident(&o.name)))
                    .collect()
            };
            let chart_title = format!(
                "{} · {} ({} samples)",
                state.chart_table.as_ref().map(|name| db::qualified_name(&state.chart_schema, name)).unwrap_or_else(|| "—".into()),
                state.chart_column,
                state.chart_points.len()
            );
            let chart_tiles = vec![ChartTile::trend(
                chart_title.clone(),
                state.chart_points.clone(),
                TrendStyle::from_theme(&theme),
            )];

            let mut body_split = SplitterStyle::from_theme(&theme);
            body_split.sash_color = theme.colors.sash;
            body_split.width = shell_w - 8.0;
            body_split.height = content_h;
            body_split.axis = Axis::Vertical;
            body_split.ratio = if docks_on { main_frac } else { 0.98 };

            let mut page_scroll_style = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
            page_scroll_style.width = main_w - 8.0;
            page_scroll_style.height = tab_page_h - 8.0;
            page_scroll_style.padding = 0.0;
            let mut tabs_style = TabsStyle::from_theme(&theme);
            tabs_style.width = main_w - 4.0;
            tabs_style.height = content_h - 4.0;
            tabs_style.content_bg = theme.colors.content_bg;
            tabs_style.strip_bg = theme.colors.strip_bg;
            tabs_style.active_fg = theme.colors.text_primary;

            let mut struct_tb = RowStyle::from_theme(&theme);
            struct_tb.width = main_w - 24.0;
            struct_tb.height = 34.0;
            struct_tb.padding = 0.0;
            struct_tb.cross_align = CrossAlign::Center;

            let mut struct_split = SplitterStyle::from_theme(&theme);
            struct_split.width = main_w - 24.0;
            struct_split.height = (tab_page_h - 48.0).max(280.0);
            struct_split.axis = Axis::Vertical;
            struct_split.ratio = state.structure_ratio.clamp(0.25, 0.65);

            let tree_w = (main_w - 24.0) * struct_split.ratio - 8.0;
            let right_w = (main_w - 24.0) * (1.0 - struct_split.ratio) - 8.0;
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
            browse_table_style.drag_select = true;
            browse_table_style.width = main_w - 24.0;
            browse_table_style.height = (tab_page_h - 268.0).max(160.0);

            let mut sql_tb = RowStyle::from_theme(&theme);
            sql_tb.width = main_w - 24.0;
            sql_tb.height = 34.0;
            sql_tb.padding = 0.0;
            sql_tb.cross_align = CrossAlign::Center;
            let mut sql_area_style = TextAreaStyle::from_theme(&theme);
            sql_area_style.width = main_w - 24.0;
            sql_area_style.height = 120.0;
            let mut sql_table_style = TableStyle::from_theme(&theme);
            sql_table_style.width = main_w - 24.0;
            let sql_controls_h = sql_area_style.height + 2.0 * tool_row.height
                + AlertStyle::from_theme(&theme).height + 4.0 * page_scroll_style.gap
                + if state.toolbar_sql { sql_tb.height + page_scroll_style.gap } else { 0.0 };
            sql_table_style.height = (page_scroll_style.height - sql_controls_h).max(120.0);

            let mut dock_tabs_style = TabsStyle::from_theme(&theme);
            dock_tabs_style.width = dock_w.max(220.0);
            dock_tabs_style.scrollable = true;
            dock_tabs_style.height = content_h - 4.0;
            dock_tabs_style.content_bg = theme.colors.content_bg;
            dock_tabs_style.strip_bg = theme.colors.strip_bg;
            dock_tabs_style.active_fg = theme.colors.text_primary;

            let mut dock_col = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
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
            chart_style.padding = 10.0;
            chart_style.gap = 8.0;

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

            let mut pragma_col = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
            pragma_col.width = main_w - 24.0;
            pragma_col.height = (tab_page_h - 100.0).max(200.0);
            pragma_col.padding = 4.0;
            pragma_col.gap = 4.0;
            pragma_col.cross_align = CrossAlign::Start;
            pragma_col.background = Some(theme.colors.content_bg);

            let sw = SwitchStyle::from_theme(&theme);
            let mut prag_sel = SelectStyle::from_theme(&theme);
            prag_sel.width = 160.0;
            let mut prag_inp = TextInputStyle::from_theme(&theme);
            prag_inp.width = 180.0;

            let main_pane = |left: &mut next_gui_ui::FlexBuilder<'_>| {
                    tabs = left.tabs(tabs_style, |tb| {
                        tb.page(MainTab::Structure.title(), |p| {
                            if state.tab != MainTab::Structure {
                                p.text_styled("", theme.colors.content_bg, 1.0);
                                return;
                            }
                            page_scroll = p.scroll_area(page_scroll_style, |p| {
                            p.scroll_row(struct_tb, |tb| {
                                btn_struct_new_table = tb.tool_button("New Table");
                                btn_struct_new_index = tb.tool_button("New Index");
                                btn_struct_modify = tb.tool_button("Modify");
                                btn_struct_delete = tb.tool_button("Delete");
                                btn_struct_refresh = tb.tool_button("Refresh");
                            });
                            structure_splitter = p.splitter(
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
                                            c.text(format!("Selected: {:?} {}.{}", sel.kind, db::quote_ident(&sel.schema), db::quote_ident(&sel.name)));
                                            if sel.schema != "main" {
                                                c.text("Definition preview · use Execute SQL for qualified DDL");
                                            }
                                        } else {
                                            c.text("Selected: (none) — use New Table / New Index wizards");
                                        }
                                    });
                                },
                            );
                            });
                        });

                        tb.page(MainTab::Browse.title(), |p| {
                            if state.tab != MainTab::Browse {
                                p.text_styled("", theme.colors.content_bg, 1.0);
                                return;
                            }
                            page_scroll = p.scroll_area(page_scroll_style, |p| {
                            // Browse multi-tabs strip
                            let titles: Vec<String> =
                                state.browse_tabs.iter().map(|t| t.title.clone()).collect();
                            p.scroll_row(tool_row, |row| {
                                btn_browse_new_tab = row.tool_button("+ Tab");
                                for (i, title) in titles.iter().enumerate() {
                                    let label = if i == state.browse_tab_idx {
                                        format!("[{title}]")
                                    } else {
                                        title.clone()
                                    };
                                    browse_tab_btns.push(row.tool_button(label));
                                }
                                btn_browse_close_tab = row.tool_button("Close Tab");
                            });
                            browse_tabs = WidgetId(0);
                            p.scroll_row(tool_row, |row| {
                                row.text("Table");
                                browse_select =
                                    row.select(browse_opts_a.clone(), browse_sel, "table…", sel_style);
                                btn_add_row = row.tool_button("New Record");
                                btn_del_row = row.tool_button("Delete Record");
                            });
                            p.scroll_row(tool_row, |row| {
                                browse_global_filter = row.text_input_styled(
                                    state.browse().global_filters.join(" "),
                                    "Filter in any column",
                                    filter_style,
                                );
                                browse_filter = row.text_input_styled(
                                    &state.browse().filter,
                                    "WHERE …",
                                    filter_style,
                                );
                                btn_browse_go = row.tool_button("Apply");
                                btn_browse_clear_filters = row.tool_button("Clear Filters");
                                btn_browse_clear_sort = row.tool_button("Clear Sort");
                            });
                            p.scroll_row(tool_row, |row| {
                                btn_form_view = row.tool_button(match state.browse_view {
                                    crate::state::BrowseViewMode::Table => "Form View",
                                    crate::state::BrowseViewMode::Form => "Grid View",
                                });
                                btn_freeze = row.tool_button(if state.browse_frozen_cols() == 0 {
                                    "Freeze columns".into()
                                } else { format!("Unfreeze {} columns", state.browse_frozen_cols()) });
                                btn_cond_fmt = row.tool_button("Cond Format…");
                                btn_fk_nav = row.tool_button("FK →");
                            });
                            // Per-column filters
                            if !col_names.is_empty() && state.browse_view == crate::state::BrowseViewMode::Form {
                                p.scroll_row(filt_row, |row| {
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
                            match state.browse_view {
                                crate::state::BrowseViewMode::Table => {
                                    let mut filtered_style = browse_table_style;
                                    filtered_style.height += col_filt_style.height + 4.0;
                                    (browse_table, browse_col_filters) = p.filtered_table(
                                        browse_cols.clone(),
                                        browse_rows.clone(),
                                        &col_filter_vals,
                                        filtered_style,
                                        col_filt_style,
                                    );
                                }
                                crate::state::BrowseViewMode::Form => {
                                    let b = state.browse();
                                    let row_i = b.sel_row.unwrap_or(0);
                                    let res = b.result.as_ref();
                                    p.scroll_row(tool_row, |row| {
                                        btn_form_prev = row.tool_button("◀ Prev");
                                        btn_form_next = row.tool_button("Next ▶");
                                        let n = res.map(|r| r.rows.len()).unwrap_or(0);
                                        let total = b.total_rows.unwrap_or(n);
                                        let global_row = b.offset.saturating_add(row_i);
                                        row.text(format!(
                                            "Record {} / {}",
                                            if n == 0 { 0 } else { global_row + 1 },
                                            total
                                        ));
                                    });
                                    let mut form_scroll = next_gui_ui::ScrollAreaStyle::from_theme(&theme);
                                    form_scroll.width = main_w - 24.0;
                                    form_scroll.height = (tab_page_h - 232.0).max(120.0);
                                    p.scroll_area(form_scroll, |p| {
                                    if let Some(res) = res {
                                        let cells = res.rows.get(row_i);
                                        for (ci, cname) in res.columns.iter().enumerate() {
                                            let val = cells
                                                .and_then(|r| r.get(ci))
                                                .cloned()
                                                .unwrap_or_default();
                                            p.scroll_row(tool_row, |row| {
                                                row.text(cname.as_str());
                                                let id = row.tool_button(format!("{}: {}", cname, val));
                                                form_fields.push(id);
                                            });
                                        }
                                    } else {
                                        p.text("No rows — Apply a table first");
                                    }
                                    });
                                    browse_table = WidgetId(0);
                                }
                            }
                            p.scroll_row(tool_row, |row| {
                                btn_record_first = row.tool_button("|◀ First");
                                btn_record_prev = row.tool_button("◀ Previous");
                                row.text("Go to record");
                                let number_style = TextInputStyle { width: 84.0, ..col_filt_style };
                                let value = &state.browse().goto_record;
                                record_number = row.text_input_styled(value, "Record #", number_style);
                                btn_record_go = row.tool_button("Go");
                                btn_record_next = row.tool_button("Next ▶");
                                btn_record_last = row.tool_button("Last ▶|");
                                let tab = state.browse();
                                row.text(format!("{} / {} records", tab.sel_row.map_or(0, |r| tab.offset + r + 1), tab.total_rows.unwrap_or(0)));
                            });
                            p.text_styled("Ctrl/Cmd+click a header to add a sort column.", theme.colors.text_secondary, 12.0);
                            });
                        });

                        tb.page(MainTab::Pragmas.title(), |p| {
                            if state.tab != MainTab::Pragmas {
                                p.text_styled("", theme.colors.content_bg, 1.0);
                                return;
                            }
                            page_scroll = p.scroll_area(page_scroll_style, |p| {
                            p.text("Database preferences (PRAGMA). Edit values then Save Pragmas.");
                            p.scroll_area(pragma_col, |c| {
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
                            p.scroll_row(tool_row, |row| {
                                btn_pragma_apply = row.tool_button("Save Pragmas");
                            });
                            });
                        });

                        tb.page(MainTab::ExecuteSql.title(), |p| {
                            if state.tab != MainTab::ExecuteSql {
                                p.text_styled("", theme.colors.content_bg, 1.0);
                                return;
                            }
                            page_scroll = p.scroll_area(page_scroll_style, |p| {
                            if state.toolbar_sql {
                                p.scroll_row(sql_tb, |tb| {
                                    btn_sql_new_tab = tb.tool_button("New Tab");
                                    btn_sql_open = tb.tool_button("Open SQL");
                                    btn_sql_save = tb.tool_button("Save SQL");
                                    btn_sql_save_as = tb.tool_button("Save As…");
                                    btn_sql_run = tb.tool_button_styled("Execute", theme.colors.accent, theme.colors.on_accent);
                                    btn_sql_line = tb.tool_button("Execute Line");
                                    btn_sql_stop = tb.tool_button("Stop");
                                    btn_sql_find = tb.tool_button("Find");
                                    tb.tooltip("Toggle line comments (Ctrl/Cmd+/)", |tb| {
                                        btn_sql_comment = tb.tool_button("Comment");
                                    });

                                    tb.spacer(8.0, 1.0);
                                    tb.text_styled(
                                        format!("SQL {}", state.sql_tab_idx + 1),
                                        theme.colors.text_secondary,
                                        12.0,
                                    );
                                });
                            }
                            // SQL editor tabs as buttons
                            p.scroll_row(tool_row, |row| {
                                for i in 0..state.sql_tabs.len() {
                                    let label = state.sql_tab_label(i);
                                    sql_tab_btns.push(row.tool_button(label));
                                }
                                btn_sql_left = row.tool_button("←");
                                btn_sql_right = row.tool_button("→");
                                btn_sql_close = row.tool_button("Close Tab");
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
                            let mut alert_s = AlertStyle::from_theme(&theme);
                            alert_s.width = main_w - 24.0;
                            sql_alert = p.alert_styled(tone, msg, false, alert_s);
                            p.scroll_row(tool_row, |row| {
                                btn_sql_csv = row.tool_button("Results → CSV");
                                btn_sql_json = row.tool_button("Results → JSON");
                                let mut view_style = TextInputStyle::from_theme(&theme);
                                view_style.width = 160.0;
                                sql_view_name = row.text_input_styled(&state.sql_view_name, "view name", view_style);
                                btn_sql_view = row.tool_button("Save as VIEW");
                            });
                            sql_table = p.table(sql_cols.clone(), sql_rows.clone(), sql_table_style);
                            });
                        });
                    });
                };
            let dock_pane = |right: &mut next_gui_ui::FlexBuilder<'_>| {
                    dock_tabs = right.tabs(dock_tabs_style, |tb| {
                        for dock in &visible_docks {
                            match *dock {
                                DockKind::EditCell => {
                                    tb.page(DockKind::EditCell.title(), |p| {
                                        dock_scroll = p.scroll_area(dock_col, |c| {
                                            c.text("Column");
                                            edit_col = c.text_input_styled(
                                                &state.edit_col,
                                                "column",
                                                edit_col_style,
                                            );
                                            c.row(tool_row, |r| {
                                                btn_edit_text = r.tool_button("Text");
                                                btn_edit_hex = r.tool_button("Hex");
                                                btn_edit_json = r.tool_button("JSON");
                                                btn_edit_image = r.tool_button("Image");
                                            });
                                            if generated_cell { c.text("Generated · read-only"); }
                                            else { edit_type = c.select(
                                                ["NULL", "INTEGER", "REAL", "TEXT", "BLOB"].map(String::from).to_vec(),
                                                Some(state.edit_type), "Storage type", sel_style,
                                            ); }
                                            c.text("Value");
                                            let mut cell_style = TextAreaStyle::from_theme(&theme);
                                            cell_style.width = edit_val_style.width;
                                            cell_style.height = 90.0;
                                            edit_value = c.text_area(&state.edit_value, "value", cell_style);
                                            c.row(tool_row, |r| {
                                                btn_edit_cell = r.tool_button_styled("Apply", theme.colors.accent, theme.colors.on_accent);
                                                btn_edit_discard = r.tool_button("Discard Draft");
                                            });
                                            let mode = match state.edit_mode {
                                                crate::state::EditCellMode::Text => "Text",
                                                crate::state::EditCellMode::Hex => "Hex",
                                                crate::state::EditCellMode::Json => "JSON",
                                                crate::state::EditCellMode::Image => "Image",
                                            };
                                            edit_meta = c.text(format!(
                                                "Mode {mode} · {} bytes{}",
                                                state.edit_binary_len,
                                                if state.edit_image_kind.is_empty() {
                                                    String::new()
                                                } else {
                                                    format!(" · {}", state.edit_image_kind)
                                                }
                                            ));
                                            if let Some((w, h, px)) = state.edit_image_rgba.as_ref() {
                                                let mut istyle = ImageStyle::from_theme(&theme);
                                                istyle.width = (dock_w - 40.0).max(1.0);
                                                istyle.height = 200.0;
                                                c.row(tool_row, |r| {
                                                    image_fit = r.tool_button("Fit");
                                                    image_actual = r.tool_button("1:1");
                                                    image_in = r.tool_button("+");
                                                    image_out = r.tool_button("−");
                                                });
                                                image_export = c.tool_button("Export Image…");
                                                edit_image = c.image_rgba_caption(
                                                    *w,
                                                    *h,
                                                    px,
                                                    format!("{}×{}", w, h),
                                                    istyle,
                                                );
                                            } else if matches!(
                                                state.edit_mode,
                                                crate::state::EditCellMode::Image
                                            ) {
                                                let mut istyle = ImageStyle::from_theme(&theme);
                                                istyle.width = 160.0;
                                                istyle.height = 48.0;
                                                edit_image = c.image_placeholder(
                                                    "No preview available",
                                                    istyle,
                                                );
                                            }
                                            if let Some(row) = state.browse().sel_row {
                                                c.text(format!("Record {} of {}", state.browse().offset + row + 1, state.browse().total_rows.unwrap_or(0)));
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
                                        dock_scroll = p.scroll_area(dock_col, |c| {
                                            c.scroll_row(dock_row, |row| {
                                                let mut filter_style = sel_style;
                                                filter_style.width = 118.0;
                                                log_filter = row.select(["User", "Application", "Error Log", "All"], Some(state.sql_log_filter), "source", filter_style);
                                                log_clear = row.tool_button("Clear");
                                                log_export = row.tool_button("Export…");
                                            });
                                            let (entries, evicted) = state.sql_log.snapshot(crate::sql_log::Source::from_filter(state.sql_log_filter));
                                            c.text(format!("{} retained · {evicted} evicted", entries.len()));
                                            let mut style = TextAreaStyle::from_theme(&theme);
                                            style.width = (dock_w - 40.0).max(160.0);
                                            style.height = (dock_col.height - 110.0).max(140.0);
                                            log_area = c.text_area(&log_text, "SQL log", style);
                                            c.text("Select text · Cmd/Ctrl+C to copy");
                                        });
                                    });
                                }
                                DockKind::Plot => {
                                    tb.page(DockKind::Plot.title(), |p| {
                                        dock_scroll = p.scroll_area(dock_col, |c| {
                                            c.row(dock_row, |row| {
                                                let mut table_style = sel_style;
                                                table_style.width = (dock_w - 52.0).max(160.0);
                                                chart_select = row.select(
                                                    browse_opts_b.clone(),
                                                    chart_sel,
                                                    "table…",
                                                    table_style,
                                                );
                                            });
                                            c.row(dock_row, |row| {
                                                chart_col = row.text_input_styled(
                                                    &state.chart_column,
                                                    "col",
                                                    chart_col_style,
                                                );
                                                btn_chart = row.tool_button("Plot");
                                            });
                                            c.tooltip(chart_title.clone(), |t| { chart = t.chart(chart_tiles.clone(), chart_style); });
                                        });
                                    });
                                }
                                DockKind::Remote => {
                                    tb.page(DockKind::Remote.title(), |p| {
                                        dock_scroll = p.scroll_area(dock_col, |c| {
                                            c.text("Remote / connection list");
                                            c.text_styled(
                                                "DB4S cloud Remote is not used here — local paths only.",
                                                theme.colors.text_secondary,
                                                11.0,
                                            );
                                            let items = if state.remote_paths.is_empty() {
                                                vec!["(no paths — add below)".into()]
                                            } else {
                                                state.remote_paths.clone()
                                            };
                                            remote_list = c.list(items, list_style);
                                            c.row(tool_row, |r| {
                                                remote_draft = r.text_input_styled(
                                                    &state.remote_draft,
                                                    "path or file URL",
                                                    edit_val_style,
                                                );
                                            });
                                            c.row(tool_row, |r| {
                                                btn_remote_add = r.tool_button("Add");
                                                btn_remote_open = r.tool_button("Open");
                                            });
                                        });
                                    });
                                }

                            }
                        }
                    });
                };
            if docks_on {
                body_splitter = col.splitter(body_split, main_pane, dock_pane);
            } else {
                let mut full_width = ColumnStyle::from_theme(&theme);
                full_width.width = shell_w - 8.0;
                full_width.height = content_h;
                full_width.padding = 0.0;
                full_width.gap = 0.0;
                full_width.background = None;
                col.column(full_width, main_pane);
            }
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
    let status_fg = theme.colors.text_secondary;
    ui.status_bar(status_style, |bar| {
        status = bar.text_styled(status_text(state), status_fg, 12.0);
    });

    if ov_csv_preview_error.0 != 0 { ui.set_text_area_read_only(ov_csv_preview_error, true); ui.set_accessible_name(ov_csv_preview_error, "CSV preview error"); }
    if ov_csv_custom_delimiter.0 != 0 { ui.set_accessible_name(ov_csv_custom_delimiter, "Custom CSV separator"); }
    if ov_csv_custom_quote.0 != 0 { ui.set_accessible_name(ov_csv_custom_quote, "Custom CSV quote"); }
    if ov_csv_preview.0 != 0 { ui.set_accessible_name(ov_csv_preview, "CSV data preview"); }
    if ov_csv_scroll.0 != 0 { ui.set_accessible_name(ov_csv_scroll, "CSV import options"); }
    if ov_csv_encoding.0!=0 {ui.set_accessible_name(ov_csv_encoding,"CSV file encoding");}
    if ov_csv_custom_encoding.0!=0 {ui.set_accessible_name(ov_csv_custom_encoding,"Custom CSV encoding");}
    if ov_cipher_pass.0 != 0 {
        ui.set_text_input_secret(ov_cipher_pass, true);
    }
    if ov_fields_table.0 != 0 {
        ui.set_table_selected_row(ov_fields_table,Some(state.wizard_col_sel));
        ui.set_table_selected_col(ov_fields_table,state.wizard_field_column);
        ui.set_table_frozen_cols(ov_fields_table,1);
        ui.set_table_check_cells(ov_fields_table,table_field_checks(state));
        ui.set_table_dropdown_columns(ov_fields_table,vec![1,8]);
        ui.set_table_editable_columns(ov_fields_table,if state.table_wizard_names_valid() {vec![0,1,6,7,8,9]} else {vec![0,1,6,8]});
        for (column,width) in state.wizard_field_widths.iter().enumerate() { ui.set_table_column_width(ov_fields_table,column,*width); }
        if let Some(edit)=&state.wizard_field_edit {
            if let Some(session)=&edit.input {ui.restore_text_input_state(ov_field_editor,session);}
            if let Some(session)=&edit.autocomplete {ui.set_autocomplete_state(ov_field_editor,session);}
            ui.set_accessible_name(ov_field_editor,match edit.column {0=>"Column name cell",1=>"Column type cell",6=>"Column default cell",7=>"Column CHECK expression cell",8=>"Column collation cell",9=>"Column foreign key cell",_=>"Column cell"});
        }
        ui.set_accessible_name(ov_fields_table,"Table definition fields: Name, Type, NN, PK, AI, U, Default, Check, Collation, Foreign Key. Click Type or Collation to open its dropdown; click a checkbox or press Space to toggle.");
        ui.set_accessible_name(ov_field_top,"Move selected column to top"); ui.set_accessible_name(ov_field_bottom,"Move selected column to bottom");
        for button in [ov_field_top,ov_btn_up] { ui.set_button_enabled(button,state.wizard_col_sel>0 && !state.wizard_cols.is_empty()); }
        for button in [ov_field_bottom,ov_btn_down] { ui.set_button_enabled(button,state.wizard_col_sel+1<state.wizard_cols.len()); }
        ui.set_button_enabled(ov_btn_del,!state.wizard_cols.is_empty());
    }
    for (table, columns) in [(ov_keys_table,if state.table_wizard_names_valid() {vec![0,1,2]} else {vec![1,2]}),(ov_fks_table,if state.table_wizard_names_valid() {vec![0,1,2]} else {vec![1]}),(ov_checks_table,if state.table_wizard_names_valid() {vec![0,1]} else {vec![1]})] {
        ui.set_table_editable_columns(table,columns);
    }
    if let Some(edit) = &state.wizard_constraint_edit {
        if let Some(session) = &edit.input { ui.restore_text_input_state(ov_constraint_editor,session); }
        ui.set_accessible_name(ov_constraint_editor, match (edit.page,edit.column) {
            (0,1) => "Index constraint type cell", (0,2) => "Index constraint name cell", (1,1) => "Foreign key name cell",
            (1,2) => "Foreign key references cell", (2,0) => "CHECK expression cell", _ => "CHECK name cell",
        });
    }
    if state.overlay.is_none() {
        let _ = ui.set_tabs_selected(tabs, state.tab.index());
        if docks_on && dock_tabs.0 != 0 {
            let focus_idx = visible_docks_index(state);
            let _ = ui.set_tabs_selected(dock_tabs, focus_idx);
        }
        if tree.0 != 0 {
            for database in 0..state.schema_names.len() {
                let root = database as u64 * 5 + 1;
                if let Some(expanded) = state.schema_tree_expanded.get(&state.schema_names[database]) {
                    for (group, value) in expanded.iter().enumerate() { ui.set_tree_expanded(tree, root + group as u64, *value); }
                }
            }
            if let Some(sel) = &state.selected {
                if let Some(idx) = state
                    .schema
                    .iter()
                    .position(|o| o.schema == sel.schema && o.name == sel.name && o.kind == sel.kind)
                {
                    ui.set_tree_selected(tree, Some(10_000 + idx as u64));
                }
            } else if let Some((schema, group)) = &state.schema_group {
                if let Some(database) = state.schema_names.iter().position(|name| name == schema) {
                    ui.set_tree_selected(tree, Some(database as u64 * 5 + 1 + group));
                }
            }
        }
        if let Some(row) = state.browse().sel_row {
            if browse_table.0 != 0 {
                ui.set_table_selected_row(browse_table, Some(row));
                let row_range = (
                    state.browse().sel_row_start.unwrap_or(row),
                    state.browse().sel_row_end.unwrap_or(row),
                );
                ui.set_table_selected_rows(browse_table, Some(row_range));
                if let Some(col) = state.browse().sel_col {
                    ui.set_table_selected_col(browse_table, Some(col));
                }
                if let Some((col_start, col_end)) = state
                    .browse()
                    .sel_col_start
                    .zip(state.browse().sel_col_end)
                {
                    ui.set_table_selected_cells(browse_table, Some(row_range), Some((col_start, col_end)));
                    if let Some(col) = state.browse().sel_col {
                        ui.set_table_active_cell(browse_table, row, col);
                    }
                }
            }
        }
        if browse_table.0 != 0 {
            ui.set_table_frozen_cols(browse_table, state.browse_frozen_cols());
            ui.set_table_cell_bg(browse_table, state.cell_format_colors());
            for (column, width) in state.browse().column_widths.iter().enumerate() {
                ui.set_table_column_width(browse_table, column, *width);
            }
            let (x, y) = state.browse().scroll;
            ui.set_table_scroll_offset(browse_table, x, y);
        }
        if sql_table.0 != 0 {
            for (column, width) in state.sql_column_widths.iter().enumerate() {
                ui.set_table_column_width(sql_table, column, *width);
            }
            let (x, y) = state.sql_scroll;
            ui.set_table_scroll_offset(sql_table, x, y);
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
        ConfirmKind::DeleteColumn(row) => (
            "Remove Field",
            format!("Remove field '{}'?\nThe field and its data will be removed when you save.", state.wizard_cols.get(row).map_or("", |column| column.name.as_str())),
            "Cancel",
        ),
        ConfirmKind::DeleteRow => (
            "Delete Record",
            "Delete the selected browse row (by rowid)?".into(),
            "Delete",
        ),
        ConfirmKind::Documents => (
            if matches!(state.pending_document, Some(crate::state::DocumentAction::SelectSchemaNode(_))) { "Keep edited DDL?" } else { "Save changes before continuing?" },
            if let Some(crate::state::DocumentAction::CloseSqlTab(index)) = state.pending_document {
                format!("Save changes to {} before closing this SQL tab? Discard closes this tab. Cancel keeps it open.", state.sql_tab_label(index))
            } else if matches!(state.pending_document, Some(crate::state::DocumentAction::SelectSchemaNode(_))) {
                "Keep the edited DDL in a new SQL tab before selecting another object. Discard replaces only this DDL draft. Cancel keeps the current object.".into()
            } else { "Save database edits, SQL drafts and project settings. Discard restores the saved state. Cancel keeps this session open.".into() },
            if matches!(state.pending_document, Some(crate::state::DocumentAction::SelectSchemaNode(_))) { "Keep in SQL tab" } else { "Save" },
        ),
        ConfirmKind::Revert => (
            "Revert Changes",
            "Discard all unwritten changes and reload?".into(),
            "Revert",
        ),
        ConfirmKind::None => ("Confirm", "Confirm action?".into(), "OK"),
    };
    let mut modal_style = ModalStyle::from_theme(&theme);
    if matches!(state.confirm_kind, ConfirmKind::DeleteColumn(_)) { modal_style.height = 170.0; }
    let modal = ui.modal(
        modal_title,
        modal_body,
        modal_primary,
        Some(if matches!(state.confirm_kind, ConfirmKind::DeleteColumn(_)) { "Remove" } else if state.confirm_kind == ConfirmKind::Documents { "Discard" } else { "Cancel" }.into()),
        modal_style,
    );
    if state.confirm_kind == ConfirmKind::Documents {
        ui.set_modal_tertiary(modal, Some("Cancel".into()));
    }
    if state.confirm_open {
        let _ = ui.set_modal_open(modal, true);
    }

    let browse_header_menu = ui.context_menu_popup(ui.context_menu_style());
    ui.set_accessible_name(browse_header_menu, "Browse column actions");
    ui.set_accessible_name(ov_col_type, "Column declared type (editable)");
    ui.set_accessible_name(ov_ai, "Column autoincrement");
    ui.set_accessible_name(ov_collation, "Column collation (editable)");
    ui.set_accessible_name(ov_check, "Column CHECK expression");
    ui.set_accessible_name(ov_foreign_key, "Column foreign key clause");
    ui.set_accessible_name(ov_generation, "Column generation mode");
    ui.set_accessible_name(ov_generated, "Generated column expression");
    ui.set_accessible_name(ov_table_preview, "Generated CREATE TABLE SQL preview");
    ui.set_accessible_name(ov_table_splitter,"Resize table definition and SQL preview");
    ui.set_splitter_min_sizes(ov_table_splitter,TABLE_PANE_MINIMUMS.0,TABLE_PANE_MINIMUMS.1);
    ui.set_text_area_read_only(ov_table_preview, true);
    ui.set_accessible_name(ov_table_tabs, "Table definition pages");
    if state.overlay==Some(OverlayKind::EditTable) { ui.set_accessible_name(ov_name,"Table name"); }
    ui.set_accessible_name(ov_table_advanced,if state.wizard_table_advanced { "Hide advanced table options" } else { "Show advanced table options" });
    ui.set_tabs_selected(ov_table_tabs, state.wizard_table_page);
    for (index,table) in [ov_keys_table,ov_fks_table,ov_checks_table].into_iter().enumerate() {
        let view=&state.wizard_constraint_views[index];
        ui.set_table_frozen_cols(table,1);ui.set_table_selected_col(table,view.column);
        for (column,&width) in view.widths.iter().enumerate() {ui.set_table_column_width(table,column,width);}
    }
    ui.set_accessible_name(ov_checks_table, "Table and column CHECK constraints");
    ui.set_table_selected_row(ov_checks_table, (!state.wizard_checks.is_empty()).then_some(state.wizard_check_sel));
    ui.set_accessible_name(ov_check_name, "CHECK constraint name");
    ui.set_accessible_name(ov_check_expr, "CHECK expression");
    ui.set_text_area_read_only(ov_check_expr, state.wizard_checks.is_empty() || !state.table_wizard_names_valid());
    ui.set_accessible_name(ov_check_add, "Add table CHECK constraint");
    ui.set_accessible_name(ov_check_add_column, "Add CHECK to selected column");
    ui.set_button_enabled(ov_check_add_column, !state.wizard_cols.is_empty() && state.table_wizard_names_valid());
    ui.set_accessible_name(ov_check_del, "Remove CHECK constraint");
    ui.set_accessible_name(ov_check_up, "Move CHECK up");
    ui.set_accessible_name(ov_check_down, "Move CHECK down");
    ui.set_button_enabled(ov_check_del, !state.wizard_checks.is_empty());
    let check_can_swap = |target| state.wizard_checks.get(state.wizard_check_sel).zip(state.wizard_checks.get(target)).is_some_and(|(a,b)|crate::table_sql::checks_can_swap(a,b));
    ui.set_button_enabled(ov_check_up, state.wizard_check_sel>0 && check_can_swap(state.wizard_check_sel.saturating_sub(1)));
    ui.set_button_enabled(ov_check_down, check_can_swap(state.wizard_check_sel+1));
    ui.set_accessible_name(ov_keys_table, "Table index constraints");
    ui.set_table_selected_row(ov_keys_table, (!state.wizard_index_constraints.is_empty()).then_some(state.wizard_key_sel));
    ui.set_accessible_name(ov_key_name, "Index constraint name");
    ui.set_accessible_name(ov_key_columns, "Index constraint ordered columns SQL");
    ui.set_accessible_name(ov_key_conflict, "Index constraint conflict action");
    ui.set_text_area_read_only(ov_key_columns, state.wizard_index_constraints.is_empty() || !state.table_wizard_names_valid());
    ui.set_accessible_name(ov_key_add_pk, "Add primary key constraint"); ui.set_accessible_name(ov_key_add_unique, "Add UNIQUE constraint");
    ui.set_accessible_name(ov_key_del, "Remove index constraint"); ui.set_accessible_name(ov_key_up, "Move index constraint up"); ui.set_accessible_name(ov_key_down, "Move index constraint down");
    ui.set_button_enabled(ov_key_add_pk, !state.wizard_index_constraints.iter().any(|key| key.primary));
    ui.set_button_enabled(ov_key_del, !state.wizard_index_constraints.is_empty());
    let movable_key = |index| state.wizard_index_constraints.get(index).is_some_and(|key| !crate::table_sql::index_constraint_requires_inline(key));
    ui.set_button_enabled(ov_key_up, state.wizard_key_sel > 0 && movable_key(state.wizard_key_sel) && movable_key(state.wizard_key_sel.saturating_sub(1)));
    ui.set_button_enabled(ov_key_down, movable_key(state.wizard_key_sel) && movable_key(state.wizard_key_sel+1));
    ui.set_accessible_name(ov_fks_table, "Table and column foreign keys");
    ui.set_table_selected_row(ov_fks_table, (!state.wizard_foreign_keys.is_empty()).then_some(state.wizard_foreign_key_sel));
    ui.set_accessible_name(ov_fk_name, "Foreign key constraint name");
    ui.set_accessible_name(ov_fk_columns, "Foreign key local columns SQL");
    ui.set_accessible_name(ov_fk_clause, "Foreign key REFERENCES clause");
    for area in [ov_fk_columns, ov_fk_clause] { ui.set_text_area_read_only(area, state.wizard_foreign_keys.is_empty() || !state.table_wizard_names_valid()); }
    ui.set_accessible_name(ov_fk_add, "Add table foreign key"); ui.set_accessible_name(ov_fk_del, "Remove foreign key");
    ui.set_accessible_name(ov_fk_up, "Move foreign key up"); ui.set_accessible_name(ov_fk_down, "Move foreign key down");
    ui.set_button_enabled(ov_fk_del, !state.wizard_foreign_keys.is_empty());
    let fk_can_swap = |target| state.wizard_foreign_keys.get(state.wizard_foreign_key_sel).zip(state.wizard_foreign_keys.get(target)).is_some_and(|(a,b)|crate::table_sql::foreign_keys_can_swap(a,b));
    ui.set_button_enabled(ov_fk_up, state.wizard_foreign_key_sel>0 && fk_can_swap(state.wizard_foreign_key_sel.saturating_sub(1)));
    ui.set_button_enabled(ov_fk_down, fk_can_swap(state.wizard_foreign_key_sel+1));
    ui.set_text_area_read_only(ov_generated, state.wizard_cols.get(state.wizard_col_sel).is_none_or(|column| column.generated.is_none()));
    ui.set_text_area_read_only(edit_value, generated_cell);
    ui.set_button_enabled(btn_edit_cell, !generated_cell);
    ui.set_accessible_name(ov_without_rowid, "Table WITHOUT ROWID option");
    ui.set_accessible_name(ov_strict, "Table STRICT option");
    ui.set_accessible_name(ov_pk_conflict, "Primary key conflict action");
    if state.overlay == Some(OverlayKind::EditIndex) {
        ui.set_accessible_name(ov_idx_available, "Available table columns for index");
        ui.set_accessible_name(ov_list, "Ordered index keys");
        ui.set_accessible_name(ov_idx_preview, "Generated CREATE INDEX SQL preview");
        ui.set_accessible_name(ov_idx_order, "Selected index key sort order");
        ui.set_text_area_read_only(ov_idx_preview, true);
        ui.set_list_selected(ov_idx_available, Some(state.wizard_index_available_sel));
        ui.set_list_selected(ov_list, Some(state.wizard_index_key_sel));
        let keys = crate::index_sql::keys(&state.wizard_index_cols);
        let count = keys.as_ref().map_or(0, Vec::len);
        ui.set_button_enabled(ov_btn_add, keys.is_ok() && ui.list_selected(ov_idx_available).is_some());
        ui.set_button_enabled(ov_idx_expression, keys.is_ok());
        for id in [ov_btn_del, ov_idx_edit_key] { ui.set_button_enabled(id, count > 0); }
        ui.set_button_enabled(ov_btn_up, count > 0 && state.wizard_index_key_sel > 0);
        ui.set_button_enabled(ov_btn_down, state.wizard_index_key_sel + 1 < count);
        crate::sql_syntax::apply(&mut ui, ov_idx_preview, &theme);
    }
    ui.set_accessible_name(ov_idx_cols, "Index columns and expressions");
    ui.set_accessible_name(ov_idx_where, "Partial index WHERE condition");
    if btn_sql_comment.0 != 0 {
        ui.set_accessible_name(btn_sql_comment, "Toggle SQL line comments (Ctrl/Cmd+/)");
    }

    let browse = state.browse();
    let loaded = browse.result.as_ref().map_or(0, |r| r.rows.len());
    let total = browse.total_rows.unwrap_or(0);
    let can_browse = state.conn.is_some() && !state.sql_busy && browse.result.is_some();
    for id in [btn_browse_new_tab, btn_browse_close_tab].into_iter().chain(browse_tab_btns.iter().copied()) {
        ui.set_button_enabled(id, !state.sql_busy);
    }
    ui.set_button_enabled(btn_browse_clear_sort, can_browse && !browse.ordering().is_empty());
    ui.set_button_enabled(btn_freeze, can_browse && browse.result.as_ref().is_some_and(|result| !result.columns.is_empty()));
    let record = browse.offset.saturating_add(browse.sel_row.unwrap_or(0));
    ui.set_button_enabled(btn_record_prev, can_browse && loaded > 0 && record > 0);
    ui.set_button_enabled(btn_record_next, can_browse && loaded > 0 && (browse.sel_row.is_none() || record.saturating_add(1) < total));
    ui.set_button_enabled(btn_form_prev, can_browse && loaded > 0 && record > 0);
    ui.set_button_enabled(btn_form_next, can_browse && loaded > 0 && record.saturating_add(1) < total);
    ui.set_button_enabled(btn_record_first, can_browse && total > 0 && (browse.sel_row.is_none() || record > 0));
    ui.set_button_enabled(btn_record_last, can_browse && total > 0 && (browse.sel_row.is_none() || record.saturating_add(1) < total));
    ui.set_button_enabled(btn_record_go, can_browse && total > 0);
    let can_write = state.conn.is_some() && !state.read_only && state.dirty && !state.sql_busy;
    let main_selection = state.selected.as_ref().is_none_or(|s| s.schema == "main");
    ui.set_text_area_read_only(ddl_area, !main_selection);
    ui.set_button_enabled(btn_run_ddl, main_selection && state.conn.is_some() && !state.read_only && !state.sql_busy);
    ui.set_button_enabled(btn_struct_modify, state.selected.as_ref().is_some_and(|s| matches!(s.kind, ObjectKind::Table | ObjectKind::Index)) && !state.read_only && !state.sql_busy);
    ui.set_button_enabled(btn_struct_delete, state.selected.is_some() && !state.read_only && !state.sql_busy);
    ui.set_button_enabled(btn_write, can_write);
    ui.set_button_enabled(btn_revert, can_write);

    ui.set_image_view(edit_image, state.edit_image_view);
    ui.set_text_area_line_numbers(sql_area, true);
    ui.set_text_area_read_only(log_area, true);
    crate::csv_files::configure(&mut ui,csv_files,state);
    crate::csv_files::build_confirmation(&mut ui,&mut csv_files,state,&theme,width as f32);
    let column_picker = crate::column_picker::build(&mut ui, state, &theme, width as f32);
    ui.sync(&mut scene);
    crate::column_picker::restore(&mut ui, column_picker, state.wizard_column_picker.as_ref());
    crate::csv_files::restore(&mut ui,csv_files,state);
    if let Some(draft)=state.wizard_constraint_edit.as_ref().and_then(|edit|edit.reference.as_ref()).or_else(||state.wizard_field_edit.as_ref().and_then(|edit|edit.reference.as_ref())) {crate::reference_editor::restore(&mut ui,reference_editor,draft);}
    ui.set_tree_scroll_offset(tree, state.schema_tree_scroll);
    if state.overlay == Some(OverlayKind::EditIndex) {
        for (index, list) in [ov_list, ov_idx_available].into_iter().enumerate() {
            ui.set_list_scroll_offset(list, state.wizard_index_list_scroll[index]);
        }
    }
    ui.set_table_scroll_offset(ov_fields_table,state.wizard_field_scroll.0,state.wizard_field_scroll.1);
    if state.wizard_field_edit.is_some() {ui.reveal_table_selection(ov_fields_table);}
    for (index,table) in [ov_keys_table,ov_fks_table,ov_checks_table].into_iter().enumerate() {
        let (x,y)=state.wizard_constraint_views[index].scroll;ui.set_table_scroll_offset(table,x,y);
    }
    if state.wizard_constraint_edit.as_ref().is_some_and(|edit|edit.reference.is_some()) {ui.reveal_table_selection(ov_fks_table);}
    if ov_csv_scroll.0 != 0 { ui.set_scroll_offset(ov_csv_scroll, state.csv_scroll.0, state.csv_scroll.1); }
    ui.set_scroll_offset(ov_scroll, state.wizard_scroll.0, state.wizard_scroll.1);
    ui.set_scroll_offset(ov_checks_scroll, state.wizard_check_scroll.0, state.wizard_check_scroll.1);
    ui.set_scroll_offset(ov_keys_scroll, state.wizard_key_scroll.0, state.wizard_key_scroll.1);
    ui.set_scroll_offset(ov_fks_scroll, state.wizard_foreign_key_scroll.0, state.wizard_foreign_key_scroll.1);
    ui.set_scroll_offset(shell_scroll, state.shell_scroll.0, state.shell_scroll.1);
    let page_offset = state.page_scroll[state.tab.index()];
    ui.set_scroll_offset(page_scroll, page_offset.0, page_offset.1);
    let dock_offset = state.dock_scroll[state.dock_focus as usize];
    ui.set_scroll_offset(dock_scroll, dock_offset.0, dock_offset.1);
    if let Some(Some(editor)) = state.sql_edit_states.get(state.sql_tab_idx) {
        ui.restore_text_area_state(sql_area, editor);
    }
    for (index, area) in [ov_check, ov_foreign_key, ov_generated].into_iter().enumerate() {
        if let Some(Some(editor)) = state.wizard_column_edit_states.get(state.wizard_col_sel).map(|states| &states[index]) {
            ui.restore_text_area_state(area, editor);
        }
        crate::sql_syntax::apply(&mut ui, area, &theme);
    }
    for (index, area) in [ov_idx_cols, ov_idx_where, ov_idx_preview].into_iter().enumerate() {
        if let Some(editor) = &state.wizard_index_edit_states[index] { ui.restore_text_area_state(area, editor); }
        crate::sql_syntax::apply(&mut ui, area, &theme);
    }
    if let Some(editor) = &state.wizard_table_preview_state { ui.restore_text_area_state(ov_table_preview, editor); }
    if state.overlay==Some(OverlayKind::EditTable) {
        if let Some(editor)=&state.wizard_table_name_state { ui.restore_text_input_state(ov_name,editor); }
    }
    crate::sql_syntax::apply(&mut ui, ov_table_preview, &theme);
    if let Some(Some(editor)) = state.wizard_check_states.get(state.wizard_check_sel) { ui.restore_text_area_state(ov_check_expr, editor); }
    crate::sql_syntax::apply(&mut ui, ov_check_expr, &theme);
    if let Some(Some(editor)) = state.wizard_key_states.get(state.wizard_key_sel) { ui.restore_text_area_state(ov_key_columns, editor); }
    crate::sql_syntax::apply(&mut ui, ov_key_columns, &theme);
    for (index, area) in [ov_fk_columns, ov_fk_clause].into_iter().enumerate() {
        if let Some(editors) = state.wizard_foreign_key_states.get(state.wizard_foreign_key_sel) {
            if let Some(editor) = &editors[index] { ui.restore_text_area_state(area, editor); }
        }
        crate::sql_syntax::apply(&mut ui, area, &theme);
    }
    if !state.sql_log_view.as_ref().is_some_and(|view| ui.restore_text_area_state(log_area, view)) {
        let len = ui.text_area_value(log_area).map_or(0, |s| s.chars().count());
        ui.select_text_area_range(log_area, len..len);
    }
    crate::sql_syntax::apply(&mut ui, sql_area, &theme);
    ui.sync(&mut scene);
    scene.mark_full_dirty(win_w, win_h);

    BuiltUi {
        ui,
        csv_files,
        column_picker,
        reference_editor,
        body_splitter,
        structure_splitter,
        shell_scroll,
        page_scroll,
        dock_scroll,
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
        btn_browse_close_tab,
        browse_select,
        browse_filter,
        browse_global_filter,
        btn_browse_clear_filters,
        btn_browse_clear_sort,
        btn_browse_go,
        btn_record_prev,
        btn_record_next,
        btn_record_first,
        btn_record_last,
        record_number,
        btn_record_go,
        btn_add_row,
        btn_del_row,
        browse_table,
        browse_header_menu,
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
        btn_sql_save_as,
        btn_sql_close,
        btn_sql_left,
        btn_sql_right,
        btn_sql_save,
        btn_sql_find,
        btn_sql_stop,
        btn_sql_csv,
        btn_sql_json,
        sql_table,
        sql_alert,
        dock_tabs,
        edit_col,
        edit_value,
        edit_type,
        btn_edit_discard,
        btn_edit_cell,
        schema_list,
        log_area,
        log_filter,
        log_clear,
        log_export,
        chart_select,
        chart_col,
        btn_chart,
        chart,
        status,
        modal,
        ov_name,
        ov_schema,
        ov_scroll,
        insert_values,
        insert_modes,
        ov_list,
        ov_fields_table,
        ov_field_editor,
        ov_constraint_editor,
        ov_field_top,
        ov_field_bottom,
        ov_col_name,
        ov_col_type,
        ov_pk,
        ov_ai,
        ov_collation,
        ov_check,
        ov_foreign_key,
        ov_generated,
        ov_generation,
        ov_table_preview,
        ov_table_splitter,
        ov_table_tabs,
        ov_table_advanced,
        ov_checks_scroll,
        ov_checks_table,
        ov_check_name,
        ov_check_expr,
        ov_check_add,
        ov_check_add_column,
        ov_check_del,
        ov_check_up,
        ov_check_down,
        ov_keys_scroll,
        ov_keys_table,
        ov_key_name,
        ov_key_columns,
        ov_key_conflict,
        ov_key_add_pk,
        ov_key_add_unique,
        ov_key_del,
        ov_key_up,
        ov_key_down,
        ov_fks_scroll,
        ov_fks_table,
        ov_fk_name,
        ov_fk_columns,
        ov_fk_clause,
        ov_fk_add,
        ov_fk_del,
        ov_fk_up,
        ov_fk_down,
        ov_without_rowid,
        ov_strict,
        ov_pk_conflict,
        ov_nn,
        ov_unique,
        ov_default,
        ov_btn_add,
        ov_btn_del,
        ov_btn_up,
        ov_btn_down,
        ov_btn_ok,
        ov_btn_cancel,
        ov_idx_available,
        ov_idx_expression,
        ov_idx_edit_key,
        ov_idx_preview,
        ov_idx_order,
        ov_idx_table,
        ov_idx_cols,
        ov_idx_where,
        ov_idx_unique,
        ov_csv_table,
        ov_csv_header,
        ov_csv_delim,
        ov_csv_quote,
        ov_csv_encoding,
        ov_csv_custom_encoding,
        ov_csv_custom_delimiter,
        ov_csv_custom_quote,
        ov_csv_trim,
        ov_csv_local,
        ov_csv_types,
        ov_csv_defaults,
        ov_csv_missing,
        ov_csv_conflict,
        ov_csv_advanced,
        ov_csv_refresh,
        ov_csv_preview,
        ov_csv_preview_error,
        ov_csv_scroll,
        ov_csv_paste,
        ov_attach_path,
        ov_attach_schema,
        ov_find,
        ov_replace,
        ov_find_options,
        ov_btn_find_all,
        ov_btn_find,
        ov_btn_repl,
        ov_btn_repl_all,
        ov_cipher_pass,
        ov_cipher_page,
        ov_cipher_kdf,
        ov_prefs_font,
        ov_prefs_theme,
        ov_prefs_confirm,
        ov_prefs_completion,
        ov_prefs_upper_keywords,
        ov_prefs_delim,
        ov_frozen,
        ov_export_struct,
        ov_export_data,
        ov_export_drop,
        ov_export_fk,
        ov_cond_col,
        ov_cond_op,
        ov_cond_val,
        ov_cond_color,
        btn_sql_line,
        btn_sql_comment,
        btn_sql_view,
        sql_view_name,
        btn_edit_text,
        btn_edit_hex,
        btn_edit_json,
        btn_edit_image,
        btn_fk_nav,
        btn_freeze,
        btn_cond_fmt,
        edit_meta,
        toolbar_extra,
        toolbar_project,
        btn_project_save,
        btn_integrity,
        ov_ext_path,
        ov_ext_entry,
        btn_form_view,
        btn_form_prev,
        btn_form_next,
        form_fields,
        image_fit,
        image_actual,
        image_in,
        image_out,
        image_export,
        edit_image,
        remote_list,
        remote_draft,
        btn_remote_add,
        btn_remote_open,
    }
}

fn visible_docks_index(state: &AppState) -> usize {
    state
        .visible_docks()
        .iter()
        .position(|d| *d == state.dock_focus)
        .unwrap_or(0)
}
