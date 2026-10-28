//! DB Browser for SQLite–style example (`e28_db_browser`) — DB4S chrome parity.
//!
//! Soft↔GPU: Space when nothing focused. Theme: toolbar / View menu.
//! `NEXT_GUI_DB_BROWSER_QA=1` dumps a PPM after first paint then exits.

mod db;
mod csv_encoding;
mod csv_import;
mod csv_files;
mod csv_jobs;
mod csv_numbers;
mod data_export;
mod export_jobs;
mod cell_clipboard;
mod cell_snapshot;
mod plot;
mod plot_load;
mod plot_export;
#[cfg(test)]
mod dock_tests;
mod file_dlg;
mod index_sql;
mod table_sql;
mod column_picker;
mod reference_editor;
mod table_migration;
mod filters;
mod prefs;
mod pragmas;
mod project;
mod project_xml;
mod sql_log;
mod sql_export;
mod sql_search;
mod sql_editor;
mod sql_syntax;
mod sql_completion;
mod sql_calltip;
mod sql_function_help;
mod state;
mod ui_build;

use std::{path::PathBuf, time::Instant};

use next_gui_platform::{
    AppCallbacks, Event, ImeEvent, KeyCode, Modifiers, PointerButton, PresentBackend, WindowAttrs,
    WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::{load_default_cjk_font, Font};
use next_gui_ui::{ModalAction, Theme, ThemeMode, Ui, UiKey, WidgetId};

use crate::db::{self as dbmod, ObjectKind};
use crate::state::{AppState, ConfirmKind, DockKind, DocumentAction, MainTab, OverlayKind};
use crate::ui_build::{build_ui, clear_color, mid, BuiltUi};

const CURRENT_PLOT_QA_SQL: &str = "CREATE TABLE quarterly_revenue(observed TEXT, revenue REAL, expenses REAL, region TEXT);
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<18)
INSERT INTO quarterly_revenue SELECT printf('2026-10-%02d',x), CASE WHEN x=9 THEN NULL ELSE 1200+x*150+(x%3)*80 END, 520+x*64, CASE WHEN x%2=0 THEN 'APAC' ELSE 'EMEA' END FROM n;";

const BAR_PLOT_QA_SQL: &str = "CREATE TABLE quarterly_revenue(observed TEXT,revenue REAL,expenses REAL,region TEXT);
INSERT INTO quarterly_revenue VALUES ('Q1',12,8,'EMEA'),('Q2',18,11,'APAC'),('Q3',-7,-4,'EMEA'),('Q4',9,-3,'APAC');";
const NATIVE_PLOT_QA_XML: &str = r##"<sqlb_project><db path=':memory:'/><tab_browse><browse_table_settings>
    <table schema='main' name='quarterly_revenue' plot_x_axis='observed'><plot_y_axes>
    <y_axis name='revenue' line_style='2' point_shape='5' colour='#3385eb' active='1'/>
    <y2_axis name='expenses' line_style='5' point_shape='0' colour='#d7a132' active='1'/>
    </plot_y_axes></table></browse_table_settings></tab_browse></sqlb_project>"##;

const FULL_PLOT_QA_SQL: &str = "CREATE TABLE telemetry(sequence INTEGER,measured REAL,reference REAL,region TEXT);
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<3000)
INSERT INTO telemetry SELECT x,CASE WHEN x%97=0 THEN NULL ELSE 1200+(x*73)%1100+(x%7)*70 END,520+(x%101)*30,CASE WHEN x%2=0 THEN 'APAC' ELSE 'EMEA' END FROM n;";

const SCHEMA_NAMESPACE_QA_SQL: &str = "CREATE TABLE customers(id INTEGER PRIMARY KEY, name TEXT);
    INSERT INTO main.customers VALUES(1, 'Main customer');
    CREATE TEMP TABLE customers(temp_value TEXT);
    ATTACH ':memory:' AS analytics;
    CREATE TABLE analytics.customers(id INTEGER PRIMARY KEY, region TEXT, revenue REAL);
    INSERT INTO analytics.customers VALUES(1, 'APAC', 125.5);
    CREATE VIEW analytics.customer_summary AS SELECT region, sum(revenue) FROM customers GROUP BY region;
    CREATE INDEX analytics.customer_region ON customers(region);
    ATTACH ':memory:' AS archive;";

enum Backend {
    Soft(SoftRenderer),
    Gpu(GpuRenderer),
}

impl Backend {
    fn name(&self) -> &str {
        match self {
            Backend::Soft(_) => "Soft",
            Backend::Gpu(_) => "GPU",
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        match self {
            Backend::Soft(s) => s.resize(w, h),
            Backend::Gpu(g) => g.resize(w, h),
        }
    }

    fn render(
        &mut self,
        cmds: &[next_gui_render::DrawCommand],
        dirty: &next_gui_scene::DirtyRegion,
    ) {
        match self {
            Backend::Soft(s) => s.render(cmds, dirty),
            Backend::Gpu(g) => g.render(cmds, dirty),
        }
    }

    fn set_clear_color(&mut self, c: Color) {
        match self {
            Backend::Soft(s) => s.set_clear_color(c),
            Backend::Gpu(g) => g.set_clear_color(c),
        }
    }

    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)> {
        match self {
            Backend::Soft(s) => s.frame_pixels(),
            Backend::Gpu(g) => g.frame_pixels(),
        }
    }
}

fn map_key(key: KeyCode, modifiers: Modifiers) -> Option<UiKey> {
    let word_modifier = if cfg!(target_os = "macos") {
        modifiers.alt && !modifiers.ctrl && !modifiers.meta
    } else {
        modifiers.ctrl && !modifiers.alt && !modifiers.meta
    };
    if word_modifier {
        match key {
            KeyCode::Left => return Some(if modifiers.shift { UiKey::SelectWordLeft } else { UiKey::WordLeft }),
            KeyCode::Right => return Some(if modifiers.shift { UiKey::SelectWordRight } else { UiKey::WordRight }),
            KeyCode::Backspace => return Some(UiKey::DeleteWordLeft),
            KeyCode::Delete => return Some(UiKey::DeleteWordRight),
            _ => {}
        }
    }
    if modifiers.commandish() {
        return match key {
            KeyCode::Char('a' | 'A') => Some(UiKey::SelectAll),
            KeyCode::Char('z' | 'Z') => Some(if modifiers.shift { UiKey::Redo } else { UiKey::Undo }),
            KeyCode::Char('y' | 'Y') => Some(UiKey::Redo),
            _ => None,
        };
    }
    if modifiers.shift {
        match key {
            KeyCode::Left => return Some(UiKey::SelectLeft),
            KeyCode::Right => return Some(UiKey::SelectRight),
            KeyCode::Up => return Some(UiKey::SelectUp),
            KeyCode::Down => return Some(UiKey::SelectDown),
            KeyCode::PageUp => return Some(UiKey::SelectPageUp),
            KeyCode::PageDown => return Some(UiKey::SelectPageDown),
            KeyCode::Home => return Some(UiKey::SelectHome),
            KeyCode::End => return Some(UiKey::SelectEnd),
            _ => {}
        }
    }
    if matches!(key, KeyCode::Tab) {
        return Some(if modifiers.shift {
            UiKey::BackTab
        } else {
            UiKey::Tab
        });
    }
    Some(match key {
        KeyCode::Backspace => UiKey::Backspace,
        KeyCode::Delete => UiKey::Delete,
        KeyCode::Left => UiKey::Left,
        KeyCode::Right => UiKey::Right,
        KeyCode::Up => UiKey::Up,
        KeyCode::Down => UiKey::Down,
        KeyCode::Home => UiKey::Home,
        KeyCode::End => UiKey::End,
        KeyCode::PageUp => UiKey::PageUp,
        KeyCode::PageDown => UiKey::PageDown,
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Char(c) => UiKey::Char(c),
        KeyCode::Function(2) => UiKey::Edit,
        KeyCode::Function(_) | KeyCode::Other(_) => return None,
        KeyCode::Tab => unreachable!(),
    })
}

fn write_ppm(path: &str, pixels: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "P6\n{w} {h}\n255")?;
    for &px in pixels {
        let r = ((px >> 16) & 0xff) as u8;
        let g = ((px >> 8) & 0xff) as u8;
        let b = (px & 0xff) as u8;
        f.write_all(&[r, g, b])?;
    }
    Ok(())
}

struct Demo {
    font: Font,
    theme: Theme,
    state: AppState,
    ids: BuiltUiIds,
    scene: Scene,
    ui: Ui,
    backend: Backend,
    gpu_stash: Option<GpuRenderer>,
    surface_gpu: Option<GpuRenderer>,
    using_gpu: bool,
    window_backend: PresentBackend,
    win_w: u32,
    win_h: u32,
    needs_paint: bool,
    pending_native: bool,
    rebuild: bool,
    qa_exit: bool,
    qa_done: bool,
    qa_stepper_started: bool,
    last_body_ratio: Option<f32>,
    last_structure_ratio: Option<f32>,
    last_dock_ratio: Option<f32>,
    last_plot_ratio: Option<f32>,
    last_tree_sel: Option<u64>,
    last_schema_names: Vec<String>,
    reveal_schema_selection: bool,
    last_tab: usize,
    last_browse_sel: Option<usize>,
    last_browse_row: Option<usize>,
    last_log_revision: u64,
    cell_clipboard: Option<cell_clipboard::CellClipboard>,
    exit_requested: bool,
    reveal_browse_selection: bool,
    browse_header_column: Option<usize>,
}

#[allow(dead_code)]
struct BuiltUiIds {
    csv_files: csv_files::Controls,
    data_export: data_export::Controls,
    sql_export: sql_export::Controls,
    plot_load: plot_load::Controls,
    column_picker: column_picker::Controls,
    reference_editor: reference_editor::Controls,
    status: WidgetId,
    body_splitter: WidgetId,
    structure_splitter: WidgetId,
    dock_splitter: WidgetId,
    plot_splitter: WidgetId,
    dock_scrolls: [WidgetId;5],
    btn_dock_edit_close: WidgetId,
    btn_dock_close: WidgetId,
    shell_scroll: WidgetId,
    page_scroll: WidgetId,
    pragma_scroll: WidgetId,
    menu_file: WidgetId,
    menu_edit: WidgetId,
    menu_view: WidgetId,
    menu_tools: WidgetId,
    menu_help: WidgetId,
    btn_new: WidgetId,
    btn_open: WidgetId,
    btn_write: WidgetId,
    btn_revert: WidgetId,
    btn_theme: WidgetId,
    tabs: WidgetId,
    tree: WidgetId,
    ddl_area: WidgetId,
    btn_struct_new_table: WidgetId,
    btn_struct_new_index: WidgetId,
    btn_struct_modify: WidgetId,
    btn_struct_delete: WidgetId,
    btn_struct_refresh: WidgetId,
    btn_run_ddl: WidgetId,
    btn_browse_new_tab: WidgetId,
    btn_browse_close_tab: WidgetId,
    browse_select: WidgetId,
    browse_filter: WidgetId,
    browse_global_filter: WidgetId,
    btn_browse_clear_filters: WidgetId,
    btn_browse_clear_sort: WidgetId,
    btn_browse_go: WidgetId,
    btn_record_prev: WidgetId,
    btn_record_next: WidgetId,
    btn_record_first: WidgetId,
    btn_record_last: WidgetId,
    record_number: WidgetId,
    btn_record_go: WidgetId,
    btn_add_row: WidgetId,
    btn_del_row: WidgetId,
    browse_table: WidgetId,
    context_menu: WidgetId,
    browse_col_filters: Vec<WidgetId>,
    browse_tab_btns: Vec<WidgetId>,
    sql_tab_btns: Vec<WidgetId>,
    pragma_inputs: Vec<WidgetId>,
    pragma_switches: Vec<WidgetId>,
    pragma_selects: Vec<WidgetId>,
    btn_pragma_apply: WidgetId,
    sql_area: WidgetId,
    btn_sql_run: WidgetId,
    btn_sql_new_tab: WidgetId,
    btn_sql_open: WidgetId,
    btn_sql_save_as: WidgetId,
    btn_sql_close: WidgetId,
    btn_sql_left: WidgetId,
    btn_sql_right: WidgetId,
    btn_sql_save: WidgetId,
    btn_sql_find: WidgetId,
    btn_sql_stop: WidgetId,
    btn_sql_csv: WidgetId,
    btn_sql_json: WidgetId,
    sql_table: WidgetId,
    dock_tabs: WidgetId,
    edit_col: WidgetId,
    edit_value: WidgetId,
    edit_type: WidgetId,
    btn_edit_discard: WidgetId,
    btn_edit_cell: WidgetId,
    chart: WidgetId,
    plot: plot::Controls,
    chart_select: WidgetId,
    chart_col: WidgetId,
    btn_chart: WidgetId,
    modal: WidgetId,
    ov_name: WidgetId,
    ov_schema: WidgetId,
    ov_scroll: WidgetId,
    insert_values: Vec<WidgetId>,
    insert_modes: Vec<WidgetId>,
    ov_list: WidgetId,
    ov_fields_table: WidgetId,
    ov_field_editor: WidgetId,
    ov_constraint_editor: WidgetId,
    ov_field_top: WidgetId,
    ov_field_bottom: WidgetId,
    ov_col_name: WidgetId,
    ov_col_type: WidgetId,
    ov_pk: WidgetId,
    ov_ai: WidgetId,
    ov_collation: WidgetId,
    ov_check: WidgetId,
    ov_foreign_key: WidgetId,
    ov_generated: WidgetId,
    ov_generation: WidgetId,
    ov_table_preview: WidgetId,
    ov_table_splitter: WidgetId,
    ov_table_tabs: WidgetId,
    ov_table_advanced: WidgetId,
    ov_checks_scroll: WidgetId,
    ov_checks_table: WidgetId,
    ov_check_name: WidgetId,
    ov_check_expr: WidgetId,
    ov_check_add: WidgetId,
    ov_check_add_column: WidgetId,
    ov_check_del: WidgetId,
    ov_check_up: WidgetId,
    ov_check_down: WidgetId,
    ov_keys_scroll: WidgetId,
    ov_keys_table: WidgetId,
    ov_key_name: WidgetId,
    ov_key_columns: WidgetId,
    ov_key_conflict: WidgetId,
    ov_key_add_pk: WidgetId,
    ov_key_add_unique: WidgetId,
    ov_key_del: WidgetId,
    ov_key_up: WidgetId,
    ov_key_down: WidgetId,
    ov_fks_scroll: WidgetId,
    ov_fks_table: WidgetId,
    ov_fk_name: WidgetId,
    ov_fk_columns: WidgetId,
    ov_fk_clause: WidgetId,
    ov_fk_add: WidgetId,
    ov_fk_del: WidgetId,
    ov_fk_up: WidgetId,
    ov_fk_down: WidgetId,
    ov_without_rowid: WidgetId,
    ov_strict: WidgetId,
    ov_pk_conflict: WidgetId,
    ov_nn: WidgetId,
    ov_unique: WidgetId,
    ov_default: WidgetId,
    ov_btn_add: WidgetId,
    ov_btn_del: WidgetId,
    ov_btn_up: WidgetId,
    ov_btn_down: WidgetId,
    ov_btn_ok: WidgetId,
    ov_btn_cancel: WidgetId,
    ov_idx_available: WidgetId,
    ov_idx_expression: WidgetId,
    ov_idx_edit_key: WidgetId,
    ov_idx_preview: WidgetId,
    ov_idx_order: WidgetId,
    ov_idx_table: WidgetId,
    ov_idx_cols: WidgetId,
    ov_idx_where: WidgetId,
    ov_idx_unique: WidgetId,
    ov_csv_table: WidgetId,
    ov_csv_header: WidgetId,
    ov_csv_delim: WidgetId,
    ov_csv_quote: WidgetId,
    ov_csv_encoding: WidgetId,
    ov_csv_custom_encoding: WidgetId,
    ov_csv_custom_delimiter: WidgetId,
    ov_csv_custom_quote: WidgetId,
    ov_csv_trim: WidgetId,
    ov_csv_local: WidgetId,
    ov_csv_types: WidgetId,
    ov_csv_defaults: WidgetId,
    ov_csv_missing: WidgetId,
    ov_csv_conflict: WidgetId,
    ov_csv_advanced: WidgetId,
    ov_csv_refresh: WidgetId,
    ov_csv_preview: WidgetId,
    ov_csv_preview_error: WidgetId,
    ov_csv_scroll: WidgetId,
    ov_csv_paste: WidgetId,
    ov_attach_path: WidgetId,
    ov_attach_schema: WidgetId,
    ov_find: WidgetId,
    ov_replace: WidgetId,
    ov_find_options: [WidgetId; 6],
    ov_btn_find_all: WidgetId,
    ov_btn_find: WidgetId,
    ov_btn_repl: WidgetId,
    ov_btn_repl_all: WidgetId,
    ov_cipher_pass: WidgetId,
    ov_cipher_page: WidgetId,
    ov_cipher_kdf: WidgetId,
    ov_prefs_font: WidgetId,
    ov_prefs_theme: WidgetId,
    ov_prefs_confirm: WidgetId,
    ov_prefs_completion: WidgetId,
    ov_prefs_upper_keywords: WidgetId,
    ov_prefs_delim: WidgetId,
    ov_frozen: WidgetId,
    ov_cond_col: WidgetId,
    ov_cond_op: WidgetId,
    ov_cond_val: WidgetId,
    ov_cond_color: WidgetId,
    btn_sql_line: WidgetId,
    btn_sql_comment: WidgetId,
    btn_sql_view: WidgetId,
    sql_view_name: WidgetId,
    btn_edit_text: WidgetId,
    btn_edit_hex: WidgetId,
    btn_edit_json: WidgetId,
    btn_edit_image: WidgetId,
    btn_fk_nav: WidgetId,
    btn_freeze: WidgetId,
    btn_cond_fmt: WidgetId,
    toolbar_extra: WidgetId,
    toolbar_project: WidgetId,
    btn_project_save: WidgetId,
    btn_integrity: WidgetId,
    ov_ext_path: WidgetId,
    ov_ext_entry: WidgetId,
    btn_form_view: WidgetId,
    btn_form_prev: WidgetId,
    btn_form_next: WidgetId,
    form_fields: Vec<WidgetId>,
    image_fit: WidgetId,
    image_actual: WidgetId,
    image_in: WidgetId,
    image_out: WidgetId,
    image_export: WidgetId,
    edit_image: WidgetId,
    remote_list: WidgetId,
    remote_draft: WidgetId,
    btn_remote_add: WidgetId,
    btn_remote_open: WidgetId,
    log_area: WidgetId,
    log_filter: WidgetId,
    log_clear: WidgetId,
    log_export: WidgetId,
}

impl From<&BuiltUi> for BuiltUiIds {
    fn from(b: &BuiltUi) -> Self {
        Self {
            csv_files: b.csv_files,
            data_export: b.data_export,
            sql_export: b.sql_export,
            plot_load: b.plot_load,
            column_picker: b.column_picker,
            reference_editor: b.reference_editor,
            status: b.status,
            body_splitter: b.body_splitter,
            structure_splitter: b.structure_splitter,
            dock_splitter: b.dock_splitter,
            plot_splitter: b.plot_splitter,
            dock_scrolls: b.dock_scrolls,
            btn_dock_edit_close: b.btn_dock_edit_close,
            btn_dock_close: b.btn_dock_close,
            shell_scroll: b.shell_scroll,
            page_scroll: b.page_scroll,
            pragma_scroll: b.pragma_scroll,
            menu_file: b.menu_file,
            menu_edit: b.menu_edit,
            menu_view: b.menu_view,
            menu_tools: b.menu_tools,
            menu_help: b.menu_help,
            btn_new: b.btn_new,
            btn_open: b.btn_open,
            btn_write: b.btn_write,
            btn_revert: b.btn_revert,
            btn_theme: b.btn_theme,
            tabs: b.tabs,
            tree: b.tree,
            ddl_area: b.ddl_area,
            btn_struct_new_table: b.btn_struct_new_table,
            btn_struct_new_index: b.btn_struct_new_index,
            btn_struct_modify: b.btn_struct_modify,
            btn_struct_delete: b.btn_struct_delete,
            btn_struct_refresh: b.btn_struct_refresh,
            btn_run_ddl: b.btn_run_ddl,
            btn_browse_new_tab: b.btn_browse_new_tab,
            btn_browse_close_tab: b.btn_browse_close_tab,
            browse_select: b.browse_select,
            browse_filter: b.browse_filter,
            browse_global_filter: b.browse_global_filter,
            btn_browse_clear_filters: b.btn_browse_clear_filters,
            btn_browse_clear_sort: b.btn_browse_clear_sort,
            btn_browse_go: b.btn_browse_go,
            btn_record_prev: b.btn_record_prev,
            btn_record_next: b.btn_record_next,
            btn_record_first: b.btn_record_first,
            btn_record_last: b.btn_record_last,
            record_number: b.record_number,
            btn_record_go: b.btn_record_go,
            btn_add_row: b.btn_add_row,
            btn_del_row: b.btn_del_row,
            browse_table: b.browse_table,
            context_menu: b.context_menu,
            browse_col_filters: b.browse_col_filters.clone(),
            browse_tab_btns: b.browse_tab_btns.clone(),
            sql_tab_btns: b.sql_tab_btns.clone(),
            pragma_inputs: b.pragma_inputs.clone(),
            pragma_switches: b.pragma_switches.clone(),
            pragma_selects: b.pragma_selects.clone(),
            btn_pragma_apply: b.btn_pragma_apply,
            sql_area: b.sql_area,
            btn_sql_run: b.btn_sql_run,
            btn_sql_new_tab: b.btn_sql_new_tab,
            btn_sql_open: b.btn_sql_open,
            btn_sql_save_as: b.btn_sql_save_as,
            btn_sql_close: b.btn_sql_close,
            btn_sql_left: b.btn_sql_left,
            btn_sql_right: b.btn_sql_right,
            btn_sql_save: b.btn_sql_save,
            btn_sql_find: b.btn_sql_find,
            btn_sql_stop: b.btn_sql_stop,
            btn_sql_csv: b.btn_sql_csv,
            btn_sql_json: b.btn_sql_json,
            sql_table: b.sql_table,
            dock_tabs: b.dock_tabs,
            edit_col: b.edit_col,
            edit_value: b.edit_value,
            edit_type: b.edit_type,
            btn_edit_discard: b.btn_edit_discard,
            btn_edit_cell: b.btn_edit_cell,
            chart: b.chart,
            plot: b.plot,
            chart_select: b.chart_select,
            chart_col: b.chart_col,
            btn_chart: b.btn_chart,
            modal: b.modal,
            ov_name: b.ov_name,
            ov_schema: b.ov_schema,
            ov_scroll: b.ov_scroll,
            insert_values: b.insert_values.clone(),
            insert_modes: b.insert_modes.clone(),
            ov_list: b.ov_list,
            ov_fields_table: b.ov_fields_table,
            ov_field_editor: b.ov_field_editor,
            ov_constraint_editor: b.ov_constraint_editor,
            ov_field_top: b.ov_field_top,
            ov_field_bottom: b.ov_field_bottom,
            ov_col_name: b.ov_col_name,
            ov_col_type: b.ov_col_type,
            ov_pk: b.ov_pk,
            ov_ai: b.ov_ai,
            ov_collation: b.ov_collation,
            ov_check: b.ov_check,
            ov_foreign_key: b.ov_foreign_key,
            ov_generated: b.ov_generated,
            ov_generation: b.ov_generation,
            ov_table_preview: b.ov_table_preview,
            ov_table_splitter: b.ov_table_splitter,
            ov_table_tabs: b.ov_table_tabs,
            ov_table_advanced: b.ov_table_advanced,
            ov_checks_scroll: b.ov_checks_scroll,
            ov_checks_table: b.ov_checks_table,
            ov_check_name: b.ov_check_name,
            ov_check_expr: b.ov_check_expr,
            ov_check_add: b.ov_check_add,
            ov_check_add_column: b.ov_check_add_column,
            ov_check_del: b.ov_check_del,
            ov_check_up: b.ov_check_up,
            ov_check_down: b.ov_check_down,
            ov_keys_scroll: b.ov_keys_scroll,
            ov_keys_table: b.ov_keys_table,
            ov_key_name: b.ov_key_name,
            ov_key_columns: b.ov_key_columns,
            ov_key_conflict: b.ov_key_conflict,
            ov_key_add_pk: b.ov_key_add_pk,
            ov_key_add_unique: b.ov_key_add_unique,
            ov_key_del: b.ov_key_del,
            ov_key_up: b.ov_key_up,
            ov_key_down: b.ov_key_down,
            ov_fks_scroll: b.ov_fks_scroll,
            ov_fks_table: b.ov_fks_table,
            ov_fk_name: b.ov_fk_name,
            ov_fk_columns: b.ov_fk_columns,
            ov_fk_clause: b.ov_fk_clause,
            ov_fk_add: b.ov_fk_add,
            ov_fk_del: b.ov_fk_del,
            ov_fk_up: b.ov_fk_up,
            ov_fk_down: b.ov_fk_down,
            ov_without_rowid: b.ov_without_rowid,
            ov_strict: b.ov_strict,
            ov_pk_conflict: b.ov_pk_conflict,
            ov_nn: b.ov_nn,
            ov_unique: b.ov_unique,
            ov_default: b.ov_default,
            ov_btn_add: b.ov_btn_add,
            ov_btn_del: b.ov_btn_del,
            ov_btn_up: b.ov_btn_up,
            ov_btn_down: b.ov_btn_down,
            ov_btn_ok: b.ov_btn_ok,
            ov_btn_cancel: b.ov_btn_cancel,
            ov_idx_available: b.ov_idx_available,
            ov_idx_expression: b.ov_idx_expression,
            ov_idx_edit_key: b.ov_idx_edit_key,
            ov_idx_preview: b.ov_idx_preview,
            ov_idx_order: b.ov_idx_order,
            ov_idx_table: b.ov_idx_table,
            ov_idx_cols: b.ov_idx_cols,
            ov_idx_where: b.ov_idx_where,
            ov_idx_unique: b.ov_idx_unique,
            ov_csv_table: b.ov_csv_table,
            ov_csv_header: b.ov_csv_header,
            ov_csv_delim: b.ov_csv_delim,
            ov_csv_quote: b.ov_csv_quote,
            ov_csv_encoding: b.ov_csv_encoding,
            ov_csv_custom_encoding: b.ov_csv_custom_encoding,
            ov_csv_custom_delimiter: b.ov_csv_custom_delimiter,
            ov_csv_custom_quote: b.ov_csv_custom_quote,
            ov_csv_trim: b.ov_csv_trim,
            ov_csv_local: b.ov_csv_local,
            ov_csv_types: b.ov_csv_types,
            ov_csv_defaults: b.ov_csv_defaults,
            ov_csv_missing: b.ov_csv_missing,
            ov_csv_conflict: b.ov_csv_conflict,
            ov_csv_advanced: b.ov_csv_advanced,
            ov_csv_refresh: b.ov_csv_refresh,
            ov_csv_preview: b.ov_csv_preview,
            ov_csv_preview_error: b.ov_csv_preview_error,
            ov_csv_scroll: b.ov_csv_scroll,
            ov_csv_paste: b.ov_csv_paste,
            ov_attach_path: b.ov_attach_path,
            ov_attach_schema: b.ov_attach_schema,
            ov_find: b.ov_find,
            ov_replace: b.ov_replace,
            ov_find_options: b.ov_find_options,
            ov_btn_find_all: b.ov_btn_find_all,
            ov_btn_find: b.ov_btn_find,
            ov_btn_repl: b.ov_btn_repl,
            ov_btn_repl_all: b.ov_btn_repl_all,
            ov_cipher_pass: b.ov_cipher_pass,
            ov_cipher_page: b.ov_cipher_page,
            ov_cipher_kdf: b.ov_cipher_kdf,
            ov_prefs_font: b.ov_prefs_font,
            ov_prefs_theme: b.ov_prefs_theme,
            ov_prefs_confirm: b.ov_prefs_confirm,
            ov_prefs_completion: b.ov_prefs_completion,
            ov_prefs_upper_keywords: b.ov_prefs_upper_keywords,
            ov_prefs_delim: b.ov_prefs_delim,
            ov_frozen: b.ov_frozen,
            ov_cond_col: b.ov_cond_col,
            ov_cond_op: b.ov_cond_op,
            ov_cond_val: b.ov_cond_val,
            ov_cond_color: b.ov_cond_color,
            btn_sql_line: b.btn_sql_line,
            btn_sql_comment: b.btn_sql_comment,
            btn_sql_view: b.btn_sql_view,
            sql_view_name: b.sql_view_name,
            btn_edit_text: b.btn_edit_text,
            btn_edit_hex: b.btn_edit_hex,
            btn_edit_json: b.btn_edit_json,
            btn_edit_image: b.btn_edit_image,
            btn_fk_nav: b.btn_fk_nav,
            btn_freeze: b.btn_freeze,
            btn_cond_fmt: b.btn_cond_fmt,
            toolbar_extra: b.toolbar_extra,
            toolbar_project: b.toolbar_project,
            btn_project_save: b.btn_project_save,
            btn_integrity: b.btn_integrity,
            ov_ext_path: b.ov_ext_path,
            ov_ext_entry: b.ov_ext_entry,
            btn_form_view: b.btn_form_view,
            btn_form_prev: b.btn_form_prev,
            btn_form_next: b.btn_form_next,
            form_fields: b.form_fields.clone(),
            image_fit: b.image_fit,
            image_actual: b.image_actual,
            image_in: b.image_in,
            image_out: b.image_out,
            image_export: b.image_export,
            edit_image: b.edit_image,
            remote_list: b.remote_list,
            remote_draft: b.remote_draft,
            btn_remote_add: b.btn_remote_add,
            btn_remote_open: b.btn_remote_open,
            log_area: b.log_area,
            log_filter: b.log_filter,
            log_clear: b.log_clear,
            log_export: b.log_export,
        }
    }
}

impl Demo {
    fn new(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let mut font = if cfg!(target_os = "macos") {
            Font::from_path("/System/Library/Fonts/STHeiti Medium.ttc", 1).or_else(|_| load_default_cjk_font())
        } else { load_default_cjk_font() }.unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });
        if cfg!(target_os = "macos") {
            if let Ok(symbols) = Font::from_path("/System/Library/Fonts/Apple Symbols.ttf", 0) { font = font.with_fallback(symbols); }
        }
        let mut state = AppState::new_with_demo();
        let theme = ui_build::browser_theme(if state.prefs.default_theme_dark { Theme::dark() } else { Theme::light() }, state.prefs.font_size);
        if let Ok(tab) = std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB") {
            state.tab = match tab.to_ascii_lowercase().as_str() {
                "browse" | "header-menu" => MainTab::Browse,
                "pragmas" | "pragmas-bottom" | "pragmas-confirm" | "pragmas-hold" => MainTab::Pragmas,
                "sql" | "execute" | "sql-long" | "sql-calltip" | "sql-word" => MainTab::ExecuteSql,
                "wizard" | "table" | "table-types" => {
                    state.open_create_table_wizard();
                    if tab == "table-types" {
                        state.wizard_cols[1].type_name = "DECIMAL(10,2)".into();
                        state.wizard_cols.push(db::ColDef::new("untyped", ""));
                        state.wizard_col_sel = 1;
                    }
                    MainTab::Structure
                }
                "table-rename" | "table-renamed" => {
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "customers" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].name = "customer_key".into();
                    if tab == "table-renamed" {
                        state.apply_table_wizard().unwrap();
                        MainTab::Browse
                    } else { MainTab::Structure }
                }
                "table-rebuild" | "table-rebuilt" => {
                    state.new_memory_db().unwrap();
                    db::seed_demo(state.conn.as_ref().unwrap()).unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "customers" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[3].type_name = "VARYING CHARACTER(48)".into();
                    state.wizard_cols[3].notnull = true;
                    state.wizard_cols[3].unique = true;
                    state.wizard_cols[3].default_value = "'Unknown city'".into();
                    let column = state.wizard_cols.remove(3);
                    let source = state.wizard_col_sources.remove(3);
                    state.wizard_cols.insert(0, column);
                    state.wizard_col_sources.insert(0, source);
                    if tab == "table-rebuilt" {
                        state.apply_table_wizard().unwrap();
                        MainTab::Browse
                    } else { MainTab::Structure }
                }
                "table-key" | "table-keyed" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(label TEXT,revision INTEGER,note TEXT); INSERT INTO records(rowid,label,revision,note) VALUES(7,'first',1,'kept'),(9,'second',2,'kept');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].pk = true;
                    state.wizard_cols[1].pk = true;
                    state.wizard_col_sel = 1;
                    if tab == "table-keyed" { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-ai" | "table-auto" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id TEXT,code TEXT PRIMARY KEY,note TEXT); INSERT INTO records(rowid,id,code,note) VALUES(7,NULL,'first','assigned ID'),(9,'2','second','kept ID');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].pk = true;
                    state.wizard_cols[0].auto_increment = true;
                    state.wizard_cols[0].type_name = "INTEGER".into();
                    state.wizard_cols[1].pk = false;
                    if tab == "table-auto" {
                        state.apply_table_wizard().unwrap();
                        MainTab::Browse
                    } else { MainTab::Structure }
                }
                "table-conflict" | "table-conflicted" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(code TEXT CONSTRAINT named_key PRIMARY KEY ON CONFLICT FAIL,note TEXT); INSERT INTO records VALUES('first','kept'),('second','key conflict policy');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_table_options.primary_key_conflict = Some(sqlite3_parser::ast::ResolveType::Replace);
                    if tab == "table-conflicted" { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-keys" | "table-keys-saved" | "table-keys-grid" | "table-keys-editor" | "table-key-type" | "table-key-columns" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,customer TEXT CONSTRAINT customer_unique UNIQUE ON CONFLICT FAIL,region TEXT,CONSTRAINT customer_region UNIQUE(customer,region)); INSERT INTO records VALUES(7,'Northwind','west'),(11,'Contoso','east');").unwrap();
                    state.refresh_schema().unwrap();
                    let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                    state.wizard_index_constraints[2].columns="customer COLLATE NOCASE ASC, region DESC".into();
                    state.wizard_index_constraints[2].conflict=Some(sqlite3_parser::ast::ResolveType::Abort);
                    state.wizard_table_options.index_constraints=Some(state.wizard_index_constraints.clone());
                    state.wizard_cols[1].name="customer_name".into(); state.wizard_table_name="orders".into(); state.sync_table_constraint_names().unwrap();
                    state.wizard_table_page=1; state.wizard_key_sel=2; state.wizard_key_scroll=(0.0,if tab.ends_with("-grid") || tab.ends_with("-editor") || tab=="table-key-type" || tab=="table-key-columns" {0.0} else {10000.0});
                    if tab.ends_with("-saved") { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-inline-foreign-keys" | "table-inline-foreign-keys-saved" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1),(2); CREATE TABLE other_parent(id INTEGER PRIMARY KEY); INSERT INTO other_parent VALUES(1),(2); CREATE TABLE records(id INTEGER PRIMARY KEY,p INTEGER CONSTRAINT first_parent REFERENCES parent(id) CHECK(p>0) CONSTRAINT deadline DEFERRABLE INITIALLY DEFERRED NOT NULL CONSTRAINT second_parent REFERENCES other_parent(id),q INTEGER REFERENCES records(id) DEFERRABLE INITIALLY DEFERRED,FOREIGN KEY(q) REFERENCES records(id)); INSERT INTO records VALUES(7,1,NULL),(11,2,7);").unwrap();
                    state.refresh_schema().unwrap(); let row=state.schema.iter().position(|object|object.name=="records").unwrap();
                    state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                    state.wizard_foreign_keys[0].name=Some("customer_parent".into());
                    state.wizard_foreign_keys[0].clause="REFERENCES parent(id)\nCONSTRAINT customer_deadline DEFERRABLE INITIALLY DEFERRED".into();
                    state.wizard_foreign_keys[1].clause="REFERENCES other_parent(id) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED".into();
                    state.store_wizard_foreign_keys(); state.wizard_cols[1].name="parent_value".into(); state.wizard_table_name="orders".into(); state.sync_table_constraint_names().unwrap();
                    state.wizard_table_page=2; state.wizard_foreign_key_scroll=(0.0,if tab.ends_with("-grid") || tab.ends_with("-editor") {0.0} else {10000.0});
                    if tab.ends_with("-saved") { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-foreign-keys" | "table-foreign-keys-saved" | "table-foreign-keys-grid" | "table-foreign-keys-editor" | "table-fk-columns" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE customers(id INTEGER,region TEXT,PRIMARY KEY(id,region)); INSERT INTO customers VALUES(1,'west'),(2,'east'); CREATE TABLE records(id INTEGER PRIMARY KEY,customer INTEGER,region TEXT,CONSTRAINT customer_region FOREIGN KEY(customer,region) REFERENCES customers(id,region)); INSERT INTO records VALUES(7,1,'west'),(11,2,'east');").unwrap();
                    state.refresh_schema().unwrap();
                    let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                    state.wizard_foreign_keys[0].clause="REFERENCES customers(id,region)\nON UPDATE CASCADE ON DELETE SET NULL\nDEFERRABLE INITIALLY DEFERRED".into();
                    state.wizard_table_options.foreign_keys=Some(state.wizard_foreign_keys.clone());
                    state.wizard_cols[1].name="customer_id".into(); state.wizard_table_name="orders".into(); state.sync_table_constraint_names().unwrap();
                    state.wizard_table_page=2; state.wizard_foreign_key_scroll=(0.0,if tab.ends_with("-grid") || tab.ends_with("-editor") || tab=="table-fk-columns" {0.0} else {10000.0});
                    if tab.ends_with("-saved") { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-move" | "table-move-saved" | "table-move-views" | "table-move-views-saved" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS analytics;
                        CREATE TABLE main.records(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL CHECK(length(name)>0),amount REAL DEFAULT 0,total REAL AS(amount*1.2) STORED);
                        INSERT INTO main.records(id,name,amount) VALUES(7,'First customer',12.5),(11,'Second customer',24);
                        CREATE INDEX main.by_name ON records(name COLLATE NOCASE);").unwrap();
                    if tab.contains("-views") {state.conn.as_ref().unwrap().execute_batch("CREATE VIEW main.visible(id,display,amount,total) AS SELECT id,name,amount,total FROM main.records;CREATE VIEW main.customer_summary AS SELECT display,amount,total FROM main.visible;CREATE TRIGGER main.edit_visible INSTEAD OF UPDATE ON visible BEGIN UPDATE records SET name=new.display,amount=new.amount WHERE id=old.id;END;").unwrap();}
                    state.refresh_schema().unwrap();let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records").unwrap();state.select_schema_node(10_000+row as u64);state.open_modify_table_wizard().unwrap();state.wizard_schema="analytics".into();state.wizard_table_name="customer_archive".into();state.wizard_scroll=(0.0,0.0);state.wizard_table_advanced=true;
                    if tab.contains("-views") {state.wizard_cols[1].name="label".into();state.sync_table_constraint_names().unwrap();}
                    if tab.ends_with("-saved") {state.apply_table_wizard().unwrap();}
                    if tab=="table-move-views-saved" {state.switch_browse_table_in("analytics","visible").unwrap();MainTab::Browse}else{MainTab::Structure}
                }
                "csv-batch" | "csv-batch-separate" | "csv-batch-confirm" | "csv-progress" | "csv-canceling" => {
                    state.new_memory_db().unwrap();
                    let paths=std::env::var_os("NEXT_GUI_DB_BROWSER_QA_CSV_PATHS").expect("CSV batch QA fixture paths");
                    assert!(state.open_csv_files(std::env::split_paths(&paths).collect()));
                    state.csv_has_header=true;
                    state.csv_files.as_mut().unwrap().separate=tab.ends_with("-separate");
                    if matches!(tab.as_str(), "csv-progress" | "csv-canceling") {
                        let conn = state.conn.as_ref().unwrap();
                        conn.create_scalar_function("qa_csv_pause", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, |_| {
                            std::thread::sleep(std::time::Duration::from_secs(30));
                            Ok(0)
                        }).unwrap();
                        conn.execute_batch("CREATE TABLE customers(id INTEGER,name TEXT,amount REAL);CREATE TRIGGER qa_pause AFTER INSERT ON customers WHEN new.id=1 BEGIN SELECT qa_csv_pause();END").unwrap();
                        state.start_csv_import_work().expect("CSV progress QA source fixtures");
                    } else if tab.ends_with("-confirm") {
                        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE customers(id INTEGER,name TEXT,amount REAL)").unwrap();
                        state.start_csv_import_work().expect("CSV batch QA source fixtures");
                    }
                    MainTab::Structure
                }
                "csv-encoding" | "csv-encoding-other" | "csv-preview" | "csv-preview-advanced" | "csv-preview-confirm" => {
                    state.new_memory_db().unwrap();state.csv_path=std::env::var("NEXT_GUI_DB_BROWSER_QA_CSV_PATH").unwrap_or_else(|_|"customers.csv".into());state.csv_table="customers".into();state.csv_has_header=true;state.csv_encoding_idx=if tab.starts_with("csv-preview") {0} else if tab.ends_with("-other") {3} else {2};state.csv_advanced=tab.ends_with("-advanced");state.csv_custom_encoding="Shift_JIS".into();state.overlay=Some(OverlayKind::ImportCsv);if tab.ends_with("-confirm") {state.conn.as_ref().unwrap().execute_batch("CREATE TABLE customers(id INTEGER, name TEXT, amount REAL)").unwrap();state.csv_has_header=true;state.start_csv_import_work().expect("CSV QA source fixture");}MainTab::Structure
                }
                "table-fields" | "table-fields-references" | "table-field-checks" | "table-field-editor" | "table-field-type-dropdown" | "table-field-collation-dropdown" | "table-field-strict-dropdown" | "table-field-reference-editor" | "table-field-reference-empty" | "table-field-check-editor" | "table-field-remove-confirm" | "table-advanced" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL UNIQUE DEFAULT 'customer' CHECK(length(name)>0) COLLATE NOCASE,parent_id INTEGER REFERENCES parent(id) ON UPDATE CASCADE ON DELETE SET NULL,amount REAL DEFAULT 0 CHECK(amount>=0),total REAL AS(amount*1.2) STORED); INSERT INTO records(name,parent_id,amount) VALUES('First customer',1,12.5),('Second customer',1,24);").unwrap();state.refresh_schema().unwrap();let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records").unwrap();state.select_schema_node(10_000+row as u64);state.open_modify_table_wizard().unwrap();state.wizard_col_sel=1;state.wizard_scroll=(0.0,180.0);state.wizard_field_column=Some(if tab=="table-fields-references" {9} else {0});state.wizard_field_scroll=(if tab=="table-fields-references" {10000.0} else {0.0},0.0);
                    if tab=="table-field-checks" || tab=="table-field-editor" {state.wizard_scroll=(0.0,0.0);}
                    if tab=="table-field-editor" {state.wizard_field_edit=Some(state::FieldCellEdit {column:0,original:"name".into(),reference:None,input:None,autocomplete:None});}
                    if matches!(tab.as_str(),"table-field-type-dropdown"|"table-field-collation-dropdown"|"table-field-strict-dropdown") {
                        let column=if tab=="table-field-collation-dropdown" {8} else {1};state.wizard_scroll=(0.0,0.0);state.wizard_field_column=Some(column);
                        if tab=="table-field-strict-dropdown" {state.wizard_table_options.strict=true;}
                        let original=if column==1 {state.wizard_cols[1].type_name.clone()} else {state.wizard_cols[1].collation.clone()};
                        state.wizard_field_edit=Some(state::FieldCellEdit {column,original,reference:None,input:None,autocomplete:None});
                    }
                    if tab=="table-field-remove-confirm" {state.wizard_scroll=(0.0,0.0);state.wizard_col_sel=0;state.confirm_kind=ConfirmKind::DeleteColumn(0);state.confirm_open=true;}
                    if tab=="table-field-check-editor" {state.wizard_scroll=(0.0,0.0);state.wizard_field_column=Some(7);state.wizard_field_edit=Some(state::FieldCellEdit {column:7,original:state.wizard_cols[1].check_expression.clone(),reference:None,input:None,autocomplete:None});}
                    if tab=="table-field-reference-editor" || tab=="table-field-reference-empty" {
                        state.wizard_col_sel=if tab.ends_with("-empty") {1} else {2};state.wizard_field_column=Some(9);state.wizard_scroll=(0.0,0.0);
                        let original=state.wizard_cols[state.wizard_col_sel].foreign_key_clause.clone();let reference=reference_editor::Draft::new(&state,&original).ok();
                        state.wizard_field_edit=Some(state::FieldCellEdit {column:9,original,reference,input:None,autocomplete:None});
                    }
                    if tab=="table-advanced" { state.wizard_table_advanced=true; state.wizard_table_page=2; }
                    MainTab::Structure
                }
                "table-inline-checks" | "table-inline-checks-saved" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,v INTEGER CONSTRAINT positive CHECK(v>0) NOT NULL CONSTRAINT ceiling CHECK(v<10),note TEXT CONSTRAINT note_check CHECK(length(note)>0),g INTEGER AS(v*2) STORED,CONSTRAINT table_check CHECK(g<40)); INSERT INTO records(id,v,note) VALUES(7,2,'first check'),(11,4,'second check');").unwrap();
                    state.refresh_schema().unwrap();let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64);state.open_modify_table_wizard().unwrap();
                    state.wizard_checks[1].name=Some("positive_amount".into());state.wizard_checks[1].expression="v > 1\nAND v != 7".into();state.wizard_checks[2].expression="v < 20".into();state.store_wizard_checks();
                    state.wizard_cols[1].name="amount".into();state.wizard_table_name="orders".into();state.sync_table_constraint_names().unwrap();
                    state.wizard_col_sel=1;state.wizard_check_sel=1;state.wizard_table_page=3;state.wizard_check_scroll=(0.0,10000.0);
                    if tab=="table-inline-checks-saved" {state.apply_table_wizard().unwrap();}
                    MainTab::Structure
                }
                "table-checks" | "table-checks-saved" | "table-checks-renamed" | "table-checks-renamed-saved" | "table-checks-grid" | "table-checks-editor" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(v INTEGER CONSTRAINT positive CHECK(v>0),note TEXT,CONSTRAINT limit_value CHECK(v<10),CHECK(length(note)>0)); INSERT INTO records VALUES(2,'first'),(4,'second');").unwrap();
                    state.refresh_schema().unwrap();
                    let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                    state.wizard_checks[0]=table_sql::NamedCheck { column:None, name:Some("range_value".into()),expression:"v < 20\nAND v != 11".into() };
                    state.store_wizard_checks(); state.wizard_table_page=3;
                    state.wizard_check_scroll=(0.0,if tab.ends_with("-grid") || tab.ends_with("-editor") {0.0} else {10000.0});
                    if tab.starts_with("table-checks-renamed") {
                        state.wizard_cols[0].name="amount".into(); state.wizard_table_name="checked.名".into();
                        state.sync_table_constraint_names().unwrap();
                    }
                    if tab.ends_with("-saved") { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-preview" | "table-preview-bottom" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(qty INTEGER DEFAULT 2,total INTEGER CONSTRAINT generation AS(qty*4) STORED,note TEXT CONSTRAINT note_check CHECK(length(note)>0)); INSERT INTO records(qty,note) VALUES(3,'kept');").unwrap();
                    state.refresh_schema().unwrap();
                    let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].name="amount".into(); state.wizard_cols[0].notnull=true;
                    state.wizard_cols[0].default_value="4".into(); state.wizard_table_name="prepared".into();
                    state.wizard_table_options.strict=true;
                    if tab=="table-preview-bottom" { state.wizard_scroll=(0.0,10000.0); }
                    MainTab::Structure
                }
                "table-generated" | "table-computed" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(qty INTEGER,total INTEGER CONSTRAINT generation AS(qty*4) STORED,note TEXT); INSERT INTO records(qty,note) VALUES(3,'first calculation'),(5,'second calculation');").unwrap();
                    state.refresh_schema().unwrap();
                    let row=state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                    state.wizard_col_sel=1;
                    state.wizard_cols[1].generated=Some(dbmod::GeneratedColumn { expression:"qty * 5\n+ 1".into(),stored:false });
                    state.wizard_scroll=(0.0,10000.0);
                    if tab=="table-computed" {
                        state.apply_table_wizard().unwrap(); state.goto_browse_record(1).unwrap();
                        state.browse_mut().sel_col=Some(1); state.sync_edit_cell_from_selection();
                        MainTab::Browse
                    } else { MainTab::Structure }
                }
                "table-foreign-key" | "table-referenced" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1),(2); CREATE TABLE other_parent(id INTEGER PRIMARY KEY); INSERT INTO other_parent VALUES(1),(2); CREATE TABLE records(value INTEGER CONSTRAINT named_fk REFERENCES parent(id),note TEXT); INSERT INTO records VALUES(1,'first relation'),(2,'second relation');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema=="main" && o.name=="records" && o.kind==ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000+row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].foreign_key_clause="REFERENCES other_parent(id)\nON UPDATE CASCADE ON DELETE SET NULL\nDEFERRABLE INITIALLY DEFERRED".into();
                    state.wizard_scroll=(0.0,10000.0);
                    if tab=="table-referenced" { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-options" | "table-optioned" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(code TEXT PRIMARY KEY,amount INTEGER,note TEXT); INSERT INTO records VALUES('first',10,'kept'),('second',20,'strict values');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_table_advanced=true; state.wizard_table_options = table_sql::TableOptions { strict:true, without_rowid:true, primary_key_conflict:None, checks:None, inline_checks:None, foreign_keys:None, inline_foreign_keys:None, index_constraints:None };
                    if tab == "table-optioned" { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-check" | "table-checked" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(value TEXT CONSTRAINT valid_value CHECK(length(value)>0),note TEXT); INSERT INTO records VALUES('Zulu','last'),('alpha','first');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].check_expression = "length(value) >= 3\nAND value != 'blocked'".into();
                    state.wizard_scroll = (0.0, 10000.0);
                    if tab == "table-checked" { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "table-collation" | "table-collated" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(value TEXT UNIQUE,note TEXT); INSERT INTO records VALUES('Zulu','last with NOCASE'),('alpha','first with NOCASE');").unwrap();
                    state.refresh_schema().unwrap();
                    let row = state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
                    state.select_schema_node(10_000 + row as u64);
                    state.open_modify_table_wizard().unwrap();
                    state.wizard_cols[0].collation = "NOCASE".into();
                    if tab == "table-collated" { state.apply_table_wizard().unwrap(); }
                    MainTab::Structure
                }
                "index" => {
                    state.open_create_index_wizard();
                    MainTab::Structure
                }
                "csv" | "paste" | "attach" | "find" | "extension" => {
                    state.overlay = Some(match tab.as_str() {
                        "csv" => OverlayKind::ImportCsv,
                        "paste" => OverlayKind::PasteCsv,
                        "attach" => OverlayKind::Attach,
                        "find" => OverlayKind::FindReplace,
                        _ => OverlayKind::LoadExtension,
                    });
                    MainTab::Structure
                }
                "long-confirm" | "long-confirm-bottom" => {
                    state.new_memory_db().unwrap();
                    let name="Quarterly database 日本語 'quoted' identifier ".repeat(80);
                    state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE {}(v);",db::quote_ident(&name))).unwrap();
                    state.refresh_schema().unwrap();
                    state.select_schema_node(10_000);
                    state.confirm_kind=ConfirmKind::DropObject;
                    state.confirm_open=true;
                    MainTab::Structure
                }
                "save-prompt" => {
                    *state.sql_editor_mut() = "SELECT 'unsaved draft';".into();
                    state.pending_document = Some(DocumentAction::Exit);
                    state.confirm_kind = ConfirmKind::Documents;
                    state.confirm_open = true;
                    MainTab::ExecuteSql
                }
                "ddl-prompt" => {
                    state.select_schema_node(10_000);
                    state.ddl_draft = "CREATE VIEW customer_summary AS SELECT * FROM customers;".into();
                    state.pending_document = Some(DocumentAction::SelectSchemaNode(10_001));
                    state.confirm_kind = ConfirmKind::Documents;
                    state.confirm_open = true;
                    MainTab::Structure
                }
                "image" => MainTab::Browse,
                "full-width" => {
                    state.dock_edit = false; state.dock_schema = false;
                    state.dock_log = false; state.dock_plot = false; state.dock_remote = false;
                    MainTab::Browse
                }
                "schema" => {
                    state.dock_focus = DockKind::Schema;
                    MainTab::Structure
                }
                "schema-namespaces" | "browse-namespaces" | "plot-namespaces" | "plot-caption" | "table-namespaces" | "index-namespaces" => {
                    state.new_memory_db().unwrap();
                    state.conn.as_ref().unwrap().execute_batch(SCHEMA_NAMESPACE_QA_SQL).unwrap();
                    state.refresh_schema().unwrap();
                    let i = state.schema.iter().position(|o| o.schema == "analytics" && o.name == "customers").unwrap();
                    state.select_schema_node(10_000 + i as u64);
                    state.dock_focus = DockKind::Schema;
                    if tab == "plot-namespaces" || tab == "plot-caption" {
                        state.plot.settings.manual = true;
                        state.conn.as_ref().unwrap().execute_batch("WITH RECURSIVE n(x) AS (VALUES(2) UNION ALL SELECT x+1 FROM n WHERE x<15)
                            INSERT INTO analytics.customers SELECT x,'APAC',100+x*12+((x%3)*30) FROM n;").unwrap();
                        state.refresh_browse().unwrap();
                        state.chart_schema = "analytics".into();
                        state.chart_table = Some("customers".into());
                        state.chart_column = "revenue".into();
                        if tab == "plot-caption" {
                            state.conn.as_ref().unwrap().execute_batch("CREATE VIEW analytics.\"客户季度营收分析\" AS SELECT revenue AS \"季度客户收入（含税）\" FROM customers;").unwrap();
                            state.refresh_schema().unwrap();
                            state.chart_table = Some("客户季度营收分析".into());
                            state.chart_column = "季度客户收入（含税）".into();
                        }
                        state.refresh_chart().unwrap();
                        state.dock_plot = true;
                        state.dock_focus = DockKind::Plot;
                    }
                    if tab == "table-namespaces" { state.open_modify_table_wizard().unwrap(); }
                    if tab == "index-namespaces" {
                        state.open_create_index_wizard();
                        state.wizard_index_cols = "lower(region) COLLATE NOCASE DESC,\nrevenue -- keep column comment".into();
                        state.wizard_index_where = "revenue >= 100\nAND region IS NOT NULL".into();
                    }
                    if matches!(tab.as_str(), "schema-namespaces" | "table-namespaces" | "index-namespaces") { MainTab::Structure } else { MainTab::Browse }
                }
                "schema-scroll" => {
                    state.new_memory_db().unwrap();
                    for i in 0..100 {
                        state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE t{i:03}(v);")).unwrap();
                    }
                    state.refresh_schema().unwrap();
                    state.select_schema_node(10_080);
                    MainTab::Structure
                }
                "frozen" => {
                    state.frozen_cols = 1;
                    state.browse_mut().column_widths = vec![240.0; 20];
                    state.browse_mut().scroll = (300.0, 0.0);
                    MainTab::Browse
                }
                "cipher" | "encrypt" => {
                    state.overlay = Some(OverlayKind::Cipher);
                    MainTab::Structure
                }
                "prefs" | "preferences" => {
                    state.sync_prefs_drafts();
                    state.overlay = Some(OverlayKind::Preferences);
                    MainTab::Structure
                }
                "exportsql" | "export" | "sql-export-virtual" | "sql-export-data" | "sql-export-progress" | "sql-export-canceling" => {
                    state.overlay = Some(OverlayKind::ExportSql);
                    MainTab::Structure
                }
                "cond" | "condformat" => {
                    state.overlay = Some(OverlayKind::CondFormat);
                    MainTab::Browse
                }
                "form" => {
                    state.browse_view = crate::state::BrowseViewMode::Form;
                    MainTab::Browse
                }
                "insert" => {
                    state.open_insert_row().unwrap();
                    MainTab::Browse
                }
                "remote" => {
                    state.dock_remote = true;
                    state.dock_focus = DockKind::Remote;
                    MainTab::Browse
                }
                "about" => {
                    state.overlay = Some(OverlayKind::About);
                    MainTab::Structure
                }
                // legacy aliases → docks / structure
                "charts" | "plot" => {
                    state.dock_plot = true;
                    state.dock_focus = DockKind::Plot;
                    MainTab::Browse
                }
                "log" => {
                    state.dock_log = true;
                    state.dock_focus = DockKind::Log;
                    state.sql_log_filter = 3;
                    let _ = state.run_sql_text_sync("SELECT '完整 SQL 日志 · Unicode';\nSELECT count(*) FROM customers;");
                    let _ = state.run_sql_text_sync("SELECT * FROM missing_log_example;");
                    state.error_msg = None;
                    MainTab::Structure
                }
                _ => MainTab::Structure,
            };
        }
        if std::env::var_os("NEXT_GUI_DB_BROWSER_QA").is_some() {
            if matches!(state.tab, MainTab::ExecuteSql) {
                state.sql_tabs[0] = "SELECT * FROM customers;".into();
                let _ = state.run_sql_sync();
            }
            if matches!(state.tab, MainTab::Browse) {
                let _ = state.refresh_browse();
                if matches!(state.browse_view, crate::state::BrowseViewMode::Form) {
                    if state.browse().sel_row.is_none() {
                        state.browse_mut().sel_row = Some(0);
                        state.browse_mut().sel_col = Some(0);
                    }
                    state.sync_edit_cell_from_selection();
                }
            }
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("image") {
            let pixels = image::RgbaImage::from_fn(320, 180, |x, y| {
                let v = if (x / 2 + y / 2) % 2 == 0 { 255 } else { 0 };
                image::Rgba([
                    v,
                    (x * 255 / 319) as u8,
                    (y * 255 / 179) as u8,
                    if x < 160 { 255 } else { 128 },
                ])
            });
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(pixels)
                .write_to(&mut bytes, image::ImageOutputFormat::Png)
                .unwrap();
            state.edit_original = rusqlite::types::Value::Blob(bytes.into_inner());
            state.edit_type = 4;
            state.edit_saved = state.edit_original.clone();
            state.edit_mode = crate::state::EditCellMode::Image;
            state.set_edit_mode(crate::state::EditCellMode::Image).unwrap();
        }
        let mut theme = if state.prefs.default_theme_dark {
            theme
        } else {
            Theme::light()
        };
        if let Ok(mode) = std::env::var("NEXT_GUI_DB_BROWSER_QA_THEME") {
            if mode.eq_ignore_ascii_case("light") {
                theme = Theme::light();
            }
        }
        if std::env::var_os("NEXT_GUI_DB_BROWSER_QA").is_some()
            && (state.overlay == Some(OverlayKind::FindReplace)
                || std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("sql-long"))
        {
            state.sql_result = None;
            state.sql_message = "Select SQL to execute, or run the whole script".into();
            state.sql_tabs[0] = (1..=80).map(|i| format!("SELECT {i}, '中文 match {i}';\n")).collect();
            state.find_text = "中文 match 40".into();
            state.replace_text = "東京".into();
        }
        let qa_calltip = std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("sql-calltip");
        let qa_word = std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("sql-word");
        if qa_word {
            state.sql_result = None;
            state.sql_tabs[0] = "SELECT 中文_name, other_name".into();
        }
        if qa_calltip {
            state.sql_result = None;
            state.sql_tabs[0] = format!("{}SELECT coalesce(\n    substr('中文 SQL editor', 1,", "-- Function argument hints\n".repeat(24));
        }
        if let Ok(tab)=std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB") {
            if let Some((page,row))=match tab.as_str() {"table-key-columns"=>Some((0,state.wizard_key_sel)),"table-fk-columns"=>Some((1,state.wizard_foreign_key_sel)),_=>None} {
                state.wizard_column_picker=Some(column_picker::Draft::new(&state,page,row).unwrap());
            }
            let target=match tab.as_str() {"table-keys-editor"=>Some((0,state.wizard_key_sel,2)),"table-key-type"=>Some((0,state.wizard_key_sel,1)),"table-foreign-keys-editor"=>Some((1,state.wizard_foreign_key_sel,2)),"table-checks-editor"=>Some((2,state.wizard_check_sel,0)),_=>None};
            if let Some((page,row,column))=target {
                let original=state.constraint_cell_value(page,row,column).unwrap().to_string();
                let original=if (page,column)==(0,1) {state::ConstraintCellOriginal::Index(state.wizard_index_constraints[row].clone())} else {state::ConstraintCellOriginal::Text(original)};
                let reference=if (page,column)==(1,2) {reference_editor::Draft::new(&state,&state.wizard_foreign_keys[row].clause).ok()} else {None};
                state.wizard_constraint_edit=Some(state::ConstraintCellEdit {page,row,column,original,input:None,reference});
            }
        }
        let theme = ui_build::browser_theme(theme, state.prefs.font_size);
        if std::env::var_os("NEXT_GUI_DB_BROWSER_QA_CSV_LOCAL").is_some()
            && matches!(state.overlay, Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv)) {
            state.csv_import_settings.local_conventions = true;
            state.csv_preview = None;
        }
        if matches!(state.overlay, Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv)) { state.refresh_csv_preview(); }
        if state.overlay==Some(OverlayKind::ExportSql) {
            if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref()==Ok("sql-export-virtual") {
                state.new_memory_db().unwrap();
                state.conn.as_ref().unwrap().execute_batch("CREATE VIRTUAL TABLE documents USING fts5(v,content=''); CREATE VIRTUAL TABLE locations USING rtree(id,x0,x1,y0,y1); INSERT INTO documents(rowid,v) VALUES(9007199254740993,'pending alpha'); INSERT INTO locations VALUES(8,1,2,3,4);").unwrap();
                state.refresh_schema().unwrap();state.overlay=Some(OverlayKind::ExportSql);
            }
            state.sql_export=Some(sql_export::Draft::new(state.conn.as_ref().unwrap(), &state.prefs.sql_export, None).unwrap());
            let qa_tab=std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").unwrap_or_default();
            if qa_tab=="sql-export-data" { state.sql_export.as_mut().unwrap().options.structure=false; }
            if matches!(qa_tab.as_str(),"sql-export-progress"|"sql-export-canceling") {
                let progress=std::sync::Arc::new(export_jobs::Progress::default());
                let observing=progress.clone();
                let (entered,waiting)=std::sync::mpsc::channel();
                let mut paused=false;
                state.conn.as_ref().unwrap().progress_handler(1,Some(move || {
                    if !paused && observing.counts().0>0 {
                        paused=true;let _=entered.send(());
                        std::thread::sleep(std::time::Duration::from_secs(30));
                    }
                    false
                }));
                let path=std::path::PathBuf::from(std::env::var_os("NEXT_GUI_EXPORT_QA_PATH").expect("SQL progress QA needs an owned output path"));
                let options=state.sql_export.as_ref().unwrap().options().unwrap();
                state.export_work=Some(export_jobs::Work {
                    job:export_jobs::Job::start_sql(state.conn.take().unwrap(),path,options.clone(),progress.clone()),
                    progress:progress.clone(),options:export_jobs::Options::Sql(options),count:1,
                });
                waiting.recv_timeout(std::time::Duration::from_secs(10)).expect("SQL progress QA never staged a row");
                if qa_tab=="sql-export-canceling" {progress.cancel();}
            }

        }
        if let Ok(tab)=std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB") {
            if matches!(tab.as_str(),"export-csv"|"export-json"|"query-export-csv"|"query-export-json") {
                state.new_memory_db().unwrap();db::seed_demo(state.conn.as_ref().unwrap()).unwrap();state.refresh_schema().unwrap();
                let mut draft=data_export::Draft::new(&state,tab.ends_with("json"));
                if tab.starts_with("query-") {
                    state.tab=MainTab::ExecuteSql;
                    state.sql_result=Some(db::execute_sql(state.conn.as_ref().unwrap(), "SELECT id, name FROM customers").unwrap());
                    draft.set_query(state.sql_result.as_ref().unwrap()).unwrap();
                }
                state.export_dialog=Some(draft);
            }
            if matches!(tab.as_str(), "plot-current-browse" | "plot-current-sql" | "plot-native-settings" | "plot-bars-grouped" | "plot-bars-stacked" | "plot-bars-grouped-narrow" | "plot-bars-stacked-narrow" | "docks-browse" | "docks-sql" | "docks-narrow" | "plot-hover" | "plot-hover-narrow" | "toolbar-browse" | "toolbar-browse-narrow" | "toolbar-sql" | "toolbar-sql-narrow" | "cell-structure" | "cell-pragmas" | "cell-structure-narrow" | "cell-pragmas-narrow") {
                state.new_memory_db().unwrap();
                state.conn.as_ref().unwrap().execute_batch(if tab.starts_with("plot-bars-") {BAR_PLOT_QA_SQL}else{CURRENT_PLOT_QA_SQL}).unwrap();
                state.refresh_schema().unwrap();
                state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
                state.tab = if tab.ends_with("sql") || tab == "toolbar-sql-narrow" { MainTab::ExecuteSql } else { MainTab::Browse };
                if state.tab == MainTab::ExecuteSql {
                    *state.sql_editor_mut() = "SELECT observed, revenue, expenses, region\nFROM quarterly_revenue\nORDER BY observed DESC;".into();
                    state.sql_result = Some(db::execute_sql(state.conn.as_ref().unwrap(), state.sql_editor()).unwrap());
                }
                state.plot.settings.manual = false;
                state.sync_current_plot().unwrap();
                for (row, axis) in [(1, 1), (2, 2), (3, if tab.starts_with("plot-bars-") {2}else{3})] { assert!(state.plot.check(row, axis, true)); }
                state.plot.settings.stacked_bars=tab.starts_with("plot-bars-stacked");
                state.plot.set_styles(1, 4);
                if tab == "plot-native-settings" {
                    let native = project_xml::decode(NATIVE_PLOT_QA_XML).unwrap();
                    state.plot.settings = native.plot_settings;
                    state.plot.invalidate();
                }
                state.sync_current_plot().unwrap();
                if tab.starts_with("docks-") || tab.starts_with("plot-hover") || tab.starts_with("toolbar-") || tab.starts_with("cell-") {
                    state.browse_mut().sel_row=Some(0);state.browse_mut().sel_col=Some(1);state.sync_edit_cell_from_selection();
                }
                state.body_ratio = if tab.starts_with("docks-") || tab.starts_with("plot-hover") || tab.starts_with("toolbar-") || tab.starts_with("cell-") {0.45}else{0.52};
                state.dock_focus = DockKind::Plot;
                state.dock_plot = true;
                if tab.starts_with("cell-structure") { state.tab = MainTab::Structure; }
                if tab.starts_with("cell-pragmas") { state.tab = MainTab::Pragmas; }
            }
            if matches!(tab.as_str(), "plot-all-browse" | "plot-all-sql" | "plot-all-progress" | "sql-results-end" | "sql-results-end-narrow" | "plot-image-clipboard" | "plot-image-clipboard-narrow") {
                state.new_memory_db().unwrap();
                state.conn.as_ref().unwrap().execute_batch(FULL_PLOT_QA_SQL).unwrap();
                let gate = std::sync::Arc::new(std::sync::Mutex::new(None::<std::sync::Arc<plot_load::Progress>>));
                let table = if tab == "plot-all-progress" {
                    let waiting = gate.clone();
                    state.conn.as_ref().unwrap().create_scalar_function("qa_plot_wait", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, move |_| {
                        let deadline = Instant::now() + std::time::Duration::from_secs(30);
                        while Instant::now() < deadline && !waiting.lock().unwrap().as_ref().is_some_and(|p| p.cancelled()) {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Ok(2501)
                    }).unwrap();
                    state.conn.as_ref().unwrap().execute_batch("CREATE VIEW gated AS SELECT sequence,CASE WHEN sequence=2501 THEN qa_plot_wait() ELSE measured END AS measured,reference,region FROM telemetry").unwrap();
                    "gated"
                } else { "telemetry" };
                state.refresh_schema().unwrap();
                state.switch_browse_table_in("main", table).unwrap();
                state.tab = if tab.ends_with("sql") || matches!(tab.as_str(), "sql-results-end" | "sql-results-end-narrow" | "plot-image-clipboard" | "plot-image-clipboard-narrow") { MainTab::ExecuteSql } else { MainTab::Browse };
                if state.tab == MainTab::ExecuteSql {
                    *state.sql_editor_mut() = "SELECT sequence,measured,reference,region\nFROM telemetry ORDER BY sequence DESC;".into();
                    state.sql_result = Some(db::execute_sql(state.conn.as_ref().unwrap(), state.sql_editor()).unwrap());
                }
                state.plot.settings.manual = false;
                state.sync_current_plot().unwrap();
                for (row, axis) in [(1, 1), (2, 2), (3, 3)] { state.plot.check(row, axis, true); }
                state.plot.set_styles(1, 0);
                state.sync_current_plot().unwrap();
                state.start_plot_load().unwrap();
                if tab == "plot-all-progress" { *gate.lock().unwrap() = Some(state.plot_work.as_ref().unwrap().progress.clone()); }
                else {
                    let deadline = Instant::now() + std::time::Duration::from_secs(10);
                    while state.plot_work.is_some() {
                        assert!(Instant::now() < deadline);
                        state.poll_plot_load().unwrap();
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                state.body_ratio = 0.52;
                state.dock_focus = DockKind::Plot;
                state.dock_plot = true;
                if tab.starts_with("sql-results-end") {
                    state.sql_selection = state::SqlSelection { rows: Some((2998, 2999)), columns: Some((0, 3)) };
                    state.sql_scroll = (0.0, 1_000_000.0);
                    state.sql_message = "OK — 3000 result rows · final rows selected".into();
                }
            }
            if matches!(tab.as_str(), "pragmas" | "pragmas-bottom" | "pragmas-confirm" | "pragmas-hold") {
                state.new_memory_db().unwrap();
                state.dock_edit = false;
                if tab == "pragmas-confirm" {
                    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE kept(v);INSERT INTO kept VALUES('pending')").unwrap();
                    state.dirty = true;
                    state.confirm_kind = ConfirmKind::Pragmas;
                    state.confirm_open = true;
                }
            }
            if matches!(tab.as_str(), "export-progress" | "export-canceling") {
                state.new_memory_db().unwrap();
                let conn = state.conn.as_ref().unwrap();
                conn.create_scalar_function("wait_export", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, |_| {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    Ok(2)
                }).unwrap();
                conn.execute_batch("CREATE VIEW export_sample AS SELECT 1 AS value UNION ALL SELECT wait_export()").unwrap();
                state.refresh_schema().unwrap();
                state.export_dialog = Some(data_export::Draft::new(&state, false));
                let path = std::path::PathBuf::from(std::env::var_os("NEXT_GUI_EXPORT_QA_PATH").expect("Export progress QA needs an owned output path"));
                let progress = std::sync::Arc::new(export_jobs::Progress::default());
                let options = data_export::Options::default();
                let outputs = vec![data_export::Output { schema: "main".into(), table: "export_sample".into(), path }];
                state.export_work = Some(export_jobs::Work {
                    job: export_jobs::Job::start(state.conn.take().unwrap(), outputs, None, false, options.clone(), true, progress.clone()),
                    progress, options: export_jobs::Options::Data(options), count: 1,
                });
            }
        }
        if let Err(error) = state.sync_current_plot() { state.error_msg = Some(error); }
        if state.dock_edit {
            if let Err(error) = state.sync_edit_cell_for_page() { state.error_msg = Some(error); }
        }
        let mut built = build_ui(font.clone(), theme, width, height, &state);
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref()==Ok("long-confirm-bottom") {
            built.ui.key_input(UiKey::End);built.ui.sync(&mut built.scene);
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("pragmas-bottom") {
            built.ui.set_scroll_offset(built.pragma_scroll, 0.0, 10000.0);
            built.ui.sync(&mut built.scene);
        }


        if let Some(edit)=&state.wizard_constraint_edit {if edit.reference.is_some() {built.reference_editor.focus(&mut built.ui,0);} else {built.ui.set_focus(Some(built.ov_constraint_editor));built.ui.key_input(if (edit.page,edit.column)==(0,1) {UiKey::Space} else {UiKey::SelectAll});}built.ui.sync(&mut built.scene);}
        if let Some(edit)=&state.wizard_field_edit {if edit.reference.is_some() {built.reference_editor.focus(&mut built.ui,0);} else {built.ui.set_focus(Some(built.ov_field_editor));built.ui.key_input(UiKey::SelectAll);if matches!(edit.column,1|8) {if built.ui.is_select(built.ov_field_editor) {built.ui.key_input(UiKey::Space);} else {built.ui.open_autocomplete_options(built.ov_field_editor);}}}built.ui.sync(&mut built.scene);}
        if std::env::var_os("NEXT_GUI_DB_BROWSER_QA").is_some() && !state.find_text.is_empty() {
            built.ui.select_text_area_range(built.sql_area, 0..0);
            built.ui.find_in_text_area(built.sql_area, &state.find_text);
            built.ui.set_focus(Some(built.sql_area));
            built.ui.sync(&mut built.scene);
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab| tab == "schema-scroll") {
            built.ui.reveal_tree_selection(built.tree);
            built.ui.set_focus(Some(built.tree));
            built.ui.sync(&mut built.scene);
        }
        if qa_word {
            let caret = state.sql_editor().chars().count();
            built.ui.set_focus(Some(built.sql_area));
            built.ui.select_text_area_range(built.sql_area, caret..caret);
            built.ui.key_input(UiKey::WordLeft);
            built.ui.key_input(UiKey::SelectWordRight);
            built.ui.sync(&mut built.scene);
        }
        if matches!(std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref(),Ok("plot-image-clipboard" | "plot-image-clipboard-narrow" | "plot-bars-grouped" | "plot-bars-stacked" | "plot-bars-grouped-narrow" | "plot-bars-stacked-narrow")) {
            built.ui.set_focus(Some(built.chart)); built.ui.sync(&mut built.scene);
        }
        if qa_calltip {
            let caret = state.sql_editor().chars().count();
            built.ui.set_focus(Some(built.sql_area));
            built.ui.select_text_area_range(built.sql_area, caret..caret);
            if let Some(hint) = state.sql_completion.calltip(state.sql_editor(), caret) {
                built.ui.set_text_area_hints(built.sql_area, hint.anchor, hint.items, hint.preferred);
            }
            built.ui.sync(&mut built.scene);
        }
        let clear = clear_color(&theme);
        let qa_exit = std::env::var_os("NEXT_GUI_DB_BROWSER_QA").is_some();
        let last_log_revision = state.sql_log.revision();
        let last_schema_names = state.schema_names.clone();
        Self {
            font,
            theme,
            state,
            ids: BuiltUiIds::from(&built),
            last_body_ratio: built.ui.splitter_ratio(built.body_splitter),
            last_structure_ratio: built.ui.splitter_ratio(built.structure_splitter),
            last_dock_ratio: built.ui.splitter_ratio(built.dock_splitter),
            last_plot_ratio: built.ui.splitter_ratio(built.plot_splitter),
            scene: built.scene,
            ui: built.ui,
            backend: Backend::Soft(SoftRenderer::new(width, height, clear)),
            gpu_stash: None,
            surface_gpu: None,
            using_gpu: false,
            window_backend,
            win_w: width,
            win_h: height,
            needs_paint: true,
            pending_native: matches!(window_backend, PresentBackend::GpuSurface),
            rebuild: false,
            qa_exit,
            qa_done: false,
            qa_stepper_started: false,
            last_tree_sel: None,
            last_schema_names,
            reveal_schema_selection: false,
            last_tab: 0,
            last_browse_sel: None,
            last_browse_row: None,
            last_log_revision,
            cell_clipboard: None,
            exit_requested: false,
            reveal_browse_selection: false,
            browse_header_column: None,
        }
    }

    fn snapshot_inputs(&mut self) {
        if self.state.export_dialog.is_some() {self.snapshot_data_export();return;}
        self.snapshot_column_picker();
        if self.ids.tree.0 != 0 {
            for (database, name) in self.last_schema_names.iter().enumerate() {
                let root = database as u64 * 5 + 1;
                let expanded = [0, 1, 2, 3, 4].map(|group| self.ui.tree_expanded(self.ids.tree, root + group).unwrap_or(false));
                self.state.schema_tree_expanded.insert(name.clone(), expanded);
            }
            if let Some(y) = self.ui.tree_scroll_offset(self.ids.tree) { self.state.schema_tree_scroll = y; }
        }
        if let Some(view) = self.ui.text_area_state(self.ids.log_area) { self.state.sql_log_view = Some(view); }
        if let Some(ratio) = self.ui.splitter_ratio(self.ids.body_splitter) {
            if self.state.any_dock_visible() && Some(ratio) != self.last_body_ratio { self.state.body_ratio = ratio; }
            self.last_body_ratio = Some(ratio);
        }
        if let Some(ratio) = self.ui.splitter_ratio(self.ids.structure_splitter) {
            if Some(ratio) != self.last_structure_ratio { self.state.structure_ratio = ratio; }
            self.last_structure_ratio = Some(ratio);
        }
        if let Some(value) = self.ui.text_input_value(self.ids.sql_view_name) { self.state.sql_view_name = value.to_string(); }
        if let Some(offset) = self.ui.scroll_offset(self.ids.page_scroll) { self.state.page_scroll[self.last_tab] = offset; }
        if let Some(ratio)=self.ui.splitter_ratio(self.ids.dock_splitter) {
            if Some(ratio)!=self.last_dock_ratio {self.state.dock_ratio=ratio;}
            self.last_dock_ratio=Some(ratio);
        }
        if let Some(ratio)=self.ui.splitter_ratio(self.ids.plot_splitter) {
            if Some(ratio)!=self.last_plot_ratio {self.state.plot_ratio=ratio;}
            self.last_plot_ratio=Some(ratio);
        }
        for (index,id) in self.ids.dock_scrolls.iter().enumerate() {
            if let Some(offset)=self.ui.scroll_offset(*id) {self.state.dock_scroll[index]=offset;}
        }
        if let Some(offset) = self.ui.scroll_offset(self.ids.shell_scroll) { self.state.shell_scroll = offset; }
        if let Some(view) = self.ui.image_view(self.ids.edit_image) { self.state.edit_image_view = view; }
        if let Some(widths) = self.ui.table_column_widths(self.ids.browse_table) {
            self.state.browse_mut().column_widths = widths;
            self.state.browse_mut().scroll =
                self.ui.table_scroll_offset(self.ids.browse_table).unwrap();
        }
        if let Some(widths) = self.ui.table_column_widths(self.ids.sql_table) {
            self.state.sql_column_widths = widths;
            self.state.sql_scroll = self.ui.table_scroll_offset(self.ids.sql_table).unwrap();
            let anchor = self.ui.table_selection_anchor(self.ids.sql_table);
            self.state.sql_selection.rows = anchor.zip(self.ui.table_selected_row(self.ids.sql_table)).map(|((row, _), end)| (row, end));
            self.state.sql_selection.columns = anchor.and_then(|(_, column)| column).zip(self.ui.table_selected_col(self.ids.sql_table));
        }
        if self.ids.ddl_area.0 != 0 {
            if let Some(v) = self.ui.text_area_value(self.ids.ddl_area) {
                self.state.ddl_draft = v.to_string();
            }
        }
        if self.ids.browse_filter.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.browse_filter) {
                self.state.browse_mut().filter = v.to_string();
            }
        }
        if let Some(value) = self.ui.text_input_value(self.ids.record_number) {
            self.state.browse_mut().goto_record = value.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.browse_global_filter) {
            self.state.browse_mut().global_filters = v.trim().split(' ').filter(|s| !s.is_empty()).map(String::from).collect();
        }
        for (i, id) in self.ids.browse_col_filters.iter().enumerate() {
            if id.0 == 0 {
                continue;
            }
            if let Some(v) = self.ui.text_input_value(*id) {
                let tab = self.state.browse_mut();
                if tab.col_filters.len() <= i {
                    tab.col_filters.resize(i + 1, String::new());
                }
                tab.col_filters[i] = v.to_string();
            }
        }
        if !self.state.edit_cell_read_only() {
            if self.ids.edit_col.0 != 0 {
                if let Some(v) = self.ui.text_input_value(self.ids.edit_col) {
                    self.state.edit_col = v.to_string();
                }
            }
            if let Some(t) = self.ui.select_selected(self.ids.edit_type) {
                self.state.edit_type = t;
            }
            if self.ids.edit_value.0 != 0 {
                if let Some(v) = self.ui.text_area_value(self.ids.edit_value) {
                    self.state.edit_value = v.to_string();
                }
            }
        }
        if self.ids.sql_area.0 != 0 {
            if let Some(v) = self.ui.text_area_value(self.ids.sql_area) {
                *self.state.sql_editor_mut() = v.to_string();
                self.state.sql_edit_states.resize(self.state.sql_tabs.len(), None);
                self.state.sql_edit_states[self.state.sql_tab_idx] = self.ui.text_area_state(self.ids.sql_area);
            }
        }
        for (index, button) in self.ids.sql_tab_btns.iter().enumerate() {
            self.ui.set_button_label(*button, self.state.sql_tab_label(index));
        }
        if let Some(plot)=self.ui.chart_plot(self.ids.chart,0) {
            if !self.state.plot.settings.manual && self.state.plot.matches_rendered(plot) {
                self.state.plot.data.state = plot.state.clone();
            } else if self.state.plot.settings.manual && plot.series.first().is_some_and(|series| std::sync::Arc::ptr_eq(&series.points,&self.state.chart_points)) {
                self.state.chart_state=plot.state.clone();
            }
        }
        if self.ids.chart_col.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.chart_col) {
                self.state.chart_column = v.to_string();
            }
        }
        if self.ids.tabs.0 != 0 {
            if let Some(i) = self.ui.tabs_selected(self.ids.tabs) {
                self.state.tab = MainTab::from_index(i);
            }
        }
        if self.ids.dock_tabs.0 != 0 {
            if let Some(i) = self.ui.tabs_selected(self.ids.dock_tabs) {
                let docks = self.state.visible_dock_tabs();
                if let Some(d) = docks.get(i) {
                    if self.state.dock_focus != *d {
                        self.state.dock_focus = *d;
                        self.rebuild = true;
                    }
                }
            }
        }
        for (i, (key, _)) in self.state.pragmas.clone().iter().enumerate() {
            let sw = self
                .ids
                .pragma_switches
                .get(i)
                .copied()
                .unwrap_or(WidgetId(0));
            let sel = self
                .ids
                .pragma_selects
                .get(i)
                .copied()
                .unwrap_or(WidgetId(0));
            let inp = self
                .ids
                .pragma_inputs
                .get(i)
                .copied()
                .unwrap_or(WidgetId(0));
            if self.state.pragma_drafts.len() <= i {
                self.state.pragma_drafts.resize(i + 1, String::new());
            }
            if sw.0 != 0 {
                if let Some(on) = self.ui.is_checked(sw) {
                    self.state.pragma_drafts[i] = if on { "1".into() } else { "0".into() };
                }
            } else if sel.0 != 0 {
                if let Some(si) = self.ui.select_selected(sel) {
                    let Some((value, _)) = pragmas::choices(key).get(si) else { continue; };
                    let val = *value;
                    self.state.pragma_drafts[i] = val.into();
                }
            } else if inp.0 != 0 {
                if let Some(v) = self.ui.text_input_value(inp) {
                    self.state.pragma_drafts[i] = v.to_string();
                }
            }
        }
        self.snapshot_overlay();
    }

    fn snapshot_overlay(&mut self) {
        let Some(kind) = self.state.overlay else {
            return;
        };
        if self.ids.ov_scroll.0 != 0 {
            if let Some(offset) = self.ui.scroll_offset(self.ids.ov_scroll) { self.state.wizard_scroll = offset; }
        }
        if self.ids.ov_name.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.ov_name) {
                match kind {
                    OverlayKind::EditTable => self.state.wizard_table_name = v.to_string(),
                    OverlayKind::EditIndex => self.state.wizard_index_name = v.to_string(),
                    _ => {}
                }
            }
        }
        match kind {
            OverlayKind::InsertRow => {
                for (i, id) in self.ids.insert_values.iter().enumerate() {
                    if let Some(value) = self.ui.text_input_value(*id) {
                        self.state.insert_values[i] = value.to_string();
                    }
                    if let Some(mode) = self.ui.select_selected(self.ids.insert_modes[i]) {
                        self.state.insert_modes[i] = mode;
                    }
                }
            }
            OverlayKind::EditTable => {
                self.snapshot_field_cell_editor();
                self.snapshot_constraint_cell_editor();
                for (index,table) in [self.ids.ov_keys_table,self.ids.ov_fks_table,self.ids.ov_checks_table].into_iter().enumerate() {
                    let view=&mut self.state.wizard_constraint_views[index];
                    if let Some(scroll)=self.ui.table_scroll_offset(table) {view.scroll=scroll;}
                    if let Some(widths)=self.ui.table_column_widths(table) {view.widths=widths;}
                    view.column=self.ui.table_selected_col(table);
                }
                if let Some(ratio)=self.ui.splitter_ratio(self.ids.ov_table_splitter) {self.state.wizard_table_ratio=ratio;}
                self.state.wizard_table_preview_state = self.ui.text_area_state(self.ids.ov_table_preview);
                self.state.wizard_table_name_state = self.ui.text_input_state(self.ids.ov_name);
                if let Some(page) = self.ui.tabs_selected(self.ids.ov_table_tabs) { self.state.wizard_table_page = page; }
                if let Some(offset) = self.ui.scroll_offset(self.ids.ov_checks_scroll) { self.state.wizard_check_scroll = offset; }
                if let Some(offset) = self.ui.scroll_offset(self.ids.ov_keys_scroll) { self.state.wizard_key_scroll = offset; }
                if let Some(selected) = self.ui.table_selected_row(self.ids.ov_keys_table) {
                    if selected != self.state.wizard_key_sel {
                        self.flush_table_index_constraint(); self.flush_table_check(); self.flush_table_foreign_key(); self.flush_wizard_col();
                        self.sync_table_constraint_names(); self.state.wizard_key_sel = selected;
                        self.rebuild = true; return;
                    }
                }
                self.flush_table_index_constraint();
                if let Some(offset) = self.ui.scroll_offset(self.ids.ov_fks_scroll) { self.state.wizard_foreign_key_scroll = offset; }
                if let Some(selected) = self.ui.table_selected_row(self.ids.ov_checks_table) {
                    if selected != self.state.wizard_check_sel {
                        self.flush_table_check();
                        self.flush_table_foreign_key();
                        self.flush_wizard_col();
                        self.sync_table_constraint_names();
                        self.state.wizard_check_sel = selected;
                        self.rebuild = true;
                        return;
                    }
                }
                if let Some(selected) = self.ui.table_selected_row(self.ids.ov_fks_table) {
                    if selected != self.state.wizard_foreign_key_sel {
                        self.flush_table_check(); self.flush_table_foreign_key(); self.flush_wizard_col();
                        self.sync_table_constraint_names(); self.state.wizard_foreign_key_sel = selected;
                        self.rebuild = true; return;
                    }
                }
                self.flush_table_check(); self.flush_table_foreign_key();
                if let Some(on) = self.ui.is_checked(self.ids.ov_without_rowid) { self.state.wizard_table_options.without_rowid = on; }
                if let Some(on) = self.ui.is_checked(self.ids.ov_strict) { self.state.wizard_table_options.strict = on; }
                if let Some(index) = self.ui.select_selected(self.ids.ov_pk_conflict) {
                    if let Some((_, action)) = table_sql::PRIMARY_KEY_CONFLICTS.get(index) {
                        if self.state.wizard_table_options.primary_key_conflict != *action {
                            self.state.wizard_table_options.primary_key_conflict = *action;
                            if let Some(key) = self.state.wizard_index_constraints.iter_mut().find(|key| key.primary) {
                                key.conflict = *action; self.state.wizard_table_options.index_constraints = Some(self.state.wizard_index_constraints.clone());
                                self.refresh_table_index_controls();
                            }
                        }
                    }
                }
                if self.ids.ov_schema.0 != 0 {
                    if let Some(index) = self.ui.select_selected(self.ids.ov_schema) {
                        if let Some(schema) = self.state.table_schemas().get(index) {
                            if *schema!=self.state.wizard_schema { self.state.wizard_schema=(*schema).into(); self.rebuild=true; }
                        }
                    }
                }
                if let Some(offset) = self.ui.table_scroll_offset(self.ids.ov_fields_table) { self.state.wizard_field_scroll = offset; }
                if let Some(widths) = self.ui.table_column_widths(self.ids.ov_fields_table) { self.state.wizard_field_widths = widths; }
                self.state.wizard_field_column = self.ui.table_selected_col(self.ids.ov_fields_table);
                if let Some(i) = self.ui.table_selected_row(self.ids.ov_fields_table) {
                    if i != self.state.wizard_col_sel {
                        self.flush_wizard_col();
                        self.sync_table_constraint_names();
                        self.state.wizard_col_sel = i;
                        self.rebuild = true;
                        return;
                    }
                }
                self.flush_wizard_col();
                self.sync_table_constraint_names();
                let sql = self.state.table_wizard_sql().unwrap_or_else(|error| format!("-- {error}"));
                if self.ui.text_area_value(self.ids.ov_table_preview) != Some(sql.as_str()) {
                    self.ui.set_text_area_value(self.ids.ov_table_preview, sql);
                    sql_syntax::apply(&mut self.ui, self.ids.ov_table_preview, &self.theme);
                }
            }
            OverlayKind::EditIndex => {
                for (index, area) in [self.ids.ov_idx_cols, self.ids.ov_idx_where, self.ids.ov_idx_preview].into_iter().enumerate() {
                    self.state.wizard_index_edit_states[index] = self.ui.text_area_state(area);
                }
                if let Some(value) = self.ui.text_area_value(self.ids.ov_idx_where) {
                    self.state.wizard_index_where = value.to_string();
                }
                for (index, list) in [self.ids.ov_list, self.ids.ov_idx_available].into_iter().enumerate() {
                    if let Some(offset) = self.ui.list_scroll_offset(list) { self.state.wizard_index_list_scroll[index] = offset; }
                }
                let previous_keys = self.state.wizard_index_cols.clone();
                let previous_key_sel = self.state.wizard_index_key_sel;
                let previous_table = (self.state.wizard_schema.clone(), self.state.wizard_index_table.clone());
                if let Some(value) = self.ui.text_area_value(self.ids.ov_idx_cols) {
                    self.state.wizard_index_cols = value.to_string();
                }
                if let Some(index) = self.ui.list_selected(self.ids.ov_list) { self.state.wizard_index_key_sel = index; }
                if let Some(index) = self.ui.list_selected(self.ids.ov_idx_available) { self.state.wizard_index_available_sel = index; }
                if self.ids.ov_idx_unique.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_idx_unique) {
                        self.state.wizard_index_unique = on;
                    }
                }
                if self.ids.ov_idx_table.0 != 0 {
                    if let Some(i) = self.ui.select_selected(self.ids.ov_idx_table) {
                        if let Some((schema, table)) = self.state.index_tables().get(i).map(|(schema, table)| (schema.to_string(), table.to_string())) {
                            if self.state.wizard_schema != schema || self.state.wizard_index_table != table {
                                self.state.wizard_schema = schema;
                                self.state.wizard_index_table = table;
                                self.state.wizard_index_available_sel = 0;
                                self.state.wizard_index_list_scroll[1] = 0.0;
                                self.rebuild = true;
                            }
                        }
                    }
                }
                if previous_keys == self.state.wizard_index_cols && previous_key_sel == self.state.wizard_index_key_sel
                    && previous_table == (self.state.wizard_schema.clone(), self.state.wizard_index_table.clone()) {
                    if let Some(order) = self.ui.select_selected(self.ids.ov_idx_order) {
                        if Some(order == 1) != self.state.index_wizard_key_order().ok() {
                            if let Err(error) = self.set_index_key_order(order == 1) { self.set_error(error); self.rebuild = true; }
                        }
                    }
                }
                self.sync_index_wizard(previous_keys != self.state.wizard_index_cols);
            }
            OverlayKind::ImportCsv | OverlayKind::PasteCsv => {
                if self.state.csv_work.is_some() { return; }
                let previous_options = self.state.csv_options();
                let previous_modes = (self.state.csv_delim_idx, self.state.csv_quote_idx, self.state.csv_encoding_idx);
                let previous_paste = self.state.csv_paste.clone();
                if let Some(offset) = self.ui.scroll_offset(self.ids.ov_csv_scroll) { self.state.csv_scroll = offset; }
                if self.ids.ov_csv_table.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_csv_table) {
                        self.state.csv_table = v.to_string();
                    }
                }
                if self.ids.ov_csv_header.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_csv_header) {
                        self.state.csv_has_header = on;
                    }
                }
                if self.ids.ov_csv_delim.0 != 0 {
                    if let Some(i) = self.ui.select_selected(self.ids.ov_csv_delim) {
                        self.state.csv_delim_idx = i;
                    }
                }
                if self.ids.ov_csv_quote.0 != 0 {
                    if let Some(i) = self.ui.select_selected(self.ids.ov_csv_quote) {
                        self.state.csv_quote_idx = i;
                    }
                }
                if let Some(index)=self.ui.select_selected(self.ids.ov_csv_encoding) {
                    if index!=self.state.csv_encoding_idx {self.state.csv_encoding_idx=index;self.rebuild=true;}
                }
                if let Some(value)=self.ui.autocomplete_value(self.ids.ov_csv_custom_encoding) {self.state.csv_custom_encoding=value.into();}
                if kind == OverlayKind::PasteCsv && self.ids.ov_csv_paste.0 != 0 {
                    if let Some(v) = self.ui.text_area_value(self.ids.ov_csv_paste) {
                        self.state.csv_paste = v.to_string();
                    }
                }
                for (id, setting) in [(self.ids.ov_csv_custom_delimiter, &mut self.state.csv_custom_delimiter), (self.ids.ov_csv_custom_quote, &mut self.state.csv_custom_quote)] {
                    if let Some(value) = self.ui.text_input_value(id) { *setting = value.into(); }
                }
                if let Some(value) = self.ui.is_checked(self.ids.ov_csv_trim) { self.state.csv_import_settings.trim_fields = value; }
                if let Some(value) = self.ui.is_checked(self.ids.ov_csv_local) { self.state.csv_import_settings.local_conventions = value; }
                if let Some(value) = self.ui.is_checked(self.ids.ov_csv_types) { self.state.csv_import_settings.detect_types = !value; }
                if let Some(value) = self.ui.is_checked(self.ids.ov_csv_defaults) { self.state.csv_import_settings.ignore_defaults = value; }
                if let Some(value) = self.ui.is_checked(self.ids.ov_csv_missing) { self.state.csv_import_settings.fail_on_missing = value; }
                if let Some(value) = self.ui.select_selected(self.ids.ov_csv_conflict) { self.state.csv_import_settings.conflict = match value { 1 => dbmod::CsvConflict::Ignore, 2 => dbmod::CsvConflict::Replace, _ => dbmod::CsvConflict::Abort }; }
                if self.state.csv_options() != previous_options || self.state.csv_paste != previous_paste { self.state.csv_preview = None; self.rebuild = true; }
                if previous_modes != (self.state.csv_delim_idx, self.state.csv_quote_idx, self.state.csv_encoding_idx) { self.rebuild = true; }
                self.snapshot_csv_files();
            }
            OverlayKind::FindReplace => {
                let mut flags = self.state.find_search.options.flags();
                for (flag, id) in flags.iter_mut().zip(self.ids.ov_find_options) {
                    if let Some(value) = self.ui.is_checked(id) { *flag = value; }
                }
                self.state.find_search.set_options(sql_search::Options::from_flags(flags),
                    self.ui.text_area_value(self.ids.sql_area).unwrap_or_default(),
                    self.ui.text_area_selection(self.ids.sql_area).unwrap_or(0..0));
                if self.ids.ov_find.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_find) {
                        self.state.find_text = v.to_string();
                    }
                }
                if self.ids.ov_replace.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_replace) {
                        self.state.replace_text = v.to_string();
                    }
                }
            }
            OverlayKind::Attach => {
                if self.ids.ov_attach_path.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_attach_path) {
                        self.state.attach_path = v.to_string();
                    }
                }
                if self.ids.ov_attach_schema.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_attach_schema) {
                        self.state.attach_schema = v.to_string();
                    }
                }
            }
            OverlayKind::Cipher => {
                if self.ids.ov_cipher_pass.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_cipher_pass) {
                        self.state.cipher_passphrase = v.to_string();
                    }
                }
                if self.ids.ov_cipher_page.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_cipher_page) {
                        self.state.cipher_page_size = v.to_string();
                    }
                }
                if self.ids.ov_cipher_kdf.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_cipher_kdf) {
                        self.state.cipher_kdf_iter = v.to_string();
                    }
                }
            }
            OverlayKind::Preferences => {
                if self.ids.ov_prefs_font.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_prefs_font) {
                        self.state.prefs_font_size = v.to_string();
                    }
                }
                if self.ids.ov_prefs_theme.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_prefs_theme) {
                        self.state.prefs_theme_dark = on;
                    }
                }
                if self.ids.ov_prefs_confirm.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_prefs_confirm) {
                        self.state.prefs_confirm_drops = on;
                    }
                }
                if let Some(on) = self.ui.is_checked(self.ids.ov_prefs_upper_keywords) {
                    self.state.prefs_sql_upper_keywords = on;
                }
                if let Some(on) = self.ui.is_checked(self.ids.ov_prefs_completion) {
                    self.state.prefs_sql_completion = on;
                }
                if self.ids.ov_prefs_delim.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_prefs_delim) {
                        self.state.prefs_csv_delim = v.to_string();
                    }
                }
                if self.ids.ov_frozen.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_frozen) {
                        if let Ok(n) = v.trim().parse::<usize>() {
                            self.state.prefs_frozen_cols = n;
                        }
                    }
                }
            }
            OverlayKind::ExportSql => { self.snapshot_sql_export(); }
            OverlayKind::LoadExtension => {
                if self.ids.ov_ext_path.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_ext_path) {
                        self.state.ext_path = v.to_string();
                    }
                }
                if self.ids.ov_ext_entry.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_ext_entry) {
                        self.state.ext_entry = v.to_string();
                    }
                }
            }
            OverlayKind::About => {}
            OverlayKind::CondFormat => {
                if self.ids.ov_cond_col.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_cond_col) {
                        self.state.cond_col = v.to_string();
                    }
                }
                if self.ids.ov_cond_op.0 != 0 {
                    if let Some(i) = self.ui.select_selected(self.ids.ov_cond_op) {
                        self.state.cond_op_idx = i;
                    }
                }
                if self.ids.ov_cond_val.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_cond_val) {
                        self.state.cond_value = v.to_string();
                    }
                }
                if self.ids.ov_cond_color.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_cond_color) {
                        self.state.cond_color = v.to_string();
                    }
                }
            }
        }
    }

    fn sync_table_constraint_names(&mut self) {
        match self.state.sync_table_constraint_names() {
            Ok(true) => {
                self.refresh_table_index_controls();
                self.refresh_table_check_controls();
                self.refresh_table_foreign_key_controls();
            }
            Ok(false) => {}
            Err(error) => {
                // Leave malformed SQL intact and reject the rename rather than guessing its references.
                if self.state.wizard_constraint_columns.len() == self.state.wizard_cols.len() {
                    for (column, name) in self.state.wizard_cols.iter_mut().zip(&self.state.wizard_constraint_columns) { column.name = name.clone(); }
                    if let Some(column) = self.state.wizard_cols.get(self.state.wizard_col_sel) { self.ui.set_text_input(self.ids.ov_col_name, column.name.clone()); }
                }
                self.state.wizard_table_name = self.state.wizard_constraint_table.clone();
                self.ui.set_text_input(self.ids.ov_name, self.state.wizard_table_name.clone());
                self.set_error(format!("Fix the constraint SQL before renaming: {error}"));
            }
        }
        if self.state.table_wizard_names_valid() && self.state.wizard_table_options.inline_foreign_keys.is_some()
            && self.state.sync_wizard_foreign_key_fields() { self.refresh_table_foreign_key_controls(); }
        if self.state.table_wizard_names_valid() && self.state.wizard_table_options.inline_checks.is_some()
            && self.state.sync_wizard_check_fields() { self.refresh_table_check_controls(); }
        self.ui.set_button_enabled(self.ids.ov_check_add_column, !self.state.wizard_cols.is_empty() && self.state.table_wizard_names_valid());
        self.ui.set_text_area_read_only(self.ids.ov_key_columns, self.state.wizard_index_constraints.is_empty() || !self.state.table_wizard_names_valid());
        self.ui.set_text_area_read_only(self.ids.ov_check_expr, self.state.wizard_checks.is_empty() || !self.state.table_wizard_names_valid());
        for area in [self.ids.ov_fk_columns, self.ids.ov_fk_clause] { self.ui.set_text_area_read_only(area, self.state.wizard_foreign_keys.is_empty() || !self.state.table_wizard_names_valid()); }
        self.refresh_table_field_rows();
    }

    fn refresh_table_field_rows(&mut self) {
        let offset = self.ui.table_scroll_offset(self.ids.ov_fields_table);
        self.ui.set_table_rows(self.ids.ov_fields_table,ui_build::table_field_rows(&self.state));
        self.ui.set_table_check_cells(self.ids.ov_fields_table,ui_build::table_field_checks(&self.state));
        if let Some((x,y)) = offset { self.ui.set_table_scroll_offset(self.ids.ov_fields_table,x,y); }
    }

    fn set_field_cell_value(&mut self,column:usize,value:&str) {
        match column {
            0=>{if self.ui.text_input_value(self.ids.ov_col_name)!=Some(value) {self.ui.set_text_input(self.ids.ov_col_name,value);}}
            1=>{if self.ui.autocomplete_value(self.ids.ov_col_type)!=Some(value) {self.ui.set_autocomplete_value(self.ids.ov_col_type,value);}}
            6=>{if self.ui.text_input_value(self.ids.ov_default)!=Some(value) {self.ui.set_text_input(self.ids.ov_default,value);}}
            8=>{if self.ui.autocomplete_value(self.ids.ov_collation)!=Some(value) {self.ui.set_autocomplete_value(self.ids.ov_collation,value);}}
            7=>{if self.ui.text_area_value(self.ids.ov_check)!=Some(value) {self.ui.set_text_area_value(self.ids.ov_check,value);sql_syntax::apply(&mut self.ui,self.ids.ov_check,&self.theme);}}
            9=>{if self.ui.text_area_value(self.ids.ov_foreign_key)!=Some(value) {self.ui.set_text_area_value(self.ids.ov_foreign_key,value);sql_syntax::apply(&mut self.ui,self.ids.ov_foreign_key,&self.theme);}}
            _=>{}
        }
    }

    fn snapshot_field_cell_editor(&mut self) {
        if self.ids.ov_field_editor.0==0 {return;}
        if self.state.wizard_field_edit.as_ref().is_some_and(|edit|matches!(edit.column,7|9)) {self.snapshot_field_sql(true);return;}
        let Some(edit)=&mut self.state.wizard_field_edit else {return;};
        let id=self.ids.ov_field_editor;
        edit.input=self.ui.text_input_state(id);edit.autocomplete=self.ui.autocomplete_state(id);
        let column=edit.column;
        let value=if column==1 || column==8 {self.ui.autocomplete_value(id).or_else(||self.ui.select_value(id))} else {self.ui.text_input_value(id)}.map(str::to_owned);
        if let Some(value)=value {self.set_field_cell_value(column,&value);}
        if self.ui.focused()!=Some(id) {self.state.wizard_field_edit=None;self.rebuild=true;}
    }

    fn snapshot_field_sql(&mut self,finish_on_blur:bool) {
        let Some(edit)=&mut self.state.wizard_field_edit else {return;};
        let column=edit.column;
        let (changed,value)=if let Some(draft)=&mut edit.reference {(draft.snapshot(&mut self.ui,self.ids.reference_editor),draft.sql())}
            else {edit.input=self.ui.text_input_state(self.ids.ov_field_editor);(false,self.ui.text_input_value(self.ids.ov_field_editor).unwrap_or(&edit.original).to_owned())};
        self.set_field_cell_value(column,&value);
        if finish_on_blur && !self.ui.focused().is_some_and(|id|self.ui.table_cell_editor_contains(self.ids.ov_fields_table,id)) {self.finish_field_sql(false);}
        else if changed {self.rebuild=true;}
    }

    fn finish_field_sql(&mut self,cancel:bool) {
        let Some(edit)=&self.state.wizard_field_edit else {return;};let column=edit.column;
        if cancel {let original=edit.original.clone();self.set_field_cell_value(column,&original);}
        self.flush_wizard_col();self.state.wizard_field_edit=None;
        let owner=self.state.wizard_constraint_columns.get(self.state.wizard_col_sel);
        if column==7 {
            if let Some(index)=self.state.wizard_checks.iter().position(|check|check.expression.trim().is_empty()
                && check.column.as_ref().zip(owner).is_some_and(|(column,owner)|column.eq_ignore_ascii_case(owner))) {
                self.state.wizard_checks.remove(index);self.state.wizard_check_states.remove(index);
                if self.state.wizard_check_sel>index {self.state.wizard_check_sel-=1;}
                self.state.wizard_check_sel=self.state.wizard_check_sel.min(self.state.wizard_checks.len().saturating_sub(1));
                self.state.store_wizard_checks();self.state.sync_wizard_check_fields();
                let check=self.state.wizard_checks.get(self.state.wizard_check_sel);
                self.ui.set_table_selected_row(self.ids.ov_checks_table,check.map(|_|self.state.wizard_check_sel));
                self.ui.set_text_input(self.ids.ov_check_name,check.and_then(|check|check.name.as_deref()).unwrap_or(""));
                self.ui.set_text_area_value(self.ids.ov_check_expr,check.map_or("",|check|check.expression.as_str()));
                self.refresh_table_check_controls();
            }
        } else if let Some(index)=self.state.wizard_foreign_keys.iter().position(|key|key.inline && key.clause.trim().is_empty()
            && owner.is_some_and(|owner|table_sql::inline_foreign_key_column(key).is_ok_and(|name|name.eq_ignore_ascii_case(owner)))) {self.remove_reference_key(index);}
        self.rebuild=true;
    }

    fn set_constraint_cell_value(&mut self,page:usize,column:usize,value:&str) {
        let input=match (page,column) {(0,2)=>self.ids.ov_key_name,(1,1)=>self.ids.ov_fk_name,(2,1)=>self.ids.ov_check_name,_=>WidgetId(0)};
        if input.0!=0 {
            if self.ui.text_input_value(input)!=Some(value) {self.ui.set_text_input(input,value);}
        } else {
            let area=match (page,column) {(1,2)=>self.ids.ov_fk_clause,(2,0)=>self.ids.ov_check_expr,_=>return};
            // shortcut: cross-view SQL edits reset linked form history; share wizard documents for full Undo.
            if self.ui.text_area_value(area)!=Some(value) {self.ui.set_text_area_value(area,value);sql_syntax::apply(&mut self.ui,area,&self.theme);}
        }
    }

    fn change_table_index_type(&mut self,row:usize,primary:bool) {
        let Some(current)=self.state.wizard_index_constraints.get(row) else {return;};
        if current.primary==primary {return;}
        let mut keys=self.state.wizard_index_constraints.clone();
        keys[row].primary=primary;
        if !primary {keys[row].auto_increment=false;}
        match table_sql::columns_with_index_constraints(&self.state.wizard_cols,&keys) {
            Ok(columns) => {
                self.state.wizard_cols=columns;self.state.wizard_index_constraints=keys;
                self.state.wizard_table_options.index_constraints=Some(self.state.wizard_index_constraints.clone());
                self.state.wizard_table_options.primary_key_conflict=self.state.wizard_index_constraints.iter().find(|key|key.primary).and_then(|key|key.conflict);
                self.refresh_table_index_controls();self.clear_error();
                self.state.status_msg=format!("Constraint type: {} (draft)",if primary {"PRIMARY KEY"} else {"UNIQUE"});
                self.ui.set_text(self.ids.status,ui_build::status_text(&self.state));
            }
            Err(error) => {
                self.ui.set_select_selected(self.ids.ov_constraint_editor,Some(usize::from(!self.state.wizard_index_constraints[row].primary)));
                self.set_error(error);self.rebuild=true;
            }
        }
    }

    fn remove_reference_key(&mut self,row:usize) {
        if row>=self.state.wizard_foreign_keys.len() {return;}
        let selected=self.ui.table_selected_row(self.ids.ov_fks_table).unwrap_or(self.state.wizard_foreign_key_sel);
        self.state.wizard_foreign_keys.remove(row);
        self.state.wizard_foreign_key_states.remove(row);
        self.state.wizard_foreign_key_sel=(selected-usize::from(selected>row)).min(self.state.wizard_foreign_keys.len().saturating_sub(1));
        self.ui.set_table_selected_row(self.ids.ov_fks_table,(!self.state.wizard_foreign_keys.is_empty()).then_some(self.state.wizard_foreign_key_sel));
        self.state.store_wizard_foreign_keys();self.state.sync_wizard_foreign_key_fields();
        let key=self.state.wizard_foreign_keys.get(self.state.wizard_foreign_key_sel);
        self.ui.set_text_input(self.ids.ov_fk_name,key.and_then(|key|key.name.as_deref()).unwrap_or(""));
        self.ui.set_text_area_value(self.ids.ov_fk_columns,key.map_or("",|key|key.columns.as_str()));
        self.ui.set_text_area_value(self.ids.ov_fk_clause,key.map_or("",|key|key.clause.as_str()));
        self.refresh_table_foreign_key_controls();self.rebuild=true;
    }

    fn snapshot_reference_cell(&mut self,finish_on_blur:bool) {
        let Some(edit)=&mut self.state.wizard_constraint_edit else {return;};
        let Some(draft)=&mut edit.reference else {return;};
        let changed=draft.snapshot(&mut self.ui,self.ids.reference_editor);
        let value=draft.sql();
        self.set_constraint_cell_value(1,2,&value);
        if finish_on_blur && !self.ui.focused().is_some_and(|id|self.ui.table_cell_editor_contains(self.ids.ov_fks_table,id)) {
            self.finish_reference_cell(false);
        } else if changed {self.rebuild=true;}
    }

    fn finish_reference_cell(&mut self,cancel:bool)->Option<usize> {
        self.flush_table_foreign_key();
        let edit=self.state.wizard_constraint_edit.take()?;
        if cancel {
            if let state::ConstraintCellOriginal::Text(original)=edit.original {self.set_constraint_cell_value(1,2,&original);self.flush_table_foreign_key();}
        } else if edit.reference.as_ref().is_some_and(|draft|draft.value.table.is_empty()) {self.remove_reference_key(edit.row);return Some(edit.row);}
        self.rebuild=true;None
    }

    fn snapshot_constraint_cell_editor(&mut self) {
        let id=self.ids.ov_constraint_editor;
        if id.0==0 {return;}
        if self.state.wizard_constraint_edit.as_ref().is_some_and(|edit|edit.reference.is_some()) {self.snapshot_reference_cell(true);return;}
        let Some(edit)=&mut self.state.wizard_constraint_edit else {return;};
        edit.input=self.ui.text_input_state(id);
        let (page,row,column)=(edit.page,edit.row,edit.column);
        if (page,column)==(0,1) {
            if let Some(selected)=self.ui.select_selected(id) {self.change_table_index_type(row,selected==0);}
        } else if let Some(value)=self.ui.text_input_value(id).map(str::to_owned) {self.set_constraint_cell_value(page,column,&value);}
        if self.ui.focused()!=Some(id) {self.state.wizard_constraint_edit=None;self.rebuild=true;}
    }

    fn handle_constraint_cell_edit(&mut self,page:usize,mut event:next_gui_ui::TableCellEditEvent) {
        let previous=self.state.wizard_constraint_edit.clone();
        if let next_gui_ui::TableCellEditEvent::End {cancel}=event {
            if previous.as_ref().is_some_and(|edit|edit.reference.is_some()) {
                self.snapshot_reference_cell(false);self.finish_reference_cell(cancel);
                self.do_rebuild();self.ui.set_focus(Some(self.ids.ov_fks_table));self.ui.sync(&mut self.scene);return;
            }
        }
        if page==1 && previous.as_ref().is_some_and(|edit|edit.reference.is_some()) {
            if let next_gui_ui::TableCellEditEvent::Begin {mut row,column,key}=event {
                self.snapshot_reference_cell(false);
                if let Some(removed)=self.finish_reference_cell(false) {
                    if row==removed {self.do_rebuild();self.ui.set_focus(Some(self.ids.ov_fks_table));self.ui.sync(&mut self.scene);return;}
                    if row>removed {row-=1;}
                }
                self.state.wizard_foreign_key_sel=row;self.do_rebuild();
                event=next_gui_ui::TableCellEditEvent::Begin {row,column,key};
            }
        }
        if !self.rebuild {self.snapshot_inputs();}
        if self.rebuild {self.do_rebuild();}
        match event {
            next_gui_ui::TableCellEditEvent::Begin {row,column,key} => {
                if [self.state.wizard_key_sel,self.state.wizard_foreign_key_sel,self.state.wizard_check_sel][page]!=row {return;}
                if page < 2 && column == 0 {self.open_column_picker(page,row);self.do_rebuild();return;}
                let Some(original)=self.state.constraint_cell_value(page,row,column).map(str::to_owned) else {return;};
                let original=if (page,column)==(0,1) {state::ConstraintCellOriginal::Index(self.state.wizard_index_constraints[row].clone())} else {state::ConstraintCellOriginal::Text(original)};
                let reference=if (page,column)==(1,2) {reference_editor::Draft::new(&self.state,&self.state.wizard_foreign_keys[row].clause).ok()} else {None};
                self.state.wizard_constraint_edit=Some(state::ConstraintCellEdit {page,row,column,original,input:None,reference});
                self.do_rebuild();let id=self.ids.ov_constraint_editor;
                if self.state.wizard_constraint_edit.as_ref().is_some_and(|edit|edit.reference.is_some()) {self.ids.reference_editor.focus(&mut self.ui,0);} else {self.ui.set_focus(Some(id));self.ui.key_input(if (page,column)==(0,1) {UiKey::Space} else {UiKey::SelectAll});}
                if let Some(key)=key {self.ui.key_input(key);}
            }
            next_gui_ui::TableCellEditEvent::End {cancel} => {
                if cancel {if let Some(edit)=previous {
                    match edit.original {
                        state::ConstraintCellOriginal::Text(original) => {
                            self.set_constraint_cell_value(edit.page,edit.column,&original);
                            match edit.page {0=>self.flush_table_index_constraint(),1=>self.flush_table_foreign_key(),_=>self.flush_table_check()}
                        }
                        state::ConstraintCellOriginal::Index(original) => {
                            self.state.wizard_index_constraints[edit.row]=original;self.apply_table_index_flags();
                        }
                    }
                    self.sync_table_constraint_names();
                }}
                self.state.wizard_constraint_edit=None;self.do_rebuild();
                self.ui.set_focus(Some([self.ids.ov_keys_table,self.ids.ov_fks_table,self.ids.ov_checks_table][page]));
            }
        }
        self.ui.sync(&mut self.scene);
    }

    fn handle_table_cell_edit(&mut self) {
        let Some((table,event))=self.ui.take_table_cell_edit() else {return;};
        if self.state.overlay!=Some(OverlayKind::EditTable) {return;}
        if let Some(page)=[self.ids.ov_keys_table,self.ids.ov_fks_table,self.ids.ov_checks_table].iter().position(|&id|id==table) {
            self.handle_constraint_cell_edit(page,event);return;
        }
        if table!=self.ids.ov_fields_table {return;}
        let previous=self.state.wizard_field_edit.clone();
        if let next_gui_ui::TableCellEditEvent::End {cancel}=event {
            if previous.as_ref().is_some_and(|edit|matches!(edit.column,7|9)) {
                self.snapshot_field_sql(false);self.finish_field_sql(cancel);self.do_rebuild();self.ui.set_focus(Some(self.ids.ov_fields_table));self.ui.sync(&mut self.scene);return;
            }
        }
        if !self.rebuild {self.snapshot_inputs();}
        if self.rebuild {self.do_rebuild();}
        match event {
            next_gui_ui::TableCellEditEvent::Begin {row,column,key}=> {
                if row!=self.state.wizard_col_sel || ![0,1,6,7,8,9].contains(&column) {return;}
                let Some(definition)=self.state.wizard_cols.get(row) else {return;};
                let original=match column {0=>&definition.name,1=>&definition.type_name,6=>&definition.default_value,7=>&definition.check_expression,8=>&definition.collation,9=>&definition.foreign_key_clause,_=>unreachable!()}.clone();
                let reference=if column==9 {reference_editor::Draft::new(&self.state,&original).ok()} else {None};
                self.state.wizard_field_edit=Some(state::FieldCellEdit {column,original,reference,input:None,autocomplete:None});
                self.do_rebuild();let editor=self.ids.ov_field_editor;
                if self.state.wizard_field_edit.as_ref().is_some_and(|edit|edit.reference.is_some()) {self.ids.reference_editor.focus(&mut self.ui,0);} else {self.ui.set_focus(Some(editor));self.ui.key_input(UiKey::SelectAll);}
                if let Some(key)=key {self.ui.key_input(key);}
                else if matches!(column,1|8) {
                    if self.ui.is_select(editor) {self.ui.key_input(UiKey::Space);} else {self.ui.open_autocomplete_options(editor);}
                }
                self.ui.sync(&mut self.scene);
            }
            next_gui_ui::TableCellEditEvent::End {cancel}=> {
                if cancel {if let Some(edit)=previous {self.set_field_cell_value(edit.column,&edit.original);self.flush_wizard_col();self.sync_table_constraint_names();}}
                self.state.wizard_field_edit=None;self.do_rebuild();self.ui.set_focus(Some(self.ids.ov_fields_table));self.ui.sync(&mut self.scene);
            }
        }
    }

    fn handle_field_check_change(&mut self) {
        let Some((table,row,column,checked)) = self.ui.take_table_check_change() else { return; };
        if table == self.ids.plot.columns && table.0 != 0 {
            if self.state.plot.check(row, column, checked) { self.rebuild = true; }
            return;
        }
        if table==self.ids.sql_export.table && self.state.overlay==Some(OverlayKind::ExportSql) {
            if column==0 {if let Some(entry)=self.state.sql_export.as_mut().and_then(|draft|draft.objects.get_mut(row)) {entry.2=checked;self.rebuild=true;}}
            return;
        }
        if table==self.ids.data_export.table && self.state.export_dialog.is_some() {
            if column==0 {if let Some(entry)=self.state.export_dialog.as_mut().and_then(|draft|draft.tables.get_mut(row)) {entry.2=checked;self.rebuild=true;}}
            return;
        }
        if table==self.ids.csv_files.table && self.state.overlay==Some(OverlayKind::ImportCsv) {
            if column==0 {if let Some(batch)=&mut self.state.csv_files {if let Some(index)=batch.visible().get(row).copied() {batch.files[index].selected=checked;self.rebuild=true;}}}
            return;
        }
        if table != self.ids.ov_fields_table || self.state.overlay != Some(OverlayKind::EditTable)
            || row >= self.state.wizard_cols.len() { return; }
        // Pointer snapshots flush the previous row first; rebuild its replacement before editing it.
        if self.rebuild { self.do_rebuild(); }
        if row != self.state.wizard_col_sel { return; }
        let control = match column { 2=>self.ids.ov_nn,3=>self.ids.ov_pk,4=>self.ids.ov_ai,5=>self.ids.ov_unique,_=>return };
        self.ui.set_checked(control,checked);
        self.flush_wizard_col();
        self.sync_table_constraint_names();
    }

    fn refresh_constraint_table(&mut self,table:WidgetId,rows:Vec<Vec<String>>) {
        let scroll=self.ui.table_scroll_offset(table);
        self.ui.set_table_rows(table,rows);
        if let Some((x,y))=scroll {self.ui.set_table_scroll_offset(table,x,y);}
    }

    fn refresh_table_index_controls(&mut self) {
        self.refresh_table_field_rows();
        self.refresh_constraint_table(self.ids.ov_keys_table,ui_build::table_index_rows(&self.state));
        if let Some(key) = self.state.wizard_index_constraints.get(self.state.wizard_key_sel) {
            if self.ui.text_area_value(self.ids.ov_key_columns) != Some(key.columns.as_str()) {
                self.ui.set_text_area_value(self.ids.ov_key_columns, key.columns.clone());
                sql_syntax::apply(&mut self.ui, self.ids.ov_key_columns, &self.theme);
            }
            self.ui.set_select_selected(self.ids.ov_key_conflict, table_sql::PRIMARY_KEY_CONFLICTS.iter().position(|(_, action)| *action == key.conflict));
        }
        if let Some(column) = self.state.wizard_cols.get(self.state.wizard_col_sel) {
            self.ui.set_checked(self.ids.ov_pk, column.pk); self.ui.set_checked(self.ids.ov_ai, column.auto_increment); self.ui.set_checked(self.ids.ov_unique, column.unique);
        }
        self.ui.set_select_selected(self.ids.ov_pk_conflict, table_sql::PRIMARY_KEY_CONFLICTS.iter().position(|(_, action)| *action == self.state.wizard_table_options.primary_key_conflict));
        self.ui.set_button_enabled(self.ids.ov_key_add_pk, !self.state.wizard_index_constraints.iter().any(|key| key.primary));
        let movable = |index| self.state.wizard_index_constraints.get(index).is_some_and(|key| !table_sql::index_constraint_requires_inline(key));
        self.ui.set_button_enabled(self.ids.ov_key_up, self.state.wizard_key_sel > 0 && movable(self.state.wizard_key_sel) && movable(self.state.wizard_key_sel.saturating_sub(1)));
        self.ui.set_button_enabled(self.ids.ov_key_down, movable(self.state.wizard_key_sel) && movable(self.state.wizard_key_sel+1));
    }

    fn apply_table_index_flags(&mut self) {
        if let Ok(columns) = table_sql::columns_with_index_constraints(&self.state.wizard_cols, &self.state.wizard_index_constraints) {
            self.state.wizard_cols = columns;
            self.state.wizard_table_options.primary_key_conflict = self.state.wizard_index_constraints.iter().find(|key| key.primary).and_then(|key| key.conflict);
        }
        self.state.wizard_table_options.index_constraints = Some(self.state.wizard_index_constraints.clone());
        self.refresh_table_index_controls();
    }

    fn flush_table_index_constraint(&mut self) {
        let selected = self.state.wizard_key_sel;
        let Some(key) = self.state.wizard_index_constraints.get_mut(selected) else { return; };
        let previous = key.clone();
        if let Some(name) = self.ui.text_input_value(self.ids.ov_key_name) { key.name = (!name.is_empty()).then(|| name.to_string()); }
        if let Some(columns) = self.ui.text_area_value(self.ids.ov_key_columns) { key.columns = columns.to_string(); }
        if let Some((_, action)) = self.ui.select_selected(self.ids.ov_key_conflict).and_then(|index| table_sql::PRIMARY_KEY_CONFLICTS.get(index)) { key.conflict = *action; }
        if *key != previous { self.apply_table_index_flags(); }
        self.state.wizard_key_states.resize(self.state.wizard_index_constraints.len(), None);
        self.state.wizard_key_states[selected] = self.ui.text_area_state(self.ids.ov_key_columns);
    }

    fn refresh_table_foreign_key_controls(&mut self) {
        self.refresh_constraint_table(self.ids.ov_fks_table,ui_build::table_foreign_key_rows(&self.state));
        if let Some(key) = self.state.wizard_foreign_keys.get(self.state.wizard_foreign_key_sel) {
            for (area,value) in [(self.ids.ov_fk_columns,&key.columns),(self.ids.ov_fk_clause,&key.clause)] {
                if self.ui.text_area_value(area) != Some(value.as_str()) { self.ui.set_text_area_value(area,value.clone()); sql_syntax::apply(&mut self.ui,area,&self.theme); }
            }
        }
        if let Some(column) = self.state.wizard_cols.get(self.state.wizard_col_sel) {
            if self.ui.text_area_value(self.ids.ov_foreign_key) != Some(column.foreign_key_clause.as_str()) {
                // shortcut: cross-view FK edits reset linked history; share a document for wizard-wide Undo.
                self.ui.set_text_area_value(self.ids.ov_foreign_key,column.foreign_key_clause.clone()); sql_syntax::apply(&mut self.ui,self.ids.ov_foreign_key,&self.theme);
            }
        }
        let selected = self.state.wizard_foreign_key_sel;
        let can_swap = |target| self.state.wizard_foreign_keys.get(selected).zip(self.state.wizard_foreign_keys.get(target)).is_some_and(|(a,b)|table_sql::foreign_keys_can_swap(a,b));
        self.ui.set_button_enabled(self.ids.ov_fk_up,selected>0 && can_swap(selected.saturating_sub(1)));
        self.ui.set_button_enabled(self.ids.ov_fk_down,can_swap(selected+1));
    }

    fn flush_table_foreign_key(&mut self) {
        let selected = self.state.wizard_foreign_key_sel;
        let Some(key) = self.state.wizard_foreign_keys.get_mut(selected) else { return; };
        let previous = key.clone();
        if let Some(name) = self.ui.text_input_value(self.ids.ov_fk_name) {
            if name != key.name.as_deref().unwrap_or("") { key.name = (!name.is_empty()).then(|| name.to_string()); }
        }
        if let Some(columns) = self.ui.text_area_value(self.ids.ov_fk_columns) { key.columns = columns.to_string(); }
        if let Some(clause) = self.ui.text_area_value(self.ids.ov_fk_clause) { key.clause = clause.to_string(); }
        table_sql::normalize_foreign_key_scope(key);
        if *key != previous {
            self.state.store_wizard_foreign_keys(); self.state.sync_wizard_foreign_key_fields();
            self.refresh_table_foreign_key_controls();
        }
        self.state.wizard_foreign_key_states.resize_with(self.state.wizard_foreign_keys.len(), || [None, None]);
        self.state.wizard_foreign_key_states[selected] = [self.ui.text_area_state(self.ids.ov_fk_columns), self.ui.text_area_state(self.ids.ov_fk_clause)];
    }

    fn refresh_table_check_controls(&mut self) {
        self.refresh_constraint_table(self.ids.ov_checks_table,ui_build::table_check_rows(&self.state));
        if let Some(check) = self.state.wizard_checks.get(self.state.wizard_check_sel) {
            if self.ui.text_area_value(self.ids.ov_check_expr) != Some(check.expression.as_str()) {
                self.ui.set_text_area_value(self.ids.ov_check_expr,check.expression.clone()); sql_syntax::apply(&mut self.ui,self.ids.ov_check_expr,&self.theme);
            }
        }
        if let Some(column) = self.state.wizard_cols.get(self.state.wizard_col_sel) {
            if self.ui.text_area_value(self.ids.ov_check) != Some(column.check_expression.as_str()) {
                // shortcut: cross-view CHECK edits reset linked history; share a document for wizard-wide Undo.
                self.ui.set_text_area_value(self.ids.ov_check,column.check_expression.clone()); sql_syntax::apply(&mut self.ui,self.ids.ov_check,&self.theme);
            }
        }
        let selected = self.state.wizard_check_sel;
        let can_swap = |target| self.state.wizard_checks.get(selected).zip(self.state.wizard_checks.get(target)).is_some_and(|(a,b)|table_sql::checks_can_swap(a,b));
        self.ui.set_button_enabled(self.ids.ov_check_up,selected>0 && can_swap(selected.saturating_sub(1)));
        self.ui.set_button_enabled(self.ids.ov_check_down,can_swap(selected+1));
    }

    fn flush_table_check(&mut self) {
        let selected = self.state.wizard_check_sel;
        let Some(check) = self.state.wizard_checks.get_mut(selected) else { return; };
        let previous = check.clone();
        if let Some(name) = self.ui.text_input_value(self.ids.ov_check_name) {
            if name != check.name.as_deref().unwrap_or("") { check.name = (!name.is_empty()).then(|| name.to_string()); }
        }
        if let Some(expression) = self.ui.text_area_value(self.ids.ov_check_expr) { check.expression = expression.to_string(); }
        if *check != previous {
            self.state.store_wizard_checks(); self.state.sync_wizard_check_fields(); self.refresh_table_check_controls();
        }
        self.state.wizard_check_states.resize(self.state.wizard_checks.len(), None);
        self.state.wizard_check_states[selected] = self.ui.text_area_state(self.ids.ov_check_expr);
    }

    fn flush_wizard_col(&mut self) {
        let sel = self.state.wizard_col_sel;
        if sel >= self.state.wizard_cols.len() {
            return;
        }
        let previous_foreign_key = self.state.wizard_cols[sel].foreign_key_clause.clone();
        let previous_check = self.state.wizard_cols[sel].check_expression.clone();
        if let Some(mode) = self.ui.select_selected(self.ids.ov_generation) {
            let previous = self.state.wizard_cols[sel].generated.as_ref().map_or(0, |generated| if generated.stored { 2 } else { 1 });
            if previous != mode {
                self.state.wizard_cols[sel].generated = if mode == 0 { None } else {
                    Some(dbmod::GeneratedColumn { expression: self.ui.text_area_value(self.ids.ov_generated).unwrap_or("").into(), stored: mode == 2 })
                };
                self.rebuild = true;
            }
        }
        self.state.wizard_column_edit_states.resize_with(self.state.wizard_cols.len(), || [None, None, None]);
        for (index, area) in [self.ids.ov_check, self.ids.ov_foreign_key, self.ids.ov_generated].into_iter().enumerate() {
            if let Some(value) = self.ui.text_area_value(area) {
                if index == 0 { self.state.wizard_cols[sel].check_expression = value.to_string(); }
                else if index == 1 { self.state.wizard_cols[sel].foreign_key_clause = value.to_string(); }
                else if let Some(generated) = &mut self.state.wizard_cols[sel].generated { generated.expression = value.to_string(); }
                self.state.wizard_column_edit_states[sel][index] = self.ui.text_area_state(area);
            }
        }
        let previous_columns = self.state.wizard_cols.clone();
        let previous_policy = self.state.wizard_table_options.primary_key_conflict;
        let previous_pk = self.state.wizard_cols[sel].pk;
        let previous_ai = self.state.wizard_cols[sel].auto_increment;
        if self.ids.ov_col_name.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.ov_col_name) {
                self.state.wizard_cols[sel].name = v.to_string();
            }
        }
        if self.ids.ov_col_type.0 != 0 {
            if let Some(value) = self.ui.autocomplete_value(self.ids.ov_col_type) {
                self.state.wizard_cols[sel].type_name = value.into();
            }
        }
        if self.ids.ov_pk.0 != 0 {
            if let Some(on) = self.ui.is_checked(self.ids.ov_pk) {
                self.state.wizard_cols[sel].pk = on;
            }
        }
        if let Some(on) = self.ui.is_checked(self.ids.ov_ai) {
            self.state.wizard_cols[sel].auto_increment = on;
        }
        if self.state.wizard_cols[sel].auto_increment && !previous_ai {
            for (index, column) in self.state.wizard_cols.iter_mut().enumerate() {
                column.pk = index == sel;
                column.auto_increment = index == sel;
            }
            self.state.wizard_cols[sel].type_name = "INTEGER".into();
            self.ui.set_checked(self.ids.ov_pk, true);
            self.ui.set_autocomplete_value(self.ids.ov_col_type, "INTEGER");
        } else if self.state.wizard_cols[sel].pk != previous_pk {
            for (index, column) in self.state.wizard_cols.iter_mut().enumerate() {
                if index != sel || !column.pk { column.auto_increment = false; }
            }
            self.ui.set_checked(self.ids.ov_ai, self.state.wizard_cols[sel].auto_increment);
        }
        if previous_pk && !self.state.wizard_cols.iter().any(|column| column.pk) {
            self.state.wizard_table_options.primary_key_conflict = None;
            self.ui.set_select_selected(self.ids.ov_pk_conflict, Some(0));
        }
        if self.ids.ov_nn.0 != 0 {
            if let Some(on) = self.ui.is_checked(self.ids.ov_nn) {
                self.state.wizard_cols[sel].notnull = on;
            }
        }
        if self.ids.ov_unique.0 != 0 {
            if let Some(on) = self.ui.is_checked(self.ids.ov_unique) {
                self.state.wizard_cols[sel].unique = on;
            }
        }
        if self.ids.ov_default.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.ov_default) {
                self.state.wizard_cols[sel].default_value = v.to_string();
            }
        }
        if let Some(value) = self.ui.autocomplete_value(self.ids.ov_collation) {
            self.state.wizard_cols[sel].collation = value.to_string();
        }
        if previous_check != self.state.wizard_cols[sel].check_expression {
            let owner = self.state.wizard_constraint_columns.get(sel).cloned().unwrap_or_else(||self.state.wizard_cols[sel].name.clone());
            let expression = self.state.wizard_cols[sel].check_expression.clone();
            let index = self.state.wizard_checks.iter().position(|check|check.column.as_ref().is_some_and(|column|column.eq_ignore_ascii_case(&owner)));
            match index {
                Some(index) if expression.trim().is_empty() && self.state.wizard_field_edit.as_ref().is_none_or(|edit|edit.column!=7) => {
                    self.state.wizard_checks.remove(index); self.state.wizard_check_states.remove(index);
                    if index < self.state.wizard_check_sel { self.state.wizard_check_sel -= 1; }
                    self.state.wizard_check_sel = self.state.wizard_check_sel.min(self.state.wizard_checks.len().saturating_sub(1));
                }
                Some(index) => { self.state.wizard_checks[index].expression = expression; self.state.wizard_check_states[index] = None; }
                None if !expression.trim().is_empty() => {
                    self.state.wizard_checks.push(table_sql::NamedCheck { column:Some(owner),name:None,expression }); self.state.wizard_check_states.push(None);
                }
                None => {}
            }
            self.state.store_wizard_checks(); self.refresh_table_check_controls();
        }
        if previous_foreign_key != self.state.wizard_cols[sel].foreign_key_clause {
            let owner = self.state.wizard_constraint_columns.get(sel).cloned().unwrap_or_else(|| self.state.wizard_cols[sel].name.clone());
            let clause = self.state.wizard_cols[sel].foreign_key_clause.clone();
            let index = self.state.wizard_foreign_keys.iter().position(|key| key.inline && table_sql::inline_foreign_key_column(key).is_ok_and(|name|name.eq_ignore_ascii_case(&owner)));
            match index {
                Some(index) if clause.trim().is_empty() && self.state.wizard_field_edit.as_ref().is_none_or(|edit|edit.column!=9) => {
                    self.state.wizard_foreign_keys.remove(index); self.state.wizard_foreign_key_states.remove(index);
                    if index < self.state.wizard_foreign_key_sel { self.state.wizard_foreign_key_sel -= 1; }
                    self.state.wizard_foreign_key_sel = self.state.wizard_foreign_key_sel.min(self.state.wizard_foreign_keys.len().saturating_sub(1));
                }
                Some(index) => { self.state.wizard_foreign_keys[index].clause = clause; self.state.wizard_foreign_key_states[index][1] = None; }
                None if !clause.trim().is_empty() => {
                    self.state.wizard_foreign_keys.push(table_sql::NamedForeignKey { name:None,columns:dbmod::quote_ident(&owner),clause,inline:true });
                    self.state.wizard_foreign_key_states.push([None,None]);
                }
                None => {}
            }
            self.state.store_wizard_foreign_keys(); self.refresh_table_foreign_key_controls();
        }
        if previous_columns.iter().zip(&self.state.wizard_cols).any(|(old, new)| old.pk != new.pk || old.unique != new.unique || old.auto_increment != new.auto_increment) {
            let result = self.state.table_index_constraints_from_columns().and_then(|keys|
                self.state.sync_table_constraint_names().map(|_| keys));
            match result {
                Ok(keys) => {
                    for (index, (old, new)) in self.state.wizard_index_constraints.iter().zip(&keys).enumerate() {
                        if old != new { if let Some(session) = self.state.wizard_key_states.get_mut(index) { *session = None; } }
                    }
                    self.state.wizard_index_constraints = keys;
                    self.state.wizard_key_states.resize(self.state.wizard_index_constraints.len(), None);
                    self.state.wizard_key_sel = self.state.wizard_key_sel.min(self.state.wizard_index_constraints.len().saturating_sub(1));
                    self.state.wizard_table_options.index_constraints = Some(self.state.wizard_index_constraints.clone());
                    self.refresh_table_index_controls(); self.rebuild = true;
                }
                Err(error) => {
                    self.state.wizard_cols = previous_columns; self.state.wizard_table_options.primary_key_conflict = previous_policy;
                    self.ui.set_text_input(self.ids.ov_col_name,self.state.wizard_cols[sel].name.clone());
                    self.ui.set_autocomplete_value(self.ids.ov_col_type,self.state.wizard_cols[sel].type_name.clone());
                    self.refresh_table_index_controls(); self.set_error(error);
                }
            }
        }
    }

    fn sync_index_wizard(&mut self, keys_changed: bool) {
        let keys = crate::index_sql::keys(&self.state.wizard_index_cols);
        let count = keys.as_ref().map_or(0, Vec::len);
        self.state.wizard_index_key_sel = self.state.wizard_index_key_sel.min(count.saturating_sub(1));
        if keys_changed {
            self.ui.set_list_items(self.ids.ov_list, keys.as_ref().map(|keys| keys.iter().enumerate()
                .map(|(i, key)| format!("{}. {}", i + 1, key.split_whitespace().collect::<Vec<_>>().join(" "))).collect::<Vec<_>>()).unwrap_or_default());
            self.ui.set_list_selected(self.ids.ov_list, Some(self.state.wizard_index_key_sel));
            self.ui.set_list_scroll_offset(self.ids.ov_list, self.state.wizard_index_list_scroll[0]);
        }
        self.ui.set_button_enabled(self.ids.ov_btn_add, keys.is_ok() && self.ui.list_selected(self.ids.ov_idx_available).is_some());
        self.ui.set_button_enabled(self.ids.ov_idx_expression, keys.is_ok());
        for id in [self.ids.ov_btn_del, self.ids.ov_idx_edit_key] { self.ui.set_button_enabled(id, count > 0); }
        self.ui.set_button_enabled(self.ids.ov_btn_up, count > 0 && self.state.wizard_index_key_sel > 0);
        self.ui.set_button_enabled(self.ids.ov_btn_down, self.state.wizard_index_key_sel + 1 < count);
        let order = self.state.index_wizard_key_order().ok().map(usize::from);
        if self.ui.select_selected(self.ids.ov_idx_order) != order { self.ui.set_select_selected(self.ids.ov_idx_order, order); }
        let sql = self.state.index_wizard_sql();
        if self.ui.text_area_value(self.ids.ov_idx_preview) != Some(sql.as_str()) {
            self.ui.set_text_area_value(self.ids.ov_idx_preview, sql);
            sql_syntax::apply(&mut self.ui, self.ids.ov_idx_preview, &self.theme);
        }
    }

    fn set_index_key_order(&mut self, descending: bool) -> Result<(), String> {
        let keys = crate::index_sql::keys(&self.state.wizard_index_cols)?;
        let selected = self.state.wizard_index_key_sel;
        let key = keys.get(selected).ok_or("Select an index key")?;
        let (expression, _) = crate::index_sql::key_order(self.state.conn.as_ref().ok_or("No database open")?,
            &self.state.wizard_schema, &self.state.wizard_index_table, key)?;
        let range = crate::index_sql::key_ranges(&self.state.wizard_index_cols)?[selected].clone();
        self.ui.replace_text_area_range(self.ids.ov_idx_cols, range, &format!("{expression}\n{}", if descending { "DESC" } else { "ASC" }));
        self.state.wizard_index_cols = self.ui.text_area_value(self.ids.ov_idx_cols).unwrap().to_string();
        self.state.wizard_index_edit_states[0] = self.ui.text_area_state(self.ids.ov_idx_cols);
        Ok(())
    }

    fn edit_index_key(&mut self, id: WidgetId) -> Result<(), String> {
        let mut keys = crate::index_sql::keys(&self.state.wizard_index_cols)?;
        let mut selected = self.state.wizard_index_key_sel;
        if id == self.ids.ov_btn_add {
            let columns = self.state.index_wizard_columns()?;
            keys.push(dbmod::quote_ident(columns.get(self.state.wizard_index_available_sel).ok_or("Select a table column")?));
            selected = keys.len() - 1;
        } else if id == self.ids.ov_idx_expression {
            keys.push("NULL".into());
            selected = keys.len() - 1;
        } else if id == self.ids.ov_btn_del && selected < keys.len() {
            keys.remove(selected);
            selected = selected.min(keys.len().saturating_sub(1));
        } else if id == self.ids.ov_btn_up && selected > 0 && selected < keys.len() {
            keys.swap(selected, selected - 1); selected -= 1;
        } else if id == self.ids.ov_btn_down && selected + 1 < keys.len() {
            keys.swap(selected, selected + 1); selected += 1;
        }
        if id != self.ids.ov_idx_edit_key {
            // A newline before each separator keeps trailing line comments from swallowing the next key.
            let sql = keys.join("\n,\n");
            let len = self.state.wizard_index_cols.chars().count();
            self.ui.replace_text_area_range(self.ids.ov_idx_cols, 0..len, &sql);
            self.state.wizard_index_cols = sql;
        }
        self.state.wizard_index_key_sel = selected;
        self.sync_index_wizard(true);
        self.ui.reveal_list_selection(self.ids.ov_list);
        self.state.wizard_index_list_scroll[0] = self.ui.list_scroll_offset(self.ids.ov_list).unwrap_or(0.0);
        if id == self.ids.ov_idx_edit_key || id == self.ids.ov_idx_expression {
            if let Some(range) = crate::index_sql::key_ranges(&self.state.wizard_index_cols)?.get(selected) {
                self.ui.select_text_area_range(self.ids.ov_idx_cols, range.clone());
                self.ui.set_focus(Some(self.ids.ov_idx_cols));
            }
        }
        self.state.wizard_index_edit_states[0] = self.ui.text_area_state(self.ids.ov_idx_cols);
        Ok(())
    }

    fn do_rebuild(&mut self) {
        let modal_session=self.ui.modal_state(self.ids.modal);
        let plot_session=self.ui.plot_session(self.ids.chart,0);
        let plot_focused=self.ui.focused()==Some(self.ids.chart);
        let plot_source = self.state.plot.source.clone();
        let plot_scroll = self.ui.table_scroll_offset(self.ids.plot.columns);
        let plot_focus = [self.ids.plot.mode, self.ids.plot.columns, self.ids.plot.line, self.ids.plot.marker, self.ids.plot.color, self.ids.plot.legend, self.ids.plot.save, self.ids.plot.copy].into_iter().position(|id| id.0 != 0 && self.ui.focused() == Some(id));
        let pragma_scroll = self.ui.scroll_offset(self.ids.pragma_scroll);
        let pragma_session = if self.state.tab == MainTab::Pragmas && self.last_tab == MainTab::Pragmas.index() && self.state.overlay.is_none() {
            self.snapshot_inputs();
            [&self.ids.pragma_inputs, &self.ids.pragma_selects, &self.ids.pragma_switches].iter().enumerate().find_map(|(kind, ids)| {
                ids.iter().enumerate().find_map(|(index, id)| (id.0 != 0 && self.ui.focused() == Some(*id)).then(|| (kind, index, self.ui.text_input_state(*id), self.ui.select_state(*id))))
            })
        } else { None };
        let sql_export_session=self.ids.sql_export.session(&self.ui);
        self.snapshot_sql_export();
        let export_session=self.ids.data_export.session(&self.ui);
        if self.state.export_dialog.is_some() {self.snapshot_data_export();}
        self.snapshot_column_picker();
        self.snapshot_csv_view();
        let csv_waiting=self.state.csv_work.is_some();
        let csv_overlay = matches!(self.state.overlay, Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv));
        if csv_overlay { self.state.refresh_csv_preview(); }
        let csv_open=csv_overlay && !csv_waiting;
        let csv_input_session = csv_open.then(|| [(self.ids.ov_csv_table,self.state.csv_table.as_str()),(self.ids.ov_csv_custom_delimiter,self.state.csv_custom_delimiter.as_str()),(self.ids.ov_csv_custom_quote,self.state.csv_custom_quote.as_str())].into_iter().enumerate().find_map(|(index, (id,value))| (id.0 != 0 && self.ui.focused() == Some(id) && self.ui.text_input_value(id)==Some(value)).then(|| self.ui.text_input_state(id).map(|session| (index, session))).flatten())).flatten();
        let csv_paste_session = (csv_open && self.ids.ov_csv_paste.0 != 0 && self.ui.focused() == Some(self.ids.ov_csv_paste) && self.ui.text_area_value(self.ids.ov_csv_paste)==Some(self.state.csv_paste.as_str())).then(|| self.ui.text_area_state(self.ids.ov_csv_paste)).flatten();
        let csv_control_focus = csv_open.then(|| [self.ids.ov_csv_header,self.ids.ov_csv_trim,self.ids.ov_csv_local,self.ids.ov_csv_types,self.ids.ov_csv_defaults,self.ids.ov_csv_missing,self.ids.ov_csv_advanced,self.ids.ov_csv_refresh].into_iter().position(|id| id.0 != 0 && self.ui.focused() == Some(id))).flatten();
        let advanced_focused=self.state.overlay==Some(OverlayKind::EditTable) && self.ui.focused()==Some(self.ids.ov_table_advanced);
        let column_input_session = self.state.wizard_cols.get(self.state.wizard_col_sel).and_then(|column| {
            if self.state.overlay!=Some(OverlayKind::EditTable) || self.ui.text_input_value(self.ids.ov_col_name)!=Some(column.name.as_str()) { return None; }
            [self.ids.ov_col_name,self.ids.ov_default].into_iter().enumerate().find_map(|(index,id)|
                (self.ui.focused()==Some(id)).then(||self.ui.text_input_state(id).map(|session|(index,session))).flatten())
        });
        let table_name_focused = self.state.overlay==Some(OverlayKind::EditTable) && self.ui.focused()==Some(self.ids.ov_name)
            && self.ui.text_input_value(self.ids.ov_name)==Some(self.state.wizard_table_name.as_str());
        let constraint_editor_focused=self.state.wizard_constraint_edit.is_some() && self.ids.ov_constraint_editor.0!=0 && self.ui.focused()==Some(self.ids.ov_constraint_editor);
        let field_editor_focused=self.state.wizard_field_edit.is_some() && self.ids.ov_field_editor.0!=0 && self.ui.focused()==Some(self.ids.ov_field_editor);
        let cell_select_session=[self.ids.ov_field_editor,self.ids.ov_constraint_editor,self.ids.reference_editor.parent,self.ids.ov_csv_encoding,self.ids.ov_csv_delim,self.ids.ov_csv_quote,self.ids.ov_csv_conflict].into_iter().enumerate().find_map(|(index,id)|
            (id.0!=0 && self.ui.focused()==Some(id)).then(||self.ui.select_state(id).map(|session|(index,session))).flatten()).filter(|_|!csv_waiting);
        let fields_focused = self.state.overlay==Some(OverlayKind::EditTable) && self.ui.focused()==Some(self.ids.ov_fields_table);
        let reveal_field = self.state.overlay==Some(OverlayKind::EditTable) && self.ui.table_selected_row(self.ids.ov_fields_table)!=Some(self.state.wizard_col_sel);
        let constraint_ids=[self.ids.ov_keys_table,self.ids.ov_fks_table,self.ids.ov_checks_table];
        let constraint_focused=(self.state.overlay==Some(OverlayKind::EditTable)).then(||constraint_ids.iter().position(|&table|table.0!=0 && self.ui.focused()==Some(table))).flatten();
        let constraint_rows=[self.state.wizard_key_sel,self.state.wizard_foreign_key_sel,self.state.wizard_check_sel];
        let constraint_reveal=[0,1,2].map(|index|constraint_ids[index].0!=0 && self.ui.table_selected_row(constraint_ids[index])!=Some(constraint_rows[index]));
        let key_focused = self.state.overlay == Some(OverlayKind::EditTable) && self.ui.focused() == Some(self.ids.ov_key_columns)
            && self.ui.text_area_value(self.ids.ov_key_columns) == self.state.wizard_index_constraints.get(self.state.wizard_key_sel).map(|key| key.columns.as_str());
        let foreign_key_focused = (self.state.overlay == Some(OverlayKind::EditTable)).then(||
            [self.ids.ov_fk_columns, self.ids.ov_fk_clause].into_iter().enumerate().find_map(|(index, area)|
                (self.ui.focused() == Some(area) && self.state.wizard_foreign_keys.get(self.state.wizard_foreign_key_sel).is_some_and(|key|
                    self.ui.text_area_value(area) == Some(if index == 0 { key.columns.as_str() } else { key.clause.as_str() }))).then_some(index))
        ).flatten();
        let table_tabs_focused = self.state.overlay == Some(OverlayKind::EditTable) && self.ui.focused() == Some(self.ids.ov_table_tabs);
        let check_focused = self.state.overlay == Some(OverlayKind::EditTable)
            && self.ui.focused() == Some(self.ids.ov_check_expr)
            && self.ui.text_area_value(self.ids.ov_check_expr) == self.state.wizard_checks.get(self.state.wizard_check_sel).map(|check| check.expression.as_str());
        let table_preview_focused = self.state.overlay == Some(OverlayKind::EditTable)
            && self.ui.focused() == Some(self.ids.ov_table_preview);
        let generation_focused = self.state.overlay == Some(OverlayKind::EditTable)
            && self.ui.focused() == Some(self.ids.ov_generation)
            && self.ui.text_input_value(self.ids.ov_col_name) == self.state.wizard_cols.get(self.state.wizard_col_sel).map(|column| column.name.as_str());
        let key_conflict_focused = self.state.overlay == Some(OverlayKind::EditTable)
            && self.ui.focused() == Some(self.ids.ov_pk_conflict)
            && self.ui.text_input_value(self.ids.ov_name) == Some(self.state.wizard_table_name.as_str());
        let column_sql_focused = self.state.wizard_cols.get(self.state.wizard_col_sel).and_then(|column| {
            if self.state.overlay != Some(OverlayKind::EditTable)
                || self.ui.text_input_value(self.ids.ov_col_name) != Some(column.name.as_str()) { return None; }
            [(self.ids.ov_check, column.check_expression.as_str()), (self.ids.ov_foreign_key, column.foreign_key_clause.as_str()), (self.ids.ov_generated, column.generated.as_ref().map_or("", |generated| generated.expression.as_str()))]
                .into_iter().position(|(area, value)| self.ui.focused() == Some(area) && self.ui.text_area_value(area) == Some(value))
        });
        if let Some(index) = column_sql_focused {
            self.state.wizard_column_edit_states.resize_with(self.state.wizard_cols.len(), || [None, None, None]);
            let area = [self.ids.ov_check, self.ids.ov_foreign_key, self.ids.ov_generated][index];
            self.state.wizard_column_edit_states[self.state.wizard_col_sel][index] = self.ui.text_area_state(area);
        }
        let csv_encoding_session=(!csv_waiting && self.state.overlay==Some(OverlayKind::ImportCsv) && self.ids.ov_csv_custom_encoding.0!=0
            && self.ui.focused()==Some(self.ids.ov_csv_custom_encoding) && self.ui.autocomplete_value(self.ids.ov_csv_custom_encoding)==Some(self.state.csv_custom_encoding.as_str()))
            .then(||self.ui.autocomplete_state(self.ids.ov_csv_custom_encoding)).flatten();
        let collation_session = (self.state.overlay == Some(OverlayKind::EditTable)
            && self.ui.focused() == Some(self.ids.ov_collation)
            && self.ui.text_input_value(self.ids.ov_col_name) == self.state.wizard_cols.get(self.state.wizard_col_sel).map(|col| col.name.as_str())
            && self.ui.autocomplete_value(self.ids.ov_collation) == self.state.wizard_cols.get(self.state.wizard_col_sel).map(|col| col.collation.as_str()))
            .then(|| self.ui.autocomplete_state(self.ids.ov_collation)).flatten();
        let column_type_session = (self.state.overlay == Some(OverlayKind::EditTable)
            && self.ui.focused() == Some(self.ids.ov_col_type)
            && self.ui.text_input_value(self.ids.ov_col_name) == self.state.wizard_cols.get(self.state.wizard_col_sel).map(|col| col.name.as_str())
            && self.ui.autocomplete_value(self.ids.ov_col_type) == self.state.wizard_cols.get(self.state.wizard_col_sel).map(|col| col.type_name.as_str()))
            .then(|| self.ui.autocomplete_state(self.ids.ov_col_type)).flatten();
        let index_focused = (self.state.overlay == Some(OverlayKind::EditIndex)).then(||
            [self.ids.ov_idx_cols, self.ids.ov_idx_where, self.ids.ov_idx_preview, self.ids.ov_list, self.ids.ov_idx_available, self.ids.ov_idx_order].into_iter().position(|area| area.0 != 0 && self.ui.focused() == Some(area))
        ).flatten();
        let tabs_focus = [self.ids.tabs,self.ids.dock_tabs].into_iter().position(|id| id.0 != 0 && self.ui.focused() == Some(id));
        let dock_tabs_view = self.ui.tabs_state(self.ids.dock_tabs);
        let log_revision = self.state.sql_log.revision();
        self.theme = ui_build::browser_theme(self.theme, self.state.prefs.font_size);
        let log_focused = self.ids.log_area.0 != 0 && self.ui.focused() == Some(self.ids.log_area)
            && self.state.dock_log && self.state.dock_focus == DockKind::Log && self.state.overlay.is_none();
        let browse_focused = self.state.tab == MainTab::Browse && self.ids.browse_table.0 != 0
            && self.ui.focused() == Some(self.ids.browse_table);
        let tree_focused = self.state.tab == MainTab::Structure && self.ids.tree.0 != 0
            && self.ui.focused() == Some(self.ids.tree) && self.state.overlay.is_none() && !self.state.confirm_open;
        let sql_focused = self.ui.focused() == Some(self.ids.sql_area) && (self.state.overlay.is_none() || self.state.overlay == Some(OverlayKind::FindReplace)) && self.state.tab == MainTab::ExecuteSql;
        let sql_table_focused = self.ids.sql_table.0 != 0 && self.ui.focused() == Some(self.ids.sql_table) && self.state.tab == MainTab::ExecuteSql;
        if let Err(error) = self.state.sync_current_plot() { self.state.plot.clear(); self.set_error(error); }
        self.sync_cell_selection();
        let built = build_ui(
            self.font.clone(),
            self.theme,
            self.win_w,
            self.win_h,
            &self.state,
        );
        self.last_body_ratio = built.ui.splitter_ratio(built.body_splitter);
        self.last_structure_ratio = built.ui.splitter_ratio(built.structure_splitter);
        self.last_dock_ratio = built.ui.splitter_ratio(built.dock_splitter);
        self.last_plot_ratio = built.ui.splitter_ratio(built.plot_splitter);
        self.browse_header_column = None;
        self.ids = BuiltUiIds::from(&built);
        self.ui = built.ui;
        self.scene = built.scene;
        if let Some(session)=modal_session {if self.ui.restore_modal_state(self.ids.modal,&session) {self.ui.sync(&mut self.scene);}}
        if let Some(session)=plot_session {if self.ui.restore_plot_session(self.ids.chart,0,&session) {if plot_focused {self.ui.set_focus(Some(self.ids.chart));} self.ui.sync(&mut self.scene);}}
        if plot_source == self.state.plot.source {
            if let Some((x, y)) = plot_scroll { self.ui.set_table_scroll_offset(self.ids.plot.columns, x, y); }
            if let Some(index) = plot_focus {
                let id = [self.ids.plot.mode, self.ids.plot.columns, self.ids.plot.line, self.ids.plot.marker, self.ids.plot.color, self.ids.plot.legend, self.ids.plot.save, self.ids.plot.copy][index];
                if id.0 != 0 { self.ui.set_focus(Some(id)); }
            }
        }
        if let Some((x, y)) = pragma_scroll { self.ui.set_scroll_offset(self.ids.pragma_scroll, x, y); }
        if let Some((kind, index, text, select)) = pragma_session {
            if let Some(id) = [&self.ids.pragma_inputs, &self.ids.pragma_selects, &self.ids.pragma_switches][kind].get(index).copied().filter(|id| id.0 != 0) {
                if let Some(text) = text { self.ui.restore_text_input_state(id, &text); }
                if let Some(select) = select { self.ui.restore_select_state(id, &select); }
                self.ui.set_focus(Some(id));
            }
        }
        if let Some(session)=sql_export_session {self.ids.sql_export.restore(&mut self.ui,&session);}
        if let Some(session)=export_session {self.ids.data_export.restore(&mut self.ui,&session);}
        if let Some((index, session)) = csv_input_session { let id = [self.ids.ov_csv_table,self.ids.ov_csv_custom_delimiter,self.ids.ov_csv_custom_quote][index]; if self.ui.restore_text_input_state(id, &session) { self.ui.set_focus(Some(id)); } }
        if let Some(session) = csv_paste_session { if self.ui.restore_text_area_state(self.ids.ov_csv_paste, &session) { self.ui.set_focus(Some(self.ids.ov_csv_paste)); } }
        if let Some(index) = csv_control_focus { let id = [self.ids.ov_csv_header,self.ids.ov_csv_trim,self.ids.ov_csv_local,self.ids.ov_csv_types,self.ids.ov_csv_defaults,self.ids.ov_csv_missing,self.ids.ov_csv_advanced,self.ids.ov_csv_refresh][index]; if id.0 != 0 { self.ui.set_focus(Some(id)); } }
        if advanced_focused { self.ui.set_focus(Some(self.ids.ov_table_advanced)); self.ui.sync(&mut self.scene); }
        if let Some((index,session))=column_input_session {
            let id=[self.ids.ov_col_name,self.ids.ov_default][index];
            if self.ui.restore_text_input_state(id,&session) { self.ui.set_focus(Some(id)); self.ui.sync(&mut self.scene); }
        }
        if table_name_focused { self.ui.set_focus(Some(self.ids.ov_name)); self.ui.sync(&mut self.scene); }
        if reveal_field {
            self.ui.reveal_table_selection(self.ids.ov_fields_table); self.ui.sync(&mut self.scene);
            if let Some(offset)=self.ui.table_scroll_offset(self.ids.ov_fields_table) { self.state.wizard_field_scroll=offset; }
        }
        if fields_focused { self.ui.set_focus(Some(self.ids.ov_fields_table)); self.ui.sync(&mut self.scene); }
        if constraint_editor_focused && self.ids.ov_constraint_editor.0!=0 {self.ui.set_focus(Some(self.ids.ov_constraint_editor));self.ui.sync(&mut self.scene);}
        if field_editor_focused && self.ids.ov_field_editor.0!=0 {self.ui.set_focus(Some(self.ids.ov_field_editor));self.ui.sync(&mut self.scene);}
        if let Some((index,session))=cell_select_session {
            let id=[self.ids.ov_field_editor,self.ids.ov_constraint_editor,self.ids.reference_editor.parent,self.ids.ov_csv_encoding,self.ids.ov_csv_delim,self.ids.ov_csv_quote,self.ids.ov_csv_conflict][index];
            if self.ui.restore_select_state(id,&session) {self.ui.set_focus(Some(id));self.ui.sync(&mut self.scene);}
        }
        if table_tabs_focused { self.ui.set_focus(Some(self.ids.ov_table_tabs)); self.ui.sync(&mut self.scene); }
        for (index,table) in [self.ids.ov_keys_table,self.ids.ov_fks_table,self.ids.ov_checks_table].into_iter().enumerate() {
            if constraint_reveal[index] {self.ui.reveal_table_selection(table);}
            if constraint_focused==Some(index) {self.ui.set_focus(Some(table));}
        }
        if constraint_focused.is_some() || constraint_reveal.iter().any(|&reveal|reveal) {self.ui.sync(&mut self.scene);}

        if let Some(index) = foreign_key_focused {
            let area = [self.ids.ov_fk_columns, self.ids.ov_fk_clause][index]; self.ui.set_focus(Some(area));
            if let Some(editors) = self.state.wizard_foreign_key_states.get(self.state.wizard_foreign_key_sel) {
                if let Some(editor) = &editors[index] { self.ui.restore_text_area_state(area, editor); }
            }
            sql_syntax::apply(&mut self.ui, area, &self.theme); self.ui.sync(&mut self.scene);
        }
        if key_focused {
            self.ui.set_focus(Some(self.ids.ov_key_columns));
            if let Some(Some(editor)) = self.state.wizard_key_states.get(self.state.wizard_key_sel) { self.ui.restore_text_area_state(self.ids.ov_key_columns, editor); }
            sql_syntax::apply(&mut self.ui, self.ids.ov_key_columns, &self.theme); self.ui.sync(&mut self.scene);
        }
        if check_focused {
            self.ui.set_focus(Some(self.ids.ov_check_expr));
            if let Some(Some(editor)) = self.state.wizard_check_states.get(self.state.wizard_check_sel) { self.ui.restore_text_area_state(self.ids.ov_check_expr, editor); }
            sql_syntax::apply(&mut self.ui, self.ids.ov_check_expr, &self.theme); self.ui.sync(&mut self.scene);
        }
        if table_preview_focused { self.ui.set_focus(Some(self.ids.ov_table_preview)); self.ui.sync(&mut self.scene); }
        if generation_focused { self.ui.set_focus(Some(self.ids.ov_generation)); self.ui.sync(&mut self.scene); }
        if key_conflict_focused {
            self.ui.set_focus(Some(self.ids.ov_pk_conflict));
            self.ui.sync(&mut self.scene);
        }
        if let Some(index) = column_sql_focused {
            let area = [self.ids.ov_check, self.ids.ov_foreign_key, self.ids.ov_generated][index];
            self.ui.set_focus(Some(area));
            if let Some(Some(editor)) = self.state.wizard_column_edit_states.get(self.state.wizard_col_sel).map(|states| &states[index]) {
                self.ui.restore_text_area_state(area, editor);
            }
            sql_syntax::apply(&mut self.ui, area, &self.theme);
            self.ui.sync(&mut self.scene);
        }
        if let Some(session)=csv_encoding_session {
            if self.ui.set_autocomplete_state(self.ids.ov_csv_custom_encoding,&session) {self.ui.set_focus(Some(self.ids.ov_csv_custom_encoding));self.ui.sync(&mut self.scene);}
        }
        if let Some(session) = collation_session {
            if self.ui.set_autocomplete_state(self.ids.ov_collation, &session) {
                self.ui.set_focus(Some(self.ids.ov_collation));
                self.ui.sync(&mut self.scene);
            }
        }
        if let Some(session) = column_type_session {
            if self.ui.set_autocomplete_state(self.ids.ov_col_type, &session) {
                self.ui.set_focus(Some(self.ids.ov_col_type));
                self.ui.sync(&mut self.scene);
            }
        }
        if let Some(index) = index_focused {
            self.ui.set_focus(Some([self.ids.ov_idx_cols, self.ids.ov_idx_where, self.ids.ov_idx_preview, self.ids.ov_list, self.ids.ov_idx_available, self.ids.ov_idx_order][index]));
            if let Some(Some(editor)) = self.state.wizard_index_edit_states.get(index) {
                self.ui.restore_text_area_state([self.ids.ov_idx_cols, self.ids.ov_idx_where, self.ids.ov_idx_preview][index], editor);
                sql_syntax::apply(&mut self.ui, [self.ids.ov_idx_cols, self.ids.ov_idx_where, self.ids.ov_idx_preview][index], &self.theme);
            }
            self.ui.sync(&mut self.scene);
        }
        self.last_schema_names = self.state.schema_names.clone();
        if self.ids.tree.0 != 0 {
            let reveal = std::mem::take(&mut self.reveal_schema_selection);
            if reveal { self.ui.reveal_tree_selection(self.ids.tree); }
            if tree_focused || reveal { self.ui.set_focus(Some(self.ids.tree)); }
            self.state.schema_tree_scroll = self.ui.tree_scroll_offset(self.ids.tree).unwrap();
            self.ui.sync(&mut self.scene);
        }
        if let Some(view) = dock_tabs_view {
            if self.ui.restore_tabs_state(self.ids.dock_tabs, &view) {
                self.ui.sync(&mut self.scene);
            }
        }
        if let Some(index) = tabs_focus {
            let id = [self.ids.tabs,self.ids.dock_tabs][index];
            if id.0 != 0 { self.ui.set_focus(Some(id)); self.ui.sync(&mut self.scene); }
        }
        if std::mem::take(&mut self.reveal_browse_selection) && self.ids.browse_table.0 != 0 {
            self.ui.reveal_table_selection(self.ids.browse_table);
            self.ui.set_focus(Some(self.ids.browse_table));
            self.ui.sync(&mut self.scene);
            if let Some(scroll) = self.ui.table_scroll_offset(self.ids.browse_table) {
                self.state.browse_mut().scroll = scroll;
            }
        }
        if browse_focused && self.ids.browse_table.0 != 0 {
            self.ui.set_focus(Some(self.ids.browse_table));
            self.ui.sync(&mut self.scene);
        }
        if sql_table_focused { self.ui.set_focus(Some(self.ids.sql_table)); self.ui.sync(&mut self.scene); }
        if sql_focused {
            self.ui.set_focus(Some(self.ids.sql_area));
            if let Some(Some(editor)) = self.state.sql_edit_states.get(self.state.sql_tab_idx) {
                self.ui.restore_text_area_state(self.ids.sql_area, editor);
            }
            self.update_sql_hint();
            sql_syntax::apply(&mut self.ui, self.ids.sql_area, &self.theme);
            self.ui.sync(&mut self.scene);
        }
        if log_focused && self.ids.log_area.0 != 0 {
            self.ui.set_focus(Some(self.ids.log_area));
            self.ui.sync(&mut self.scene);
        }
        if self.state.confirm_open { self.ui.set_focus(Some(self.ids.modal)); self.ui.sync(&mut self.scene); }
        let clear = clear_color(&self.theme);
        self.backend.set_clear_color(clear);
        if let Some(g) = self.gpu_stash.as_mut() {
            g.set_clear_color(clear);
        }
        if let Some(g) = self.surface_gpu.as_mut() {
            g.set_clear_color(clear);
        }
        self.needs_paint = true;
        self.rebuild = false;
        self.last_tree_sel = self.ui.tree_selected(self.ids.tree);
        self.last_tab = self.state.tab.index();
        self.last_browse_sel = self.ui.select_selected(self.ids.browse_select);
        self.last_browse_row = self.state.browse().sel_row;
        self.last_log_revision = log_revision;
    }

    fn refresh_sql_log(&mut self) -> bool {
        if self.state.dock_log && self.state.dock_focus == DockKind::Log
            && self.state.sql_log.revision() != self.last_log_revision {
            self.snapshot_inputs();
            self.do_rebuild();
            return true;
        }
        false
    }

    fn set_error(&mut self, e: impl Into<String>) {
        let mut e = e.into();
        match self.state.recover_ended_transaction() {
            Ok(true) => {
                e.push_str("; SQLite ended the transaction; review earlier pending changes before continuing");
                self.rebuild = true;
            }
            Ok(false) => {}
            Err(recovery) => {
                e.push_str(&format!("; could not refresh after the transaction ended: {recovery}"));
                self.rebuild = true;
            }
        }
        if self.state.error_msg.as_deref() != Some(e.as_str()) {
            self.state.sql_log.push(crate::sql_log::Source::Error, &e);
        }
        self.state.error_msg = Some(e.clone());
        self.state.status_msg = e.clone();
        self.state.sql_message = e;
    }

    fn clear_error(&mut self) {
        self.state.error_msg = None;
    }

    fn require_conn(&mut self) -> bool {
        if self.state.conn.is_some() {
            true
        } else {
            self.set_error("No database open");
            self.rebuild = true;
            false
        }
    }

    fn open_url(&mut self, url: &str) {
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(url).status();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open").arg(url).status();
        match result {
            Ok(status) if status.success() => self.state.status_msg = format!("Opened {url}"),
            other => self.set_error(format!("Cannot open {url}: {other:?}")),
        }
        self.rebuild = true;
    }

    fn request_document_action(&mut self, action: DocumentAction) {
        if let Err(error) = self.state.guard_background_idle() {
            self.set_error(error);
            return;
        }
        self.snapshot_inputs();
        let needs_save = if let DocumentAction::CloseSqlTab(index) = &action {
            self.state.sql_tab_dirty(*index)
        } else if let DocumentAction::SelectSchemaNode(node) = &action {
            self.state.schema_selection_has_draft(*node)
        } else { self.state.dirty || self.state.has_unsaved_drafts() };
        if self.state.sql_busy {
            self.set_error("Stop or finish SQL first");
        } else if needs_save {
            self.state.pending_document = Some(action);
            self.state.confirm_kind = ConfirmKind::Documents;
            self.state.confirm_open = true;
        } else {
            self.perform_document_action(action);
        }
        self.rebuild = true;
    }

    fn perform_document_action(&mut self, action: DocumentAction) {
        if let Err(error) = self.state.guard_background_idle() {
            self.set_error(error);
            return;
        }
        let result = match action {
            DocumentAction::NewDatabase(path) => self.state.create_new_db(&path.to_string_lossy()),
            DocumentAction::NewMemory => self.state.new_memory_db(),
            DocumentAction::OpenDatabase(path, read_only) => if read_only {
                self.state.open_readonly_path(&path.to_string_lossy())
            } else { self.state.open_path(&path.to_string_lossy()) },
            DocumentAction::CloseDatabase => { self.state.close_db(); Ok(()) }
            DocumentAction::OpenProject(path) => self.state.open_project(&path),
            DocumentAction::CloseProject => { self.state.close_project(); Ok(()) }
            DocumentAction::CloseSqlTab(index) => { self.state.close_sql_tab(index); Ok(()) }
            DocumentAction::SelectSchemaNode(node) => {
                self.state.select_schema_node(node);
                self.reveal_schema_selection = true;
                Ok(())
            }
            DocumentAction::Exit => { self.exit_requested = true; Ok(()) }
        };
        match result {
            Ok(()) => {
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn save_documents_interactive(&mut self, always_project: bool) -> Result<bool, String> {
        let needs_project = always_project || self.state.has_project_drafts() || self.state.has_cell_draft() || self.state.has_pragma_drafts();
        let database = if self.state.path == ":memory:" {
            let Some(path) = file_dlg::pick_save_db() else { return Ok(false); };
            Some(path)
        } else { None };
        let project = if needs_project {
            match self.state.project_path.clone().map(PathBuf::from).or_else(file_dlg::pick_save_project) {
                Some(path) => Some(path),
                None => return Ok(false),
            }
        } else { None };
        self.state.save_documents(database.as_deref(), project.as_deref())?;
        Ok(true)
    }

    fn resolve_document_action(&mut self, decision: ModalAction) {
        self.state.confirm_open = false;
        self.state.confirm_kind = ConfirmKind::None;
        let Some(action) = self.state.pending_document.take() else { return; };
        if let DocumentAction::SelectSchemaNode(_) = action {
            match decision {
                ModalAction::Primary => {
                    self.state.sql_tabs.push(self.state.ddl_draft.clone());
                    self.state.switch_sql_tab(self.state.sql_tabs.len() - 1);
                    self.perform_document_action(action);
                    self.state.status_msg = "DDL draft kept in a new SQL tab".into();
                }
                ModalAction::Secondary => self.perform_document_action(action),
                ModalAction::Tertiary | ModalAction::Dismiss => { self.reveal_schema_selection = true; }
            }
            self.rebuild = true;
            return;
        }
        if let DocumentAction::CloseSqlTab(index) = action {
            match decision {
                ModalAction::Primary => {
                    let path = self.state.sql_files.get(index).and_then(|f| f.as_ref()).map(|f| f.path.clone()).or_else(||file_dlg::pick_save_sql(None));
                    if let Some(path) = path {
                        match self.state.save_sql_file(index, &path) {
                            Ok(()) => self.perform_document_action(action),
                            Err(e) => self.set_error(e),
                        }
                    }
                }
                ModalAction::Secondary => self.perform_document_action(action),
                ModalAction::Tertiary | ModalAction::Dismiss => {}
            }
            self.rebuild = true;
            return;
        }
        let result = match decision {
            ModalAction::Primary => self.save_documents_interactive(false),
            ModalAction::Secondary => self.state.discard_document_changes().map(|()| true),
            ModalAction::Tertiary | ModalAction::Dismiss => Ok(false),
        };
        match result {
            Ok(true) => self.perform_document_action(action),
            Ok(false) => self.state.status_msg = "Action canceled; documents remain open".into(),
            Err(e) => self.set_error(format!("Save/discard incomplete; documents remain open: {e}")),
        }
        self.rebuild = true;
    }

    fn action_new_db(&mut self) {
        if let Some(path) = file_dlg::pick_save_db() {
            self.request_document_action(DocumentAction::NewDatabase(path));
        }
    }

    fn action_open(&mut self, readonly: bool) {
        if let Some(path) = file_dlg::pick_open_db() {
            self.request_document_action(DocumentAction::OpenDatabase(path, readonly));
        }
    }

    fn action_save_sql(&mut self, save_as: bool) {
        self.snapshot_inputs();
        let path = if save_as { None } else {
            self.state.sql_files.get(self.state.sql_tab_idx).and_then(|f| f.as_ref()).map(|f| f.path.clone())
        }.or_else(||file_dlg::pick_save_sql(None));
        if let Some(path) = path {
            match self.state.save_sql_file(self.state.sql_tab_idx, &path) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
        }
    }

    fn action_new_sql_tab(&mut self) {
        self.snapshot_inputs();
        self.state.sql_tabs.push(String::new());
        self.state.switch_sql_tab(self.state.sql_tabs.len() - 1);
        self.state.status_msg = format!("SQL tab {}", self.state.sql_tab_idx + 1);
        self.rebuild = true;
    }

    fn action_write(&mut self) {
        match self.state.write_changes() {
            Ok(()) => self.clear_error(),
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_revert_ask(&mut self) {
        self.state.confirm_kind = ConfirmKind::Revert;
        self.state.confirm_open = true;
        self.rebuild = true;
    }

    fn action_create_table(&mut self) {
        self.state.open_create_table_wizard();
        self.rebuild = true;
    }

    fn action_create_index(&mut self) {
        self.state.open_create_index_wizard();
        self.rebuild = true;
    }

    fn action_import_csv(&mut self) {
        if let Some(paths) = file_dlg::pick_open_csv() {
            if !self.state.open_csv_files(paths) { self.state.status_msg = "Import CSV cancelled".into(); }
        } else { self.state.status_msg = "Import CSV cancelled".into(); }
        self.rebuild = true;
    }

    fn action_export_json(&mut self) {
        self.snapshot_inputs();self.open_data_export(true);
    }

    fn action_export_csv(&mut self) {
        self.snapshot_inputs();self.open_data_export(false);
    }

    fn action_import_sql(&mut self) {
        if let Err(e) = self.state.guard_writable() {
            self.set_error(e);
            self.rebuild = true;
            return;
        }
        let Some(path) = file_dlg::pick_open_sql() else {
            self.state.status_msg = "Import SQL cancelled".into();
            self.rebuild = true;
            return;
        };
        match dbmod::import_sql_file(self.state.conn.as_ref().unwrap(), &path) {
            Ok(()) => {
                self.state
                    .log_sql(format!("-- import sql {}", path.display()));
                self.state.mark_dirty();
                let _ = self.state.refresh_schema();
                let _ = self.state.refresh_browse();
                self.state.status_msg = "SQL import ok (unwritten)".into();
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_project_open(&mut self) {
        let Some(path) = file_dlg::pick_open_project() else {
            return;
        };
        self.request_document_action(DocumentAction::OpenProject(path));
    }

    fn action_project_save(&mut self, save_as: bool) {
        self.snapshot_inputs();
        let path = if !save_as {
            self.state
                .project_path
                .clone()
                .map(PathBuf::from)
                .or_else(file_dlg::pick_save_project)
        } else {
            file_dlg::pick_save_project()
        };
        let Some(path) = path else {
            return;
        };
        match self.state.save_project(&path) {
            Ok(()) => self.clear_error(),
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_tool_check(&mut self, pragma: &str) {
        if !self.require_conn() {
            return;
        }
        match dbmod::pragma_check(self.state.conn.as_ref().unwrap(), pragma) {
            Ok(res) => {
                self.state.sql_result = Some(res);
                self.state.tab = MainTab::ExecuteSql;
                self.state.sql_message = format!("PRAGMA {pragma} complete");
                self.state.log_sql(format!("PRAGMA {pragma}"));
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_row_counts(&mut self) {
        if !self.require_conn() {
            return;
        }
        match dbmod::row_counts_sql(self.state.conn.as_ref().unwrap()) {
            Ok(res) => {
                self.state.sql_result = Some(res);
                self.state.tab = MainTab::ExecuteSql;
                self.state.sql_message = "Row counts".into();
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn toggle_browse_freeze(&mut self, column: usize) {
        let count = if self.state.browse_frozen_cols() == 0 {
            column.saturating_add(1)
        } else {
            0
        };
        self.state.browse_mut().frozen_cols = Some(count);
        self.state.status_msg = format!("Frozen columns = {}", self.state.browse_frozen_cols());
        self.rebuild = true;
    }

    fn open_plot_context_menu(&mut self,x:f32,y:f32) -> bool {
        if self.state.overlay.is_some() || self.state.confirm_open || self.ui.active_dialog().is_some()
            || self.ids.chart.0==0 || self.ui.hit_widget(&self.scene,x,y)!=Some(self.ids.chart) {
            return false;
        }
        self.snapshot_inputs();
        self.browse_header_column=None;
        self.ui.set_focus(Some(self.ids.chart));
        self.ui.set_accessible_name(self.ids.context_menu,"Plot actions");
        self.ui.open_context_menu(self.ids.context_menu,vec![
            next_gui_ui::MenuItem::new(mid::PLOT_COPY,"Copy"),
            next_gui_ui::MenuItem::new(mid::PLOT_SAVE,"Save Image…"),
            next_gui_ui::MenuItem::new(mid::PLOT_FIT,"Fit to Data"),
            next_gui_ui::MenuItem::new(mid::PLOT_LEGEND,"Show legend").checked(self.state.plot.settings.show_legend),
            next_gui_ui::MenuItem::new(mid::PLOT_FIXED_FORMAT,"Fixed number format").checked(self.state.plot.settings.fixed_number_format),
            next_gui_ui::MenuItem::new(mid::PLOT_STACKED_BARS,"Stacked bars").checked(self.state.plot.settings.stacked_bars),
            next_gui_ui::MenuItem::new(mid::PLOT_HELP,"Help"),
        ],x,y)
    }

    fn handle_plot_context_key(&mut self,key:KeyCode,modifiers:Modifiers) -> bool {
        if key!=KeyCode::Function(10) || !modifiers.shift || modifiers.commandish() || modifiers.alt
            || self.ui.focused()!=Some(self.ids.chart) {return false;}
        let bounds=self.scene.iter_paint_order().into_iter().find(|n|self.ui.widget_from_node(n.id)==Some(self.ids.chart)).map(|n|n.bounds);
        bounds.is_some_and(|b|self.open_plot_context_menu(b.x+16.0,b.y+16.0))
    }

    fn fit_plot(&mut self) {
        if let Some(data)=self.ui.chart_plot_mut(self.ids.chart,0) {data.reset_view();}
        self.snapshot_inputs();
        self.rebuild=true;
    }

    fn handle_plot_menu_action(&mut self,item:u64) -> bool {
        match item {
            mid::PLOT_SAVE=>self.save_plot(),
            mid::PLOT_FIT=>self.fit_plot(),
            mid::PLOT_HELP=>self.open_url("https://github.com/sqlitebrowser/sqlitebrowser/wiki/Plot-Dock"),
            mid::PLOT_STACKED_BARS=>{self.state.plot.settings.stacked_bars = !self.state.plot.settings.stacked_bars;self.rebuild=true;}
            mid::PLOT_LEGEND=>{self.state.plot.settings.show_legend = !self.state.plot.settings.show_legend;self.rebuild=true;}
            mid::PLOT_FIXED_FORMAT=>{self.state.plot.settings.fixed_number_format = !self.state.plot.settings.fixed_number_format;self.rebuild=true;}
            _=>return false,
        }
        true
    }

    fn open_browse_header_menu(&mut self, x: f32, y: f32) -> bool {
        if self.state.tab != MainTab::Browse
            || self.state.overlay.is_some()
            || self.state.confirm_open
            || self.state.conn.is_none()
            || self.state.sql_busy
            || self.ui.hit_widget(&self.scene, x, y) != Some(self.ids.browse_table)
        {
            return false;
        }
        let Some(column) = self
            .ui
            .table_header_column_at(&self.scene, self.ids.browse_table, x, y)
        else {
            return false;
        };
        self.snapshot_inputs();
        self.browse_header_column = Some(column);
        self.ui.set_accessible_name(self.ids.context_menu,"Browse column actions");
        let can_select = !self.state.has_cell_draft()
            && self
                .state
                .browse()
                .result
                .as_ref()
                .is_some_and(|result| !result.rows.is_empty());
        let items = vec![
            next_gui_ui::MenuItem::new(
                mid::BROWSE_FREEZE,
                if self.state.browse_frozen_cols() == 0 {
                    "Freeze Columns"
                } else {
                    "Unfreeze Columns"
                },
            ),
            if can_select {
                next_gui_ui::MenuItem::new(mid::BROWSE_SELECT_COLUMN, "Select Column")
            } else {
                next_gui_ui::MenuItem::disabled(mid::BROWSE_SELECT_COLUMN, "Select Column")
            },
            next_gui_ui::MenuItem::new(mid::BROWSE_COPY_COLUMN, "Copy Column Name"),
        ];
        self.ui
            .open_context_menu(self.ids.context_menu, items, x, y)
    }

    fn browse_header_name(&self) -> Option<&str> {
        self.state
            .browse()
            .result
            .as_ref()?
            .columns
            .get(self.browse_header_column?)
            .map(String::as_str)
    }

    fn handle_browse_header_action(&mut self, item: u64) -> bool {
        if !matches!(item, mid::BROWSE_FREEZE | mid::BROWSE_SELECT_COLUMN) {
            return false;
        }
        let Some(column) = self.browse_header_column.take() else {
            return true;
        };
        self.ui.close_popup();
        if item == mid::BROWSE_FREEZE {
            self.toggle_browse_freeze(column);
        } else if self.state.has_cell_draft() {
            self.set_error("Apply or discard the cell draft first");
            self.rebuild = true;
        } else if let Some(last) = self
            .state
            .browse()
            .result
            .as_ref()
            .and_then(|result| result.rows.len().checked_sub(1))
        {
            // shortcut: column selection covers the current cache; extend with whole-result selection.
            let tab = self.state.browse_mut();
            tab.sel_row = Some(0);
            tab.sel_row_start = Some(0);
            tab.sel_row_end = Some(last);
            tab.sel_col = Some(column);
            tab.sel_col_start = Some(column);
            tab.sel_col_end = Some(column);
            self.state.sync_edit_cell_from_selection();
            self.state.status_msg = format!(
                "Selected {} records in column {}",
                last + 1,
                self.state.browse().result.as_ref().unwrap().columns[column]
            );
            self.reveal_browse_selection = true;
            self.rebuild = true;
        }
        true
    }

    fn handle_menu(&mut self, item: u64) {
        let view_action=(mid::VIEW_DOCK_EDIT..=mid::VIEW_DOCK_REMOTE).contains(&item);
        if !view_action {if let Err(error)=self.state.guard_background_idle() {self.set_error(error);return;}}
        if self.state.wizard_column_picker.is_some() { return; }
        self.snapshot_inputs();
        if self.handle_browse_header_action(item) { return; }
        let document_action = matches!(item, mid::FILE_NEW | mid::FILE_NEW_MEM | mid::FILE_OPEN | mid::FILE_OPEN_RO | mid::FILE_CLOSE | mid::FILE_PROJECT_OPEN | mid::FILE_PROJECT_CLOSE | mid::FILE_EXIT | mid::FILE_SAVE_ALL)
            || (mid::FILE_RECENT_BASE..mid::FILE_RECENT_BASE + 5).contains(&item);
        if self.state.has_cell_draft() && !document_action && !view_action {
            self.set_error("Apply or discard the cell draft first");
            self.rebuild = true;
            return;
        }
        if self.state.sql_busy && !view_action {
            self.set_error("Stop or finish SQL first");
            self.rebuild = true;
            return;
        }
        match item {
            mid::FILE_NEW => self.action_new_db(),
            mid::FILE_NEW_MEM => self.request_document_action(DocumentAction::NewMemory),
            mid::FILE_OPEN => self.action_open(false),
            mid::FILE_OPEN_RO => self.action_open(true),
            mid::FILE_CLOSE => self.request_document_action(DocumentAction::CloseDatabase),
            mid::FILE_SAVE_AS => {
                if let Some(path) = file_dlg::pick_save_db() {
                    match self.state.save_as(&path.display().to_string()) {
                        Ok(()) => self.clear_error(),
                        Err(e) => self.set_error(e),
                    }
                }
                self.rebuild = true;
            }
            mid::FILE_WRITE => self.action_write(),
            mid::FILE_REVERT => self.action_revert_ask(),
            mid::FILE_VACUUM => {
                if self.state.dirty {
                    self.set_error("Write Changes or Revert before compacting");
                    self.rebuild = true;
                    return;
                }
                if !self.require_conn() {
                    return;
                }
                match dbmod::vacuum(self.state.conn.as_ref().unwrap()) {
                    Ok(()) => {
                        self.state.log_sql("VACUUM");
                        self.state.dirty = false;
                        self.state.status_msg = "VACUUM done".into();
                        self.clear_error();
                    }
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            mid::FILE_IMPORT_CSV => self.action_import_csv(),
            mid::FILE_IMPORT_SQL => self.action_import_sql(),
            mid::FILE_EXPORT_CSV => self.action_export_csv(),
            mid::FILE_EXPORT_SQL => self.action_export_sql(),
            mid::FILE_EXIT => self.request_document_action(DocumentAction::Exit),
            mid::EDIT_CREATE_TABLE => {
                self.state.tab = MainTab::Structure;
                self.action_create_table();
            }
            mid::EDIT_CREATE_INDEX => {
                self.state.tab = MainTab::Structure;
                self.action_create_index();
            }
            mid::EDIT_DELETE => {
                if self.state.selected.is_none() {
                    self.set_error("Select an object to delete");
                    self.rebuild = true;
                } else if self.state.prefs.confirm_drops {
                    self.state.confirm_kind = ConfirmKind::DropObject;
                    self.state.confirm_open = true;
                    self.rebuild = true;
                } else {
                    self.apply_confirmed_action(ConfirmKind::DropObject);
                }
            }
            mid::VIEW_DOCK_EDIT => {
                self.state.dock_edit = !self.state.dock_edit;
                self.rebuild = true;
            }
            mid::VIEW_DOCK_SCHEMA => {
                self.state.dock_schema = !self.state.dock_schema;
                if self.state.dock_schema {
                    self.state.dock_focus = DockKind::Schema;
                }
                self.rebuild = true;
            }
            mid::VIEW_DOCK_LOG => {
                self.state.dock_log = !self.state.dock_log;
                if self.state.dock_log {
                    self.state.dock_focus = DockKind::Log;
                }
                self.rebuild = true;
            }
            mid::VIEW_DOCK_PLOT => {
                self.state.dock_plot = !self.state.dock_plot;
                if self.state.dock_plot {
                    self.state.dock_focus = DockKind::Plot;
                }
                self.rebuild = true;
            }
            mid::VIEW_THEME => {
                self.theme = match self.theme.mode {
                    ThemeMode::Dark => Theme::light(),
                    ThemeMode::Light => Theme::dark(),
                };
                self.rebuild = true;
            }
            mid::TOOLS_INTEGRITY => self.action_tool_check("integrity_check"),
            mid::TOOLS_QUICK => self.action_tool_check("quick_check"),
            mid::TOOLS_FK => self.action_tool_check("foreign_key_check"),
            mid::TOOLS_OPTIMIZE => self.action_tool_check("optimize"),
            mid::TOOLS_ROWCOUNTS => self.action_row_counts(),
            mid::FILE_ATTACH => {
                let Some(path) = file_dlg::pick_open_db() else {
                    return;
                };
                self.state.attach_path = path.display().to_string();
                self.state.overlay = Some(OverlayKind::Attach);
                self.rebuild = true;
            }
            mid::FILE_IMPORT_CLIP => {
                self.state.csv_work=None;self.state.csv_files=None;
                self.state.csv_paste.clear();
                self.state.csv_table = "clipboard".into();
                self.state.load_csv_import_settings(); self.state.csv_scroll = (0.0,0.0);
                self.state.overlay = Some(OverlayKind::PasteCsv);
                self.rebuild = true;
            }
            mid::FILE_EXPORT_JSON => self.action_export_json(),
            mid::EDIT_MODIFY => {
                let res = match self.state.selected.as_ref().map(|s| s.kind) {
                    Some(ObjectKind::Table) => self.state.open_modify_table_wizard(),
                    Some(ObjectKind::Index) => self.state.open_modify_index_wizard(),
                    _ => Err("Select a table or index to modify".into()),
                };
                match res {
                    Ok(()) => self.clear_error(),
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            mid::FILE_PROJECT_OPEN => self.action_project_open(),
            mid::FILE_PROJECT_SAVE => self.action_project_save(false),
            mid::FILE_PROJECT_SAVE_AS => self.action_project_save(true),
            mid::FILE_PROJECT_CLOSE => self.request_document_action(DocumentAction::CloseProject),
            mid::EDIT_PREFS => {
                self.state.sync_prefs_drafts();
                self.state.overlay = Some(OverlayKind::Preferences);
                self.rebuild = true;
            }
            mid::TOOLS_ENCRYPT => {
                self.state.cipher_open_path = None;
                self.state.overlay = Some(OverlayKind::Cipher);
                self.rebuild = true;
            }
            mid::VIEW_TB_DB => {
                self.state.toolbar_db = !self.state.toolbar_db;
                self.rebuild = true;
            }
            mid::VIEW_TB_EXTRA => {
                self.state.toolbar_extra = !self.state.toolbar_extra;
                self.rebuild = true;
            }
            mid::VIEW_TB_PROJECT => {
                self.state.toolbar_project = !self.state.toolbar_project;
                self.rebuild = true;
            }
            mid::VIEW_TB_SQL => {
                self.state.toolbar_sql = !self.state.toolbar_sql;
                self.rebuild = true;
            }
            mid::TOOLS_EXT => {
                self.state.overlay = Some(OverlayKind::LoadExtension);
                self.rebuild = true;
            }
            mid::FILE_SAVE_ALL => {
                match self.save_documents_interactive(true) {
                    Ok(true) => self.clear_error(),
                    Ok(false) => self.state.status_msg = "Save canceled".into(),
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            mid::FILE_PRINT_SCHEMA => {
                if !self.require_conn() {
                    return;
                }
                let Some(path) = file_dlg::pick_save_sql(None) else {
                    return;
                };
                match self.state.print_schema_to(&path) {
                    Ok(()) => self.clear_error(),
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            mid::VIEW_DOCK_REMOTE => {
                self.state.dock_remote = !self.state.dock_remote;
                if self.state.dock_remote {
                    self.state.dock_focus = DockKind::Remote;
                }
                self.rebuild = true;
            }
            mid::HELP_ABOUT => {
                self.state.overlay = Some(OverlayKind::About);
                self.rebuild = true;
            }
            mid::HELP_WEBSITE => self.open_url("https://sqlitebrowser.org/"),
            mid::HELP_WIKI => self.open_url("https://github.com/sqlitebrowser/sqlitebrowser/wiki"),
            mid::HELP_SQLCIPHER => {
                self.open_url("https://www.zetetic.net/sqlcipher/sqlcipher-api/")
            }
            mid::HELP_BUG => self.open_url("https://github.com/sqlitebrowser/sqlitebrowser/issues"),
            mid::HELP_FEATURE => {
                self.open_url("https://github.com/sqlitebrowser/sqlitebrowser/issues")
            }
            mid::HELP_DONATE => self.open_url("https://sqlitebrowser.org/donate/"),
            mid::HELP_UPDATES => {
                self.state.status_msg =
                    "Updates: use your package manager or sqlitebrowser.org downloads".into();
                self.rebuild = true;
            }
            x if (mid::FILE_RECENT_BASE..mid::FILE_RECENT_BASE + 5).contains(&x) => {
                let idx = (x - mid::FILE_RECENT_BASE) as usize;
                if let Some(path) = self.state.recent.get(idx).cloned() {
                    self.request_document_action(DocumentAction::OpenDatabase(PathBuf::from(path), false));
                    self.rebuild = true;
                }
            }
            _ => {}
        }
    }

    fn handle_overlay_button(&mut self, id: WidgetId) -> bool {
        if self.state.overlay.is_none() {
            return false;
        }
        if id == self.ids.ov_csv_advanced && id.0 != 0 {
            self.snapshot_inputs(); self.state.csv_advanced = !self.state.csv_advanced; self.rebuild = true; return true;
        }
        if id == self.ids.ov_csv_refresh && id.0 != 0 {
            self.snapshot_inputs(); self.state.csv_preview = None; self.rebuild = true; return true;
        }
        if id == self.ids.ov_btn_cancel {
            if matches!(self.state.overlay,Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv)) {
                self.state.csv_work=None;self.state.csv_files=None;self.state.csv_preview=None;
            }
            self.state.overlay = None;
            self.state.cipher_open_path = None;
            self.state.cipher_passphrase.clear();
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_ok {
            self.snapshot_inputs();
            let kind = self.state.overlay;
            let res = match kind {
                Some(OverlayKind::InsertRow) => self.state.apply_insert_row(),
                Some(OverlayKind::EditTable) => self.state.apply_table_wizard(),
                Some(OverlayKind::EditIndex) => self.state.apply_index_wizard(),
                Some(OverlayKind::ImportCsv) | Some(OverlayKind::PasteCsv) => {
                    self.state.apply_csv_import()
                }
                Some(OverlayKind::Attach) => self.state.apply_attach(),
                Some(OverlayKind::FindReplace) => {
                    self.state.overlay = None;
                    Ok(())
                }
                Some(OverlayKind::Cipher) => {
                    if self.state.cipher_open_path.is_some() {
                        self.state.apply_cipher_overlay()
                    } else if let Some(dest) = file_dlg::pick_save_db() {
                        self.state.apply_cipher_encrypt_file(&dest)
                    } else {
                        return true;
                    }
                }
                Some(OverlayKind::Preferences) => {
                    let r = self.state.apply_prefs_overlay();
                    if r.is_ok() {
                        self.theme = if self.state.prefs.default_theme_dark {
                            Theme::dark()
                        } else {
                            Theme::light()
                        };
                        self.state.overlay = None;
                    }
                    r
                }
                Some(OverlayKind::ExportSql) => {
                    self.action_export_sql_do();
                    return true;
                }
                Some(OverlayKind::CondFormat) => {
                    self.state.add_cond_format_rule();
                    Ok(())
                }
                Some(OverlayKind::LoadExtension) => {
                    let r = self.state.load_extension_apply();
                    if r.is_ok() {
                        self.state.overlay = None;
                    }
                    r
                }
                Some(OverlayKind::About) => {
                    self.state.overlay = None;
                    Ok(())
                }
                None => Ok(()),
            };
            match res {
                Ok(()) => {
                    if !matches!(
                        kind,
                        Some(OverlayKind::CondFormat) | Some(OverlayKind::FindReplace)
                    ) && matches!(
                        kind,
                        Some(OverlayKind::EditTable)
                            | Some(OverlayKind::EditIndex)
                            | Some(OverlayKind::Attach)
                            | Some(OverlayKind::Cipher)
                            | Some(OverlayKind::Preferences)
                    ) {
                        self.state.overlay = None;
                    }
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return true;
        }
        if self.state.overlay == Some(OverlayKind::EditIndex)
            && [self.ids.ov_btn_add, self.ids.ov_btn_del, self.ids.ov_btn_up, self.ids.ov_btn_down,
                self.ids.ov_idx_expression, self.ids.ov_idx_edit_key].contains(&id) {
            match self.edit_index_key(id) {
                Ok(()) => self.clear_error(),
                Err(error) => { self.set_error(error); self.rebuild = true; }
            }
            return true;
        }
        if self.state.overlay == Some(OverlayKind::EditTable)
            && [self.ids.ov_check_add,self.ids.ov_check_add_column,self.ids.ov_check_del,self.ids.ov_check_up,self.ids.ov_check_down].contains(&id) {
            let selected = self.state.wizard_check_sel;
            if id == self.ids.ov_check_add || id == self.ids.ov_check_add_column {
                let column = if id == self.ids.ov_check_add_column {
                    if !self.state.table_wizard_names_valid() { return true; }
                    let Some(column) = self.state.wizard_cols.get(self.state.wizard_col_sel) else { return true; }; Some(column.name.clone())
                } else { None };
                self.state.wizard_checks.push(table_sql::NamedCheck { column,..Default::default() });
                self.state.wizard_check_states.push(None);
                self.state.wizard_check_sel = self.state.wizard_checks.len() - 1;
            } else if id == self.ids.ov_check_del && selected < self.state.wizard_checks.len() {
                self.state.wizard_checks.remove(selected); self.state.wizard_check_states.remove(selected);
                self.state.wizard_check_sel = selected.min(self.state.wizard_checks.len().saturating_sub(1));
            } else if id == self.ids.ov_check_up && selected > 0
                && table_sql::checks_can_swap(&self.state.wizard_checks[selected],&self.state.wizard_checks[selected-1]) {
                self.state.wizard_checks.swap(selected,selected-1); self.state.wizard_check_states.swap(selected,selected-1);
                self.state.wizard_check_sel -= 1;
            } else if id == self.ids.ov_check_down && selected + 1 < self.state.wizard_checks.len()
                && table_sql::checks_can_swap(&self.state.wizard_checks[selected],&self.state.wizard_checks[selected+1]) {
                self.state.wizard_checks.swap(selected,selected+1); self.state.wizard_check_states.swap(selected,selected+1);
                self.state.wizard_check_sel += 1;
            }
            self.state.store_wizard_checks(); self.state.sync_wizard_check_fields(); self.refresh_table_check_controls();
            self.rebuild = true;
            return true;
        }
        if self.state.overlay == Some(OverlayKind::EditTable)
            && [self.ids.ov_key_add_pk,self.ids.ov_key_add_unique,self.ids.ov_key_del,self.ids.ov_key_up,self.ids.ov_key_down].contains(&id) {
            let selected = self.state.wizard_key_sel;
            if id == self.ids.ov_key_add_pk || id == self.ids.ov_key_add_unique {
                if id == self.ids.ov_key_add_pk && self.state.wizard_index_constraints.iter().any(|key| key.primary) { return true; }
                self.state.wizard_index_constraints.push(table_sql::NamedIndexConstraint { primary:id == self.ids.ov_key_add_pk,
                    columns:self.state.wizard_cols.get(self.state.wizard_col_sel).map(|column| dbmod::quote_ident(&column.name)).unwrap_or_default(), ..Default::default() });
                self.state.wizard_key_states.push(None); self.state.wizard_key_sel = self.state.wizard_index_constraints.len() - 1;
            } else if id == self.ids.ov_key_del && selected < self.state.wizard_index_constraints.len() {
                self.state.wizard_index_constraints.remove(selected); self.state.wizard_key_states.remove(selected);
                self.state.wizard_key_sel = selected.min(self.state.wizard_index_constraints.len().saturating_sub(1));
            } else if id == self.ids.ov_key_up || id == self.ids.ov_key_down {
                let target = if id == self.ids.ov_key_up { selected.checked_sub(1) } else { selected.checked_add(1) };
                let Some(target) = target.filter(|target| *target < self.state.wizard_index_constraints.len()) else { return true; };
                if [selected,target].into_iter().any(|index| table_sql::index_constraint_requires_inline(&self.state.wizard_index_constraints[index])) { return true; }
                self.state.wizard_index_constraints.swap(selected,target); self.state.wizard_key_states.swap(selected,target); self.state.wizard_key_sel = target;
                // Table declarations retain the requested order; inline INTEGER PRIMARY KEY DESC must stay inline.
                for key in &mut self.state.wizard_index_constraints { if !table_sql::index_constraint_requires_inline(key) { key.inline = false; } }
            }
            self.apply_table_index_flags(); self.rebuild = true; return true;
        }
        if self.state.overlay == Some(OverlayKind::EditTable)
            && [self.ids.ov_fk_add,self.ids.ov_fk_del,self.ids.ov_fk_up,self.ids.ov_fk_down].contains(&id) {
            let selected = self.state.wizard_foreign_key_sel;
            if id == self.ids.ov_fk_add {
                self.state.wizard_foreign_keys.push(table_sql::NamedForeignKey { name:None,
                    columns:self.state.wizard_cols.get(self.state.wizard_col_sel).map(|column|dbmod::quote_ident(&column.name)).unwrap_or_default(), clause:String::new(), inline:false });
                self.state.wizard_foreign_key_states.push([None, None]); self.state.wizard_foreign_key_sel = self.state.wizard_foreign_keys.len() - 1;
            } else if id == self.ids.ov_fk_del && selected < self.state.wizard_foreign_keys.len() {
                self.state.wizard_foreign_keys.remove(selected); self.state.wizard_foreign_key_states.remove(selected);
                self.state.wizard_foreign_key_sel = selected.min(self.state.wizard_foreign_keys.len().saturating_sub(1));
            } else if id == self.ids.ov_fk_up || id == self.ids.ov_fk_down {
                let target = if id == self.ids.ov_fk_up { selected.checked_sub(1) } else { selected.checked_add(1) };
                let Some(target) = target.filter(|target| *target < self.state.wizard_foreign_keys.len()) else { return true; };
                if !table_sql::foreign_keys_can_swap(&self.state.wizard_foreign_keys[selected],&self.state.wizard_foreign_keys[target]) { return true; }
                self.state.wizard_foreign_keys.swap(selected,target); self.state.wizard_foreign_key_states.swap(selected,target); self.state.wizard_foreign_key_sel = target;
            }
            self.state.store_wizard_foreign_keys(); self.state.sync_wizard_foreign_key_fields(); self.refresh_table_foreign_key_controls(); self.rebuild = true; return true;
        }

        if id == self.ids.ov_btn_add {
            self.flush_wizard_col();
            let mut name = "col".to_string();
            let mut suffix = 2;
            while self.state.wizard_cols.iter().any(|column| column.name.eq_ignore_ascii_case(&name)) {
                name = format!("col_{suffix}"); suffix += 1;
            }
            self.state.wizard_cols.push(dbmod::ColDef::new(&name, "TEXT"));
            self.state.wizard_column_edit_states.push([None, None, None]);
            self.state.wizard_constraint_columns.push(name);
            if self.state.wizard_table_original.is_some() { self.state.wizard_col_sources.push(None); }
            self.state.wizard_col_sel = self.state.wizard_cols.len() - 1;
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_del {
            self.flush_wizard_col();
            let row = self.state.wizard_col_sel;
            if row < self.state.wizard_cols.len() {
                if self.state.wizard_table_original.is_some() {
                    self.state.confirm_kind = ConfirmKind::DeleteColumn(row); self.state.confirm_open = true;
                } else {
                    match self.state.remove_wizard_column(row) { Ok(()) => self.clear_error(), Err(error) => self.set_error(error) }
                }
            }
            self.rebuild = true;
            return true;
        }
        if self.state.overlay==Some(OverlayKind::EditTable)
            && [self.ids.ov_field_top,self.ids.ov_btn_up,self.ids.ov_btn_down,self.ids.ov_field_bottom].contains(&id) {
            self.flush_wizard_col(); let selected=self.state.wizard_col_sel;
            let target = if id==self.ids.ov_field_top { 0 } else if id==self.ids.ov_field_bottom { self.state.wizard_cols.len().saturating_sub(1) }
                else if id==self.ids.ov_btn_up { selected.saturating_sub(1) } else { (selected+1).min(self.state.wizard_cols.len().saturating_sub(1)) };
            if let Err(error) = self.state.move_wizard_column(target) { self.set_error(error); }
            self.rebuild = true; return true;
        }
        if id==self.ids.ov_table_advanced && self.state.overlay==Some(OverlayKind::EditTable) {
            self.state.wizard_table_advanced = !self.state.wizard_table_advanced; self.rebuild=true; return true;
        }
        if id == self.ids.ov_btn_find || id == self.ids.ov_btn_find_all || id == self.ids.ov_btn_repl || id == self.ids.ov_btn_repl_all {
            self.snapshot_inputs();
            let action = if id == self.ids.ov_btn_find_all { sql_search::Action::FindAll }
                else if id == self.ids.ov_btn_repl { sql_search::Action::Replace }
                else if id == self.ids.ov_btn_repl_all { sql_search::Action::ReplaceAll }
                else { sql_search::Action::Find };
            self.search_sql(action);
            return true;
        }
        false
    }

    fn open_sql_find(&mut self) {
        self.snapshot_inputs();
        self.state.find_search.reset(self.ui.text_area_value(self.ids.sql_area).unwrap_or_default(),
            self.ui.text_area_selection(self.ids.sql_area).unwrap_or(0..0));
        self.state.find_message.clear();
        self.state.overlay = Some(OverlayKind::FindReplace);
        self.rebuild = true;
    }

    fn update_sql_hint(&mut self) {
        let area = self.ids.sql_area;
        let hint = if self.state.tab == MainTab::ExecuteSql && !self.state.confirm_open
            && matches!(self.state.overlay, None | Some(OverlayKind::FindReplace)) && self.ui.focused() == Some(area) {
            self.ui.text_area_caret(area).and_then(|caret| self.state.sql_completion.calltip(
                self.ui.text_area_value(area).unwrap_or_default(), caret))
        } else { None };
        if let Some(hint) = hint { self.ui.set_text_area_hints(area, hint.anchor, hint.items, hint.preferred); }
        else { self.ui.set_text_area_hint(area, 0, ""); }
    }

    fn handle_sql_completion_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if modifiers.commandish() && modifiers.shift && !modifiers.alt
            && self.state.tab == MainTab::ExecuteSql && !self.state.confirm_open
            && matches!(self.state.overlay, None | Some(OverlayKind::FindReplace))
            && self.ui.focused() == Some(self.ids.sql_area)
        {
            if matches!(key, KeyCode::Space | KeyCode::Char(' ')) {
                if self.ui.text_area_hint(self.ids.sql_area).is_some() {
                    self.ui.cycle_text_area_hint(self.ids.sql_area, true);
                } else {
                    self.ui.close_popup();
                    self.ui.set_text_area_hint(self.ids.sql_area, 0, "");
                    self.update_sql_hint();
                }
                return true;
            }
            if matches!(key, KeyCode::PageUp | KeyCode::PageDown) && self.ui.text_area_hint(self.ids.sql_area).is_some() {
                self.ui.scroll_text_area_hint(self.ids.sql_area,
                    self.win_h as f32 * if key == KeyCode::PageDown { 0.5 } else { -0.5 });
                return true;
            }
        }
        if modifiers.commandish() && !modifiers.shift && !modifiers.alt
            && matches!(key, KeyCode::Space | KeyCode::Char(' '))
            && self.state.tab == MainTab::ExecuteSql && self.ui.focused() == Some(self.ids.sql_area)
        {
            self.show_sql_completions(true);
            return true;
        }
        if !self.ui.text_area_completions_open(self.ids.sql_area) {
            return key == KeyCode::Escape && self.ui.text_area_hint(self.ids.sql_area).is_some()
                && self.ui.key_input(UiKey::Escape);
        }
        if !modifiers.commandish() && !modifiers.alt
            && matches!(key, KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End | KeyCode::Enter | KeyCode::Tab | KeyCode::Escape)
        {
            if let Some(key) = map_key(key, modifiers) { self.ui.key_input(key); }
            return true;
        }
        if modifiers.commandish() { self.ui.close_popup(); }
        false
    }

    fn input_key_with_sql_completions(&mut self, key: UiKey) -> bool {
        let was_open = self.ui.text_area_completions_open(self.ids.sql_area);
        let handled = self.ui.key_input(key);
        if handled && matches!(key, UiKey::Char(_) | UiKey::Backspace | UiKey::Delete | UiKey::DeleteWordLeft | UiKey::DeleteWordRight) {
            self.show_sql_completions(was_open);
        }
        handled
    }

    fn show_sql_completions(&mut self, explicit: bool) -> bool {
        let area = self.ids.sql_area;
        if self.state.tab != MainTab::ExecuteSql || self.state.confirm_open
            || !matches!(self.state.overlay, None | Some(OverlayKind::FindReplace))
            || self.ui.focused() != Some(area) || (!explicit && !self.state.prefs.sql_completion)
        { return false; }
        let Some(caret) = self.ui.text_area_caret(area) else { return false; };
        if self.ui.text_area_selection(area) != Some(caret..caret) { return false; }
        let suggestion = self.state.sql_completion.suggest(self.ui.text_area_value(area).unwrap_or_default(), caret, explicit, self.state.prefs.sql_upper_keywords);
        let Some((range, items)) = suggestion else {
            if self.ui.text_area_completions_open(area) { self.ui.close_popup(); }
            return false;
        };
        if explicit {
            if let Some(warning) = &self.state.sql_completion.warning {
                self.state.status_msg = format!("Some completion metadata is unavailable: {warning}");
                self.ui.set_text(self.ids.status, ui_build::status_text(&self.state));
            }
        }
        self.ui.show_text_area_completions(area, range, items)
    }

    fn handle_browse_navigation_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if self.state.tab != MainTab::Browse || self.state.overlay.is_some() || self.state.confirm_open
            || self.ids.browse_table.0 == 0 || self.ui.focused() != Some(self.ids.browse_table) || self.ui.has_open_popup()
            || modifiers.shift || modifiers.alt { return false; }
        if modifiers.commandish() && matches!(key, KeyCode::PageUp | KeyCode::PageDown) {
            self.snapshot_inputs();
            let names = self.state.browse_tables();
            if names.is_empty() { return true; }
            let current = names.iter().position(|(schema, name)| *schema == self.state.browse().schema && Some(*name) == self.state.browse().table.as_deref()).unwrap_or(0);
            let index = if key == KeyCode::PageDown { (current + 1) % names.len() }
                else if current == 0 { names.len() - 1 } else { current - 1 };
            let (schema, name) = (names[index].0.to_string(), names[index].1.to_string());
            match self.state.switch_browse_table_in(&schema, &name) {
                Ok(()) => self.clear_error(),
                Err(error) => self.set_error(error),
            }
            self.rebuild = true;
            return true;
        }
        let button = match key {
            KeyCode::PageUp if !modifiers.commandish() => self.ids.btn_record_prev,
            KeyCode::PageDown if !modifiers.commandish() => self.ids.btn_record_next,
            KeyCode::Home if modifiers.commandish() => self.ids.btn_record_first,
            KeyCode::End if modifiers.commandish() => self.ids.btn_record_last,
            _ => return false,
        };
        self.handle_button(button);
        true
    }

    fn handle_sql_search_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if key != KeyCode::Function(3) || modifiers.commandish() || modifiers.alt || self.state.tab != MainTab::ExecuteSql
            || self.state.confirm_open || !matches!(self.state.overlay, None | Some(OverlayKind::FindReplace)) { return false; }
        self.snapshot_inputs();
        if self.state.find_text.is_empty() { self.open_sql_find(); }
        else { self.search_sql(if modifiers.shift { sql_search::Action::FindReverse } else { sql_search::Action::Find }); }
        true
    }

    fn handle_sql_comment_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if key != KeyCode::Char('/') || !modifiers.commandish() || modifiers.alt || modifiers.shift
            || self.ui.focused() != Some(self.ids.sql_area) || self.ui.has_open_popup()
        { return false; }
        self.toggle_sql_comments()
    }

    fn toggle_sql_comments(&mut self) -> bool {
        if self.state.tab != MainTab::ExecuteSql || self.state.overlay.is_some() || self.state.confirm_open {
            return false;
        }
        let area = self.ids.sql_area;
        let Some(sql) = self.ui.text_area_value(area) else { return false; };
        let (range, text) = sql_editor::toggle_line_comment(sql, self.ui.text_area_selection(area).unwrap_or(0..0));
        if !self.ui.replace_text_area_range(area, range, &text) { return false; }
        self.ui.set_focus(Some(area));
        self.snapshot_inputs();
        self.state.status_msg = "Toggled comments on selected SQL lines".into();
        self.rebuild = true;
        true
    }

    fn search_sql(&mut self, action: sql_search::Action) {
        let area = self.ids.sql_area;
        let result = self.state.find_search.apply(action, self.ui.text_area_value(area).unwrap_or_default(),
            &self.state.find_text, &self.state.replace_text, self.ui.text_area_selection(area).unwrap_or(0..0));
        match result {
            Ok(outcome) => {
                self.ui.set_text_area_highlights(area, outcome.highlights, self.theme.colors.warning);
                if let Some(edit) = outcome.edit {
                    self.ui.replace_text_area_range(area, edit.range, &edit.text);
                }
                if let Some(selection) = outcome.selection { self.ui.select_text_area_range(area, selection); }
                self.state.find_message = outcome.message.clone();
                self.state.status_msg = outcome.message;
                self.clear_error();
                self.snapshot_inputs();
            }
            Err(e) => { self.state.find_message = e.clone(); self.set_error(e); }
        }
        self.rebuild = true;
    }

    fn execute_sql_selection(&mut self) {
        self.snapshot_inputs();
        let result = if let Some(sql) = self.ui.text_area_selected_text(self.ids.sql_area) {
            self.state.start_sql_text(sql)
        } else {
            self.state.start_sql_job()
        };
        match result {
            Ok(()) => self.clear_error(),
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn handle_button(&mut self, id: WidgetId) {
        if id.0 == 0 || self.ui.button_enabled(id) == Some(false) { return; }
        if id==self.ids.btn_dock_edit_close || id==self.ids.btn_dock_close {
            self.snapshot_inputs();
            if id==self.ids.btn_dock_edit_close {self.state.dock_edit=false;}else{
                match self.state.dock_focus {DockKind::EditCell=>self.state.dock_edit=false,DockKind::Schema=>self.state.dock_schema=false,DockKind::Log=>self.state.dock_log=false,DockKind::Plot=>self.state.dock_plot=false,DockKind::Remote=>self.state.dock_remote=false}
            }
            self.rebuild=true;return;
        }
        if self.state.plot_work.is_some() { self.handle_plot_load_button(id); return; }
        if id == self.ids.plot.save { self.save_plot(); return; }
        if id == self.ids.plot.load_all {
            self.snapshot_inputs();
            match self.state.start_plot_load() { Ok(()) => self.clear_error(), Err(error) => self.set_error(error) }
            self.rebuild = true;
            return;
        }
        if self.state.overlay==Some(OverlayKind::ExportSql) {self.handle_sql_export_button(id);return;}
        if self.state.export_dialog.is_some() {self.handle_data_export_button(id);return;}
        if self.state.csv_work.is_some() {self.handle_csv_answer_button(id);return;}
        if self.state.wizard_column_picker.is_some() {self.handle_column_picker_button(id);return;}
        if id==self.ids.reference_editor.reset {
            if let Some(draft)=self.state.wizard_constraint_edit.as_mut().and_then(|edit|edit.reference.as_mut()) {draft.reset();self.do_rebuild();self.snapshot_reference_cell(false);self.flush_table_foreign_key();}
            else if let Some(draft)=self.state.wizard_field_edit.as_mut().and_then(|edit|edit.reference.as_mut()) {draft.reset();self.do_rebuild();self.snapshot_field_sql(false);self.flush_wizard_col();}
            return;
        }
        self.snapshot_inputs();
        if self.handle_csv_files_button(id) { return; }
        if id == self.ids.log_clear {
            self.state.sql_log.clear(crate::sql_log::Source::from_filter(self.state.sql_log_filter));
            self.state.sql_log_view = None;
            self.rebuild = true;
            return;
        }
        if id == self.ids.log_export {
            if let Some(path) = file_dlg::pick_save_log() {
                match self.state.export_sql_log(&path) {
                    Ok(()) => { self.clear_error(); self.state.status_msg = format!("SQL log exported → {}", path.display()); }
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            return;
        }
        if id == self.ids.btn_edit_discard && id.0 != 0 {
            self.state.discard_cell_draft();
            self.clear_error();
            self.rebuild = true;
            return;
        }
        if id.0 != 0 && [self.ids.image_fit, self.ids.image_actual, self.ids.image_in, self.ids.image_out].contains(&id) {
            let mut view = self.state.edit_image_view;
            let (w, h) = self.state.edit_image_rgba.as_ref().map(|(w,h,_)| (*w as f32,*h as f32)).unwrap_or((1.0,1.0));
            let bounds = self.scene.iter_paint_order().into_iter().find(|n| self.ui.widget_from_node(n.id) == Some(self.ids.edit_image)).map(|n| n.bounds);
            let fit = bounds.map_or(1.0, |b| (b.width/w).min((b.height-18.0).max(1.0)/h));
            view.scale = if id == self.ids.image_fit { None }
                else if id == self.ids.image_actual { Some(1.0) }
                else { Some((view.scale.unwrap_or(fit) * if id == self.ids.image_in { 2.0 } else { 0.5 }).clamp(1.0/64.0,64.0)) };
            self.ui.set_image_view(self.ids.edit_image, view);
            self.state.edit_image_view = self.ui.image_view(self.ids.edit_image).unwrap_or_default();
            self.rebuild = true;
            return;
        }
        if id == self.ids.image_export && id.0 != 0 {
            if let Some(path) = file_dlg::pick_save_image() {
                let result = (|| {
                    let conn = self.state.conn.as_ref().ok_or("No database open")?;
                    dbmod::guard_output_path(conn, &path)?;
                    let rusqlite::types::Value::Blob(bytes) = &self.state.edit_original else { return Err("Select an image BLOB".into()); };
                    std::fs::write(&path, bytes).map_err(|e| e.to_string())
                })();
                match result { Ok(()) => self.clear_error(), Err(e) => self.set_error(e) }
                self.rebuild = true;
            }
            return;
        }
        if id == self.ids.toolbar_project {
            self.handle_menu(mid::FILE_PROJECT_OPEN);
            return;
        }
        if id == self.ids.btn_remote_open {
            if let Some(i) = self.ui.list_selected(self.ids.remote_list) { self.state.remote_sel = i; }
            if let Some(path) = self.state.remote_paths.get(self.state.remote_sel).cloned() {
                self.request_document_action(DocumentAction::OpenDatabase(PathBuf::from(path), false));
            }
            return;
        }
        if id == self.ids.btn_new || id == self.ids.btn_open {
            self.handle_menu(if id == self.ids.btn_new { mid::FILE_NEW } else { mid::FILE_OPEN });
            return;
        }
        if self.state.has_cell_draft() && ![
            self.ids.btn_edit_cell, self.ids.btn_edit_text, self.ids.btn_edit_hex,
            self.ids.btn_edit_json, self.ids.btn_edit_image,
        ].contains(&id) {
            self.set_error("Apply or discard the cell draft first");
            self.rebuild = true;
            return;
        }
        if self.state.sql_busy && id != self.ids.btn_sql_stop {
            return;
        }
        if self.handle_overlay_button(id) {
            return;
        }
        if let Some(column) = self.ids.form_fields.iter().position(|field| *field == id) {
            self.state.browse_mut().sel_col = Some(column);
            self.state.sync_edit_cell_from_selection();
            self.state.dock_edit = true;
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_theme {
            self.handle_menu(mid::VIEW_THEME);
            return;
        }
        if id == self.ids.btn_write {
            self.action_write();
            return;
        }
        if id == self.ids.btn_revert {
            self.action_revert_ask();
            return;
        }
        if id == self.ids.btn_struct_new_table {
            self.state.open_create_table_wizard();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_struct_new_index {
            self.state.open_create_index_wizard();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_struct_modify {
            self.handle_menu(mid::EDIT_MODIFY);
            return;
        }
        if id == self.ids.btn_struct_delete {
            self.handle_menu(mid::EDIT_DELETE);
            return;
        }
        if id == self.ids.btn_struct_refresh {
            self.snapshot_inputs();
            let _ = self.state.refresh_schema();
            let _ = self.state.refresh_browse();
            let _ = self.state.refresh_pragmas();
            let _ = self.state.refresh_chart();
            self.state.status_msg = "Refreshed".into();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_run_ddl {
            if self.state.selected.as_ref().is_some_and(|s| s.schema != "main") {
                self.set_error("Use Execute SQL with a qualified object name for attached/temp DDL");
                self.rebuild = true;
                return;
            }
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            self.snapshot_inputs();
            let sql = self.state.ddl_draft.clone();
            match self.state.run_sql_text_sync(&sql) {
                Ok(_) => {
                    self.state.status_msg = "DDL executed".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_clear_sort {
            match self.state.set_browse_ordering(Vec::new()) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_record_first || id == self.ids.btn_record_last || id == self.ids.btn_record_go
            || id == self.ids.btn_record_prev || id == self.ids.btn_record_next {
            let record = if id == self.ids.btn_record_first { Ok(1) }
                else if id == self.ids.btn_record_last { Ok(usize::MAX) }
                else if id == self.ids.btn_record_prev || id == self.ids.btn_record_next {
                    let tab = self.state.browse();
                    let current = tab.offset.saturating_add(tab.sel_row.unwrap_or(0)).saturating_add(1);
                    let step = self.ui.table_visible_rows(self.ids.browse_table).unwrap_or(2).saturating_sub(1).max(1);
                    Ok(if tab.sel_row.is_none() { 1 }
                        else if id == self.ids.btn_record_next { current.saturating_add(step) }
                        else { current.saturating_sub(step).max(1) })
                }
                else { self.state.browse().goto_record.trim().parse::<i128>()
                    .map(|n| usize::try_from(n.max(1)).unwrap_or(usize::MAX))
                    .map_err(|_| "Enter a whole record number".to_string()) };
            match record.and_then(|n| self.state.goto_browse_record(n)) {
                Ok(()) => { self.clear_error(); self.reveal_browse_selection = true; }
                Err(error) => self.set_error(error),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_go || id == self.ids.btn_browse_clear_filters {
            self.snapshot_inputs();
            if let Err(error) = self.apply_browse_select() {
                self.set_error(error);
                self.rebuild = true;
                return;
            }
            if id == self.ids.btn_browse_clear_filters {
                let tab = self.state.browse_mut();
                tab.filter.clear();
                tab.global_filters.clear();
                tab.col_filters.clear();
            }
            self.state.browse_mut().offset = 0;
            match self.state.refresh_browse() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_add_row {
            match self.state.open_insert_row() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_del_row {
            self.state.confirm_kind = ConfirmKind::DeleteRow;
            self.state.confirm_open = true;
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_cell {
            if self.state.browse().sel_row.is_none() {
                self.state.browse_mut().sel_row = self.ui.table_selected_row(self.ids.browse_table);
            }
            match self.state.apply_cell_draft() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_pragma_apply {
            self.snapshot_inputs();
            if self.state.dirty {
                self.state.confirm_kind = ConfirmKind::Pragmas;
                self.state.confirm_open = true;
                self.rebuild = true;
                return;
            }
            match self.state.save_pragmas() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_new_tab {
            if let Err(e) = self.state.add_browse_tab() { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_close_tab {
            if let Err(e) = self.state.close_browse_tab() { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        for (i, bid) in self.ids.browse_tab_btns.iter().enumerate() {
            if id == *bid {
                self.snapshot_inputs();
                if let Err(e) = self.state.switch_browse_tab(i) { self.set_error(e); }
                self.rebuild = true;
                return;
            }
        }
        if id == self.ids.btn_sql_csv || id == self.ids.btn_sql_json {
            self.open_query_export(id == self.ids.btn_sql_json);
            return;
        }
        if id == self.ids.btn_sql_stop {
            self.state.request_sql_stop();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_find {
            self.open_sql_find();
            return;
        }
        for (i, bid) in self.ids.sql_tab_btns.iter().enumerate() {
            if id == *bid {
                self.snapshot_inputs();
                self.state.switch_sql_tab(i);
                self.rebuild = true;
                return;
            }
        }
        if id == self.ids.btn_sql_line {
            self.snapshot_inputs();
            match self
                .state
                .execute_current_line(self.ui.text_area_caret(self.ids.sql_area).unwrap_or(0))
            {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_comment {
            self.toggle_sql_comments();
            return;
        }
        if id == self.ids.btn_sql_view {
            self.snapshot_inputs();
            match self.state.save_results_as_view(&self.state.sql_view_name.clone()) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_freeze {
            self.toggle_browse_freeze(self.state.browse().sel_col.unwrap_or(0));
            return;
        }
        if id == self.ids.btn_cond_fmt {
            if self.state.cond_col.is_empty() {
                self.state.cond_col = self.state.edit_col.clone();
            }
            self.state.overlay = Some(OverlayKind::CondFormat);
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_fk_nav {
            self.snapshot_inputs();
            match self.state.navigate_fk() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_text {
            if let Err(e) = self.state.set_edit_mode(crate::state::EditCellMode::Text) { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_hex {
            if let Err(e) = self.state.set_edit_mode(crate::state::EditCellMode::Hex) { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_json {
            if let Err(e) = self.state.set_edit_mode(crate::state::EditCellMode::Json) { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_image {
            if let Err(e) = self.state.set_edit_mode(crate::state::EditCellMode::Image) { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_form_view {
            self.state.toggle_browse_view();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_form_prev {
            if let Err(e) = self.state.form_prev_record() { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_form_next {
            if let Err(e) = self.state.form_next_record() { self.set_error(e); }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_remote_add {
            self.snapshot_inputs();
            if self.ids.remote_draft.0 != 0 {
                if let Some(v) = self.ui.text_input_value(self.ids.remote_draft) {
                    self.state.remote_draft = v.to_string();
                }
            }
            self.state.remote_add_path();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_project_save {
            self.action_project_save(false);
            return;
        }
        if id == self.ids.btn_integrity {
            self.action_tool_check("integrity_check");
            return;
        }
        if id == self.ids.toolbar_extra {
            self.handle_menu(mid::FILE_VACUUM);
            return;
        }
        if id == self.ids.btn_sql_run {
            self.execute_sql_selection();
            return;
        }
        if id == self.ids.btn_sql_new_tab {
            self.action_new_sql_tab();
            return;
        }
        if id == self.ids.btn_sql_open {
            if let Some(path) = file_dlg::pick_open_sql() {
                match self.state.open_sql_file(&path) {
                    Ok(()) => self.clear_error(),
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            return;
        }
        if id == self.ids.btn_sql_save || id == self.ids.btn_sql_save_as {
            self.action_save_sql(id == self.ids.btn_sql_save_as);
            return;
        }
        if id == self.ids.btn_sql_close {
            self.request_document_action(DocumentAction::CloseSqlTab(self.state.sql_tab_idx));
            return;
        }
        if id == self.ids.btn_sql_left || id == self.ids.btn_sql_right {
            let index = self.state.sql_tab_idx;
            let next = if id == self.ids.btn_sql_left { index.saturating_sub(1) } else { (index + 1).min(self.state.sql_tabs.len() - 1) };
            self.state.move_sql_tab(index, next);
            self.rebuild = true;
            return;
        }
        if id == self.ids.plot.fit && id.0 != 0 {
            self.fit_plot();
            return;
        }
        if id == self.ids.btn_chart && id.0 != 0 {
            self.snapshot_inputs();
            self.state.plot.settings.manual = true;
            self.apply_chart_select();
            match self.state.refresh_chart() {
                Ok(()) => {
                    self.state.dock_plot = true;
                    self.state.dock_focus = DockKind::Plot;
                    self.state.status_msg = format!(
                        "Plot {} pts from {}.{}",
                        self.state.chart_points.len(),
                        self.state.chart_table.as_ref().map(|name| dbmod::qualified_name(&self.state.chart_schema, name)).unwrap_or_else(|| "?".into()),
                        dbmod::quote_ident(&self.state.chart_column)
                    );
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
        }
    }

    fn apply_browse_select(&mut self) -> Result<(), String> {
        if self.ids.browse_select.0 == 0 {
            return Ok(());
        }
        let opts = self.state.browse_tables();
        if let Some(i) = self.ui.select_selected(self.ids.browse_select) {
            if let Some((schema, name)) = opts.get(i).map(|(s, t)| (s.to_string(), t.to_string())) {
                self.state.switch_browse_table_in(&schema, &name)?;
            }
        }
        Ok(())
    }

    fn apply_chart_select(&mut self) {
        if self.ids.chart_select.0 == 0 {
            return;
        }
        let opts = self.state.browse_tables();
        if let Some(i) = self.ui.select_selected(self.ids.chart_select) {
            if let Some((schema, name)) = opts.get(i).map(|(schema, name)| (schema.to_string(), name.to_string())) {
                self.state.chart_schema = schema;
                self.state.chart_table = Some(name);
            }
        }
    }

    fn link_plot_selection(&mut self) {
        if self.state.plot.settings.manual || self.state.plot_work.is_some() || self.last_tab != self.state.tab.index() { return; }
        let rows = self.state.plot.data.state.selected_rows.clone();
        if rows == self.state.plot.linked_rows { return; }
        if self.state.has_cell_draft() {
            let previous = self.state.plot.linked_rows.clone();
            self.state.plot.data.state.selected_rows = previous.clone();
            if let Some(data) = self.ui.chart_plot_mut(self.ids.chart, 0) { data.state.selected_rows = previous; }
            self.set_error("Apply or discard the cell draft first");
            self.rebuild = true;
            return;
        }
        if self.state.tab == MainTab::Browse && self.state.plot.has_full() {
            if let Some(&row) = rows.first() {
                let tab = self.state.browse();
                let end = tab.offset.saturating_add(tab.result.as_ref().map_or(0, |result| result.rows.len()));
                if row < tab.offset || row >= end {
                    if let Err(error) = self.state.goto_browse_record(row.saturating_add(1)) { self.set_error(error); self.rebuild = true; return; }
                    self.state.plot.linked_rows = rows;
                    self.rebuild = true;
                    return;
                }
            }
        }
        let target = match (&self.state.plot.source, self.state.tab) {
            (Some(plot::Source::Query(index)), MainTab::ExecuteSql) if *index == self.state.sql_tab_idx => Some((self.ids.sql_table, 0)),
            (Some(plot::Source::Table { schema, table }), MainTab::Browse) if schema == &self.state.browse().schema && self.state.browse().table.as_ref() == Some(table) => Some((self.ids.browse_table, self.state.browse().offset)),
            _ => None,
        };
        if let Some((table, offset)) = target.filter(|(id, _)| id.0 != 0) {
            let count = if table == self.ids.sql_table {
                self.state.sql_result.as_ref().map_or(0, |result| result.all_values.as_ref().map_or(result.rows.len(), |values| values.len()))
            } else { self.state.browse().result.as_ref().map_or(0, |result| result.rows.len()) };
            let mut visible = rows.range(offset..offset.saturating_add(count));
            let range = visible.next().copied().map(|first| (first - offset, visible.next_back().copied().unwrap_or(first) - offset));
            if self.ui.set_table_selected_rows(table, range) {
                if self.ui.table_selected_col(table).is_none() { self.ui.set_table_selected_col(table, Some(0)); }
                self.ui.reveal_table_selection(table);
                if table == self.ids.sql_table {
                    self.state.sql_selection.rows = range;
                    self.state.sql_selection.columns = self.ui.table_selection_anchor(table).and_then(|(_, column)| column).zip(self.ui.table_selected_col(table));
                    self.state.sql_scroll = self.ui.table_scroll_offset(table).unwrap();
                }
            }
        }
        self.state.plot.linked_rows = rows;
    }

    fn poll_selection_changes(&mut self) {
        if let Some((_, error)) = self.ui.take_table_data_error() { self.set_error(error); }
        if self.ui.is_dragging() { return; }
        if let Some(mode) = self.ui.select_selected(self.ids.plot.mode) {
            if (mode == 1) != self.state.plot.settings.manual {
                self.state.plot.settings.manual = mode == 1;
                self.rebuild = true;
            }
        }
        if !self.state.plot.settings.manual {
            if let Some(show) = self.ui.is_checked(self.ids.plot.legend) {
                if show != self.state.plot.settings.show_legend {
                    self.state.plot.settings.show_legend = show;
                    self.state.plot.data.show_legend = show;
                    self.rebuild = true;
                }
            }
            let (line, marker) = self.state.plot.styles();
            let line = self.ui.select_selected(self.ids.plot.line).unwrap_or(line);
            let marker = self.ui.select_selected(self.ids.plot.marker).unwrap_or(marker);
            if self.state.plot.set_styles(line, marker) { self.rebuild = true; }
            let selected = self.ui.table_selected_row(self.ids.plot.columns).zip(self.ui.table_selected_col(self.ids.plot.columns));
            if selected != self.state.plot.selected { self.state.plot.selected = selected; self.rebuild = true; }
        }
        if let Some(index) = self.ui.select_selected(self.ids.log_filter) {
            if index != self.state.sql_log_filter {
                self.state.sql_log_filter = index;
                self.state.sql_log_view = None;
                self.rebuild = true;
            }
        }
        self.link_plot_selection();
        if self.state.has_cell_draft() {
            let selection_changed = (self.ids.browse_table.0 != 0
                && (self.ui.table_selected_row(self.ids.browse_table)
                    != self.state.browse().sel_row
                    || self
                        .ui
                        .table_selected_col(self.ids.browse_table)
                        .is_some_and(|col| Some(col) != self.state.browse().sel_col)))
                || self
                    .ui
                    .tabs_selected(self.ids.tabs)
                    .is_some_and(|i| i != self.last_tab)
                || self.ui.tree_selected(self.ids.tree) != self.last_tree_sel
                || self.ui.select_selected(self.ids.browse_select) != self.last_browse_sel;
            if selection_changed {
                self.state.tab = MainTab::from_index(self.last_tab);
                self.set_error("Apply or discard the cell draft first");
                self.rebuild = true;
            }
            return;
        }
        let mut dirty = false;
        if let Some(sel) = self.ui.tree_selected(self.ids.tree) {
            if self.last_tree_sel != Some(sel) {
                self.last_tree_sel = Some(sel);
                self.request_document_action(DocumentAction::SelectSchemaNode(sel));
                return;
            }
        }
        if let Some(tab) = self.ui.tabs_selected(self.ids.tabs) {
            if tab != self.last_tab {
                self.last_tab = tab;
                self.state.tab = MainTab::from_index(tab);
                dirty = true;
            }
        }
        if self.ids.dock_tabs.0 != 0 {
            if let Some(i) = self.ui.tabs_selected(self.ids.dock_tabs) {
                let docks = self.state.visible_dock_tabs();
                if let Some(d) = docks.get(i) {
                    if self.state.dock_focus != *d {
                        self.state.dock_focus = *d;
                        dirty = true;
                    }
                }
            }
        }
        if let Some(i) = self.ui.select_selected(self.ids.browse_select) {
            if self.last_browse_sel != Some(i) {
                self.last_browse_sel = Some(i);
                match self.apply_browse_select() {
                    Ok(()) => self.clear_error(),
                    Err(error) => self.set_error(error),
                }
                self.rebuild = true;
                return;
            }
        }
        if let Some(row) = self.ui.table_selected_row(self.ids.browse_table) {
            let col = self.ui.table_selected_col(self.ids.browse_table);
            let (anchor_row, anchor_col) = self.ui.table_selection_anchor(self.ids.browse_table)
                .unwrap_or((row, col));
            let tab = self.state.browse();
            if tab.sel_row != Some(row) || tab.sel_col != col
                || tab.sel_row_start != Some(anchor_row) || tab.sel_col_start != anchor_col
            {
                self.last_browse_row = Some(row);
                let tab = self.state.browse_mut();
                tab.sel_row = Some(row);
                tab.sel_col = col;
                tab.sel_row_start = Some(anchor_row);
                tab.sel_row_end = Some(row);
                tab.sel_col_start = anchor_col;
                tab.sel_col_end = col;
                self.state.sync_edit_cell_from_selection();
                dirty = true;
            }
        }
        dirty |= self.sync_cell_selection();
        if dirty {
            self.rebuild = true;
        }
    }

    fn sync_cell_selection(&mut self) -> bool {
        if !self.state.dock_edit || self.state.has_cell_draft() { return false; }
        match self.state.sync_edit_cell_for_page() {
            Ok(changed) => changed,
            Err(error) => { self.set_error(error); true }
        }
    }

    fn poll_header_sort(&mut self, append: bool) {
        if self.state.has_cell_draft() {
            if self.ui.take_table_header_click().is_some() {
                self.set_error("Apply or discard the cell draft first");
                self.rebuild = true;
            }
            return;
        }
        if let Some((tid, col_idx)) = self.ui.take_table_header_click() {
            if tid != self.ids.browse_table {
                return;
            }
            let Some(res) = self.state.browse().result.as_ref() else {
                return;
            };
            let Some(col_name) = res.columns.get(col_idx).cloned() else {
                return;
            };
            match self.state.sort_browse_column(col_name, append) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
        }
    }

    fn handle_modal(&mut self) {
        if let Some((mid_w, action)) = self.ui.take_modal_action() {
            if mid_w != self.ids.modal {
                return;
            }
            let kind = self.state.confirm_kind;
            self.state.confirm_open = false;
            self.state.confirm_kind = ConfirmKind::None;
            if kind == ConfirmKind::Documents {
                self.resolve_document_action(action);
                return;
            }
            if matches!(kind, ConfirmKind::DeleteColumn(_)) { self.ui.set_focus(Some(self.ids.ov_fields_table)); }
            let accepted = if matches!(kind, ConfirmKind::DeleteColumn(_) | ConfirmKind::Pragmas) { action == ModalAction::Secondary } else { action == ModalAction::Primary };
            if !accepted { self.rebuild = true; return; }
            self.apply_confirmed_action(kind);
        }
    }

    fn apply_confirmed_action(&mut self, kind: ConfirmKind) {
        match kind {
            ConfirmKind::Pragmas => match self.state.apply_pragmas(true) {
                Ok(()) => self.clear_error(),
                Err(error) => self.set_error(error),
            },
            ConfirmKind::DeleteColumn(row) => match self.state.remove_wizard_column(row) {
                Ok(()) => { self.clear_error(); self.state.status_msg = "Column removed from table draft".into(); }
                Err(error) => self.set_error(error),
            },
            ConfirmKind::Revert => match self.state.revert_changes() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            },
            ConfirmKind::DropObject => {
                if let Some(sel) = self.state.selected.clone() {
                    if let Err(e) = self.state.guard_writable() {
                        self.set_error(e);
                    } else if let Some(conn) = self.state.conn.as_ref() {
                        let unchanged_definition = self.state.schema.iter().find(|o| o.schema == sel.schema && o.kind == sel.kind && o.name == sel.name)
                            .and_then(|o| o.sql.as_deref()).unwrap_or_default() == self.state.ddl_draft;
                        match dbmod::drop_object(conn, sel.kind, &sel.schema, &sel.name) {
                            Ok(()) => {
                                self.state
                                    .log_sql(format!("DROP {:?} {}.{}", sel.kind, dbmod::quote_ident(&sel.schema), dbmod::quote_ident(&sel.name)));
                                self.state.mark_dirty();
                                self.state.selected = None;
                                if unchanged_definition { self.state.ddl_draft.clear(); }
                                if self.state.browse().schema == sel.schema && self.state.browse().table.as_deref() == Some(&sel.name)
                                    && matches!(sel.kind, ObjectKind::Table | ObjectKind::View) {
                                    self.state.browse_mut().table = None;
                                }
                                let _ = self.state.refresh_schema();
                                let _ = self.state.refresh_browse();
                                self.state.status_msg = format!("Dropped {}.{} (unwritten)", sel.schema, sel.name);
                                self.clear_error();
                            }
                            Err(e) => self.set_error(e),
                        }
                    }
                }
            }
            ConfirmKind::DeleteRow => {
                if let Err(e) = self.state.guard_writable() {
                    self.set_error(e);
                } else {
                    let table = self.state.browse().table.clone();
                    let row = self
                        .state
                        .browse()
                        .sel_row
                        .or_else(|| self.ui.table_selected_row(self.ids.browse_table));
                    if let (Some(table), Some(row)) = (table, row) {
                        if let Some(&rowid) = self.state.browse().rowids.get(row) {
                            if let Some(conn) = self.state.conn.as_ref() {
                                match dbmod::delete_row_by_rowid_in(conn, &self.state.browse().schema, &table, rowid) {
                                    Ok(_) => {
                                        self.state.log_sql(format!(
                                            "DELETE FROM {} WHERE rowid={rowid}", dbmod::qualified_name(&self.state.browse().schema, &table)
                                        ));
                                        self.state.mark_dirty();
                                        let _ = self.state.refresh_browse();
                                        self.state.status_msg = "Row deleted (unwritten)".into();
                                        self.clear_error();
                                    }
                                    Err(e) => self.set_error(e),
                                }
                            }
                        } else {
                            self.set_error("No rowid");
                        }
                    }
                }
            }
            ConfirmKind::None | ConfirmKind::Documents => {}
        }
        self.rebuild = true;
    }

    fn toggle_backend(&mut self, w: u32, h: u32) {
        match self.window_backend {
            PresentBackend::GpuSurface => {
                if self.using_gpu {
                    if let Backend::Gpu(gpu) = std::mem::replace(
                        &mut self.backend,
                        Backend::Soft(SoftRenderer::new(w, h, clear_color(&self.theme))),
                    ) {
                        self.surface_gpu = Some(gpu);
                        self.using_gpu = false;
                    }
                } else if let Some(mut gpu) = self.surface_gpu.take() {
                    gpu.resize(w, h);
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
            }
            PresentBackend::Softbuffer => {
                if !self.using_gpu {
                    match self.gpu_stash.take() {
                        Some(mut gpu) => {
                            gpu.resize(w, h);
                            self.backend = Backend::Gpu(gpu);
                            self.using_gpu = true;
                        }
                        None => match GpuRenderer::try_new(w, h, clear_color(&self.theme)) {
                            Ok(gpu) => {
                                self.backend = Backend::Gpu(gpu);
                                self.using_gpu = true;
                            }
                            Err(e) => {
                                eprintln!("toggle: GPU unavailable: {e}");
                                return;
                            }
                        },
                    }
                } else if let Backend::Gpu(gpu) = std::mem::replace(
                    &mut self.backend,
                    Backend::Soft(SoftRenderer::new(w, h, clear_color(&self.theme))),
                ) {
                    self.gpu_stash = Some(gpu);
                    self.using_gpu = false;
                }
            }
        }
        self.state.status_msg = format!("Backend {}", self.backend.name());
        self.scene.mark_full_dirty(w as f32, h as f32);
        self.needs_paint = true;
    }

    fn present_frame(&mut self, ctx: &mut WindowCtx<'_>) {
        match &mut self.backend {
            Backend::Gpu(g) => {
                if g.present_path() == GpuPresentPath::NativeSurface {
                    return;
                }
                if let Some((pixels, w, h)) = g.frame_pixels() {
                    let _ = ctx.present(w, h, pixels);
                }
            }
            Backend::Soft(s) => {
                let Some((pixels, w, h)) = s.frame_pixels() else {
                    return;
                };
                let pixels = pixels.to_vec();
                match self.window_backend {
                    PresentBackend::Softbuffer => {
                        let _ = ctx.present(w, h, &pixels);
                    }
                    PresentBackend::GpuSurface => {
                        if let Some(gpu) = self.surface_gpu.as_mut() {
                            let _ = gpu.present_xrgb_frame(w, h, &pixels);
                        }
                    }
                }
            }
        }
    }

    fn advance_pointer_repeat(&mut self) -> bool {
        if !self.ui.repeat_pointer() { return false; }
        self.snapshot_inputs();
        self.ui.sync(&mut self.scene);
        self.needs_paint = true;
        true
    }

    fn after_ui(&mut self, ctx: &mut WindowCtx<'_>) {
        if self.state.plot_work.is_some() {self.handle_plot_load_dismiss();} else if self.state.export_dialog.is_some() {self.handle_data_export_dismiss();} else if self.state.overlay==Some(OverlayKind::ExportSql) {self.handle_sql_export_dismiss();} else if matches!(self.state.overlay, Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv)) {self.handle_csv_confirmation();} else {self.handle_column_picker_dismiss();}
        self.handle_column_list_activate();
        self.handle_field_check_change();
        if let Some((id, color)) = self.ui.take_color_change() {
            if id == self.ids.plot.color && self.state.plot.set_color(color) { self.rebuild = true; }
        }
        self.handle_table_cell_edit();
        while let Some(id) = self.ui.take_button_activate() {
            self.snapshot_inputs();
            if id.0 != 0 && id == self.ids.plot.copy { self.copy_plot(ctx); continue; }
            self.handle_button(id);
        }
        while let Some((_owner, item)) = self.ui.take_menu_action() {
            self.snapshot_inputs();
            if item==mid::PLOT_COPY {
                self.copy_plot(ctx);
            } else if item == mid::BROWSE_COPY_COLUMN {
                if let Some(name) = self.browse_header_name() {
                    if let Err(error) = ctx.clipboard_set_text(name) { self.set_error(error.to_string()); }
                }
                self.browse_header_column = None;
            } else if !self.handle_plot_menu_action(item) {
                self.handle_menu(item);
            }
        }
        if !self.ui.has_open_popup() { self.browse_header_column = None; }
        self.handle_modal();
        if !self.rebuild {
            self.snapshot_inputs();
            self.poll_selection_changes();
        }
        if self.state.poll_sql_job() {
            self.rebuild = true;
        }
        if self.rebuild {
            self.do_rebuild();
        } else {
            sql_syntax::apply(&mut self.ui, self.ids.sql_area, &self.theme);
            for area in [self.ids.ov_idx_cols, self.ids.ov_idx_where, self.ids.ov_check, self.ids.ov_foreign_key, self.ids.ov_generated, self.ids.ov_table_preview, self.ids.ov_key_columns, self.ids.ov_check_expr, self.ids.ov_fk_columns, self.ids.ov_fk_clause] {
                sql_syntax::apply(&mut self.ui, area, &self.theme);
            }
            self.update_sql_hint();
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
        }
        ctx.set_ime_allowed(self.ui.text_input_wants_ime());
        if let Some((x, y, w, h)) = self.ui.ime_cursor_area(&self.scene) {
            ctx.set_ime_cursor_area(x as f64, y as f64, w as f64, h as f64);
        }
        if self.exit_requested {
            ctx.request_exit();
        }
        ctx.request_redraw();
    }
}

impl AppCallbacks for Demo {
    fn can_close(&self) -> bool {
        !self.state.dirty
            && self.state.plot_work.is_none()
            && self.state.csv_work.is_none()
            && !self.state.sql_busy
            && !self.state.has_unsaved_drafts()
            && self
                .ui
                .text_area_value(self.ids.sql_area)
                .is_none_or(|text| text == self.state.sql_editor())
            && self
                .ui
                .text_area_value(self.ids.edit_value)
                .is_none_or(|text| text == self.state.edit_value)
            && self
                .ui
                .text_area_value(self.ids.ddl_area)
                .is_none_or(|text| text == self.state.ddl_draft)
    }

    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        self.win_w = w;
        self.win_h = h;
        if self.pending_native {
            self.pending_native = false;
            let window = ctx.winit_window();
            match GpuRenderer::try_new_native(window, w, h, clear_color(&self.theme)) {
                Ok(gpu) => {
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => {
                    eprintln!("e28_db_browser: native Surface failed ({e})");
                    std::process::exit(1);
                }
            }
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) && !self.qa_exit {
            match GpuRenderer::try_new(w, h, clear_color(&self.theme)) {
                Ok(gpu) => {
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e28_db_browser: GPU unavailable ({e}); Soft only"),
            }
        }
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!("next-gui DB Browser — {}", self.backend.name()));
        eprintln!(
            "e28_db_browser ready {}x{} backend={} path={}",
            w,
            h,
            self.backend.name(),
            self.state.path
        );
        ctx.request_redraw();
    }

    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        match event {
            Event::PointerLeft {..} => {
                let repeat = self.ui.cancel_pointer_repeat();
                if self.ui.pointer_left() || repeat { self.after_ui(ctx); }
            }
            Event::FocusChanged {focused: false, ..} => {
                if self.ui.is_dragging() {
                    self.ui.cancel_pointer_capture();
                    self.after_ui(ctx);
                }
            }
            Event::CloseRequested { .. } => {
                if self.state.plot_work.is_some() {
                    self.cancel_plot_load();
                } else if self.state.export_work.is_some() {
                    self.cancel_data_export();
                } else if self.state.csv_work.is_some() {
                    if let Err(error)=self.state.continue_csv_import_work(Some(csv_files::Answer::Cancel)) {self.set_error(error);}
                    self.rebuild=true;
                } else if self.state.sql_busy {
                    self.state.request_sql_stop();
                } else {
                    self.handle_menu(mid::FILE_EXIT);
                }
                self.after_ui(ctx);
            }
            Event::Resized { width, height, .. } => {
                self.qa_stepper_started = false;
                self.snapshot_inputs();
                self.win_w = width;
                self.win_h = height;
                self.backend.resize(width, height);
                if let Some(g) = self.gpu_stash.as_mut() {
                    g.resize(width, height);
                }
                if let Some(g) = self.surface_gpu.as_mut() {
                    g.resize(width, height);
                }
                self.rebuild = true;
                self.do_rebuild();
                ctx.request_redraw();
            }
            Event::PointerButton {
                button: PointerButton::Right,
                pressed: true,
                x, y, ..
            } => {
                if self.open_plot_context_menu(x as f32,y as f32) || self.open_browse_header_menu(x as f32, y as f32) {
                    self.after_ui(ctx);
                }
            }
            Event::PointerButton {
                button: PointerButton::Left,
                pressed: true,
                x,
                y,
                modifiers,
                ..
            } => {
                let xf = x as f32;
                let yf = y as f32;
                if self.ui.pointer_pressed_with_range_selection(&mut self.scene, xf, yf, modifiers.ctrl || modifiers.meta) {
                    self.snapshot_inputs();
                    self.poll_header_sort(modifiers.commandish());
                    self.after_ui(ctx);
                    return;
                }
                if let Some(hit) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.ui.is_button(hit) {
                        self.snapshot_inputs();
                        self.handle_button(hit);
                        self.after_ui(ctx);
                    }
                }
            }
            Event::PointerButton {
                button: PointerButton::Left,
                pressed: false,
                ..
            } => {
                if self.release_column_picker_pointer() {
                    self.needs_paint = true; ctx.request_redraw(); return;
                }
                if self.ui.is_dragging() {
                    self.ui.pointer_released();
                    self.snapshot_inputs();
                    self.poll_selection_changes();
                    self.do_rebuild();
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::PointerMoved { x, y, .. } => {
                let hovered = self.ui.pointer_hover(&self.scene, x as f32, y as f32);
                if self.ui.pointer_moved(x as f32, y as f32) || hovered {
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::PointerScroll {
                x,
                y,
                delta_x,
                delta_y,
                ..
            } => {
                if self.ui.pointer_scroll(
                    &self.scene,
                    x as f32,
                    y as f32,
                    delta_x as f32,
                    delta_y as f32,
                ) {
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let text_clipboard = modifiers.commandish() && matches!(key, KeyCode::Char('c' | 'C' | 'x' | 'X' | 'v' | 'V'));
                if self.ui.active_dialog().is_some() && !text_clipboard {
                    if let Some(key)=map_key(key,modifiers) {self.ui.key_input(key);}
                    self.after_ui(ctx);return;
                }
                if self.ui.popup_owner()==Some(self.ids.context_menu) {
                    if let Some(key) = map_key(key, modifiers) { self.ui.key_input(key); }
                    self.after_ui(ctx);
                    return;
                }
                if self.handle_plot_context_key(key,modifiers) {
                    self.after_ui(ctx);return;
                }
                if self.handle_sql_completion_key(key, modifiers) {
                    self.after_ui(ctx);
                    return;
                }
                if self.handle_browse_navigation_key(key, modifiers) || self.handle_sql_search_key(key, modifiers) || self.handle_sql_comment_key(key, modifiers) {
                    self.after_ui(ctx);
                    return;
                }
                if self.state.tab == MainTab::Browse && self.state.overlay.is_none() && !self.state.confirm_open {
                    if key == KeyCode::Enter && self.ui.focused() == Some(self.ids.record_number) {
                        self.handle_button(self.ids.btn_record_go);
                        self.after_ui(ctx);
                        return;
                    }
                    if key == KeyCode::Enter && self.ui.focused().is_some_and(|id| id == self.ids.browse_filter || id == self.ids.browse_global_filter || self.ids.browse_col_filters.contains(&id)) {
                        self.handle_button(self.ids.btn_browse_go);
                        self.after_ui(ctx);
                        return;
                    }
                    if modifiers.commandish() && modifiers.alt && matches!(key, KeyCode::Char('f' | 'F')) {
                        self.ui.set_focus(Some(self.ids.browse_global_filter));
                        self.after_ui(ctx);
                        return;
                    }
                }
                if modifiers.commandish() && !self.state.confirm_open {
                    match key {
                        KeyCode::Char('s' | 'S') if self.state.overlay.is_none() => {
                            if self.state.tab == MainTab::ExecuteSql {
                                self.action_save_sql(modifiers.shift);
                            } else { self.handle_menu(mid::FILE_SAVE_ALL); }
                            self.after_ui(ctx);
                            return;
                        }
                        KeyCode::Char('w' | 'W') if self.state.overlay.is_none() => {
                            self.request_document_action(if self.state.tab == MainTab::ExecuteSql {
                                DocumentAction::CloseSqlTab(self.state.sql_tab_idx)
                            } else { DocumentAction::CloseDatabase });
                            self.after_ui(ctx);
                            return;
                        }
                        KeyCode::Char('t' | 'T') if self.state.tab == MainTab::ExecuteSql && self.state.overlay.is_none() => {
                            self.action_new_sql_tab();
                            self.after_ui(ctx);
                            return;
                        }
                        KeyCode::Char('f' | 'F') if self.state.tab == MainTab::ExecuteSql && self.state.overlay.is_none() => {
                            self.open_sql_find();
                            self.after_ui(ctx);
                            return;
                        }
                        KeyCode::Enter if self.state.tab == MainTab::ExecuteSql && self.state.overlay.is_none() => {
                            self.execute_sql_selection();
                            self.after_ui(ctx);
                            return;
                        }
                        KeyCode::Char('c' | 'C' | 'x' | 'X') => {
                            if matches!(key, KeyCode::Char('c' | 'C')) && self.ui.focused() == Some(self.ids.chart) {
                                self.copy_plot(ctx); self.after_ui(ctx); return;
                            }
                            if self.ui.focused() == Some(self.ids.browse_table) {
                                let cut = matches!(key, KeyCode::Char('x' | 'X'));
                                let result = self.state.copy_browse_cells(!cut && modifiers.shift, !cut && modifiers.alt)
                                    .and_then(|copy| {
                                        if copy.html.is_empty() { ctx.clipboard_set_text(&copy.text) }
                                        else { ctx.clipboard_set_html(&copy.html, &copy.text) }
                                            .map_err(|e| e.to_string())?;
                                        self.cell_clipboard = Some(copy);
                                        if cut { self.state.cut_browse_cells() } else { Ok(()) }
                                    });
                                if let Err(e) = result { self.set_error(e); }
                                else if cut { self.rebuild = true; }
                                self.after_ui(ctx);
                                return;
                            }
                            if let Some(text) = self.ui.focused_text_for_copy() {
                                if let Err(e) = ctx.clipboard_set_text(&text) {
                                    self.set_error(e.to_string());
                                } else if matches!(key, KeyCode::Char('x' | 'X')) {
                                    self.ui.delete_focused_text_selection();
                                    self.after_ui(ctx);
                                }
                            }
                            if let Some((_, error)) = self.ui.take_table_data_error() { self.set_error(error); self.after_ui(ctx); }
                            return;
                        }
                        KeyCode::Char('v' | 'V') => {
                            match ctx.clipboard_get_text() {
                                Ok(Some(text)) => {
                                    if self.ui.focused() == Some(self.ids.browse_table) {
                                        let cells = match ctx.clipboard_get_html() {
                                            Ok(Some(html)) => self.cell_clipboard.as_ref().and_then(|copy| copy.internal_cells(&html)),
                                            Ok(None) => None,
                                            Err(e) => {
                                                self.set_error(e.to_string());
                                                self.after_ui(ctx);
                                                return;
                                            }
                                        };
                                        let result = if let Some(cells) = cells {
                                            self.state.paste_browse_cells(cells)
                                        } else {
                                            self.state.paste_browse_tsv(&text)
                                        };
                                        if let Err(e) = result { self.set_error(e); }
                                        else { self.rebuild = true; }
                                    } else {
                                        self.ui.paste_into_focused_text(&text);
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => self.set_error(e.to_string()),
                            }
                            self.after_ui(ctx);
                            return;
                        }
                        _ => {}
                    }
                }
                if let Some(ui_key) = map_key(key, modifiers) {
                    if self.input_key_with_sql_completions(ui_key) {
                        self.after_ui(ctx);
                        return;
                    }
                    if matches!(ui_key, UiKey::Escape) {
                        if self.state.plot_work.is_some() {
                            self.cancel_plot_load(); self.do_rebuild(); ctx.request_redraw(); return;
                        }
                        if self.state.export_work.is_some() {
                            self.cancel_data_export(); self.do_rebuild(); ctx.request_redraw(); return;
                        }
                        if self.state.csv_work.is_some() {
                            if let Err(error) = self.state.continue_csv_import_work(Some(csv_files::Answer::Cancel)) {
                                self.set_error(error);
                            }
                            self.rebuild = true;
                            self.do_rebuild();
                            ctx.request_redraw();
                            return;
                        }
                        if self.state.overlay.is_some() {
                            self.snapshot_inputs();
                            if matches!(self.state.overlay,Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv)) {
                                self.state.csv_work=None;self.state.csv_files=None;self.state.csv_preview=None;
                            }
                            self.state.cipher_open_path = None;
                            self.state.cipher_passphrase.clear();
                            self.state.overlay = None;
                            self.rebuild = true;
                            self.do_rebuild();
                            ctx.request_redraw();
                            return;
                        }
                        if self.ui.modal_open(self.ids.modal).unwrap_or(false) {
                            let _ = self.ui.set_modal_open(self.ids.modal, false);
                            self.state.confirm_open = false;
                            self.rebuild = true;
                            self.do_rebuild();
                            ctx.request_redraw();
                            return;
                        }
                        self.handle_menu(mid::FILE_EXIT);
                        self.after_ui(ctx);
                        return;
                    }
                    if matches!(ui_key, UiKey::Space) && self.ui.focused().is_none() {
                        let (w, h) = ctx.size();
                        self.toggle_backend(w, h);
                        ctx.set_title(&format!("next-gui DB Browser — {}", self.backend.name()));
                        ctx.request_redraw();
                    }
                }
            }
            Event::Ime { ime, .. } => {
                match ime {
                    ImeEvent::Preedit { text, cursor } => {
                        self.ui.ime_preedit(text, cursor);
                    }
                    ImeEvent::Commit { text } => {
                        self.ui.ime_commit(text);
                        self.show_sql_completions(false);
                    }
                    ImeEvent::Disabled => {
                        self.ui.ime_clear_preedit();
                    }
                    ImeEvent::Enabled => {}
                }
                self.after_ui(ctx);
            }
            Event::RedrawRequested { .. } => {
                self.needs_paint = true;
            }
            _ => {}
        }
    }

    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab| tab.starts_with("toolbar-")) && !self.qa_stepper_started {
            self.qa_stepper_started = true;
            self.ui.set_focus(Some(if self.state.tab == MainTab::ExecuteSql { self.ids.btn_sql_comment } else { self.ids.btn_theme }));
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab| tab.starts_with("sql-results-end")) && !self.qa_stepper_started {
            self.qa_stepper_started = true;
            self.ui.set_focus(Some(self.ids.sql_table));
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab| tab.starts_with("plot-hover")) {
            if let Some(bounds) = self.scene.iter_paint_order().into_iter()
                .find(|node| self.ui.widget_from_node(node.id) == Some(self.ids.chart)).map(|node| node.bounds) {
                self.ui.set_focus(Some(self.ids.chart));
                self.ui.pointer_moved(bounds.x + bounds.width * 0.63, bounds.y + bounds.height * 0.5);
                self.ui.sync(&mut self.scene);
                self.needs_paint = true;
            }
        }
        let hold_qa = std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("pragmas-hold");
        if hold_qa && !self.qa_stepper_started {
            self.qa_stepper_started = true;
            let index = self.state.pragmas.iter().position(|(key, _)| key == "journal_size_limit").unwrap();
            let input = self.ids.pragma_inputs[index];
            self.ui.set_text_input(input, "9007199254740993");
            let bounds = self.scene.iter_paint_order().into_iter().find(|n| self.ui.widget_from_node(n.id) == Some(input)).unwrap().bounds;
            self.ui.pointer_pressed(&mut self.scene, bounds.x + bounds.width - 10.0, bounds.y + bounds.height * 0.25);
            self.snapshot_inputs();
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
            eprintln!("native held stepper armed input={input:?} focus={:?} delay={:?}", self.ui.focused(), self.ui.pointer_repeat_delay());
        }
        if self.advance_pointer_repeat() && hold_qa {
            let index = self.state.pragmas.iter().position(|(key, _)| key == "journal_size_limit").unwrap();
            eprintln!("native held stepper draft={}", self.state.pragma_drafts[index]);
        }
        if let Some(delay) = self.ui.pointer_repeat_delay() {
            ctx.request_redraw_after(delay);
        }
        let csv_progress_qa = std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab| matches!(tab.as_str(), "csv-progress" | "csv-canceling"));
        if csv_progress_qa {
            let answer = if self.state.csv_work.as_ref().is_some_and(|work| work.pending.is_some()) {
                Some(csv_files::Answer::Yes)
            } else if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("csv-canceling")
                && self.state.csv_work.as_ref().is_some_and(|work| work.progress.rows().0 > 0 && !work.progress.cancelled()) {
                Some(csv_files::Answer::Cancel)
            } else {None};
            if let Some(answer) = answer {
                if let Err(error) = self.state.continue_csv_import_work(Some(answer)) {self.set_error(error);}
                self.rebuild = true;
            }
        }
        if self.state.csv_running() {
            match self.state.poll_csv_import_work() {
                Ok(true)=>{self.clear_error();self.rebuild=true;}
                Ok(false)=>{},
                Err(error)=>{self.set_error(error);self.rebuild=true;}
            }
            if self.rebuild {self.do_rebuild();} else {self.update_csv_progress();}
            if self.state.csv_running() || csv_progress_qa && self.state.csv_work.is_some() {ctx.request_redraw();}
        }
        if self.state.plot_work.is_some() {
            self.poll_plot_load();
            if self.rebuild { self.do_rebuild(); }
            if self.state.plot_work.is_some() { ctx.request_redraw(); }
        }
        if self.state.export_work.is_some() {
            if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("export-canceling")
                && self.state.export_work.as_ref().is_some_and(|work| work.progress.counts().0 > 0 && work.progress.can_cancel()) {
                self.cancel_data_export();
            }
            self.poll_data_export();
            if self.rebuild { self.do_rebuild(); } else { self.update_data_export_progress(); }
            if self.state.export_work.is_some() { ctx.request_redraw(); }
        }
        if self.state.sql_busy {
            self.snapshot_inputs();
            if self.state.poll_sql_job() {
                self.rebuild = true;
                self.do_rebuild();
            } else {
                ctx.request_redraw();
            }
        }
        self.refresh_sql_log();
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref()==Ok("table-key-type")
            && self.ids.ov_constraint_editor.0!=0 && self.ui.focused()==Some(self.ids.ov_constraint_editor)
            && self.ui.select_selected(self.ids.ov_constraint_editor).is_some() && !self.ui.has_open_popup() {
            self.ui.key_input(UiKey::Space);self.ui.sync(&mut self.scene);self.needs_paint=true;
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab|matches!(tab.as_str(),"table-field-type-dropdown"|"table-field-collation-dropdown"|"table-field-strict-dropdown"))
            && self.ids.ov_field_editor.0!=0 && self.ui.focused()==Some(self.ids.ov_field_editor) && !self.ui.has_open_popup() {
            if self.ui.is_select(self.ids.ov_field_editor) {self.ui.key_input(UiKey::Space);} else {self.ui.open_autocomplete_options(self.ids.ov_field_editor);}
            self.ui.sync(&mut self.scene);self.needs_paint=true;
        }
        if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").as_deref() == Ok("header-menu") && !self.ui.has_open_popup() {
            let bounds = self.scene.iter_paint_order().into_iter().find(
                |node| self.ui.widget_from_node(node.id) == Some(self.ids.browse_table)
            ).map(|node| node.bounds);
            if let Some(bounds) = bounds {
                if self.open_browse_header_menu(bounds.x + 30.0, bounds.y + 10.0) {
                    self.ui.key_input(UiKey::End);
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                }
            }
        }
        if !self.needs_paint {
            return;
        }
        let dirty = self.scene.take_dirty();
        if dirty.is_empty() {
            self.needs_paint = false;
            return;
        }
        let cmds = encode_scene(&self.scene, &dirty);
        self.backend.render(&cmds, &dirty);
        self.present_frame(ctx);
        self.needs_paint = false;

        if self.qa_exit && !self.qa_done {
            self.qa_done = true;
            let path = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT")
                .unwrap_or_else(|_| "/tmp/next-gui-db-browser.ppm".into());
            if let Some((pixels, w, h)) = self.backend.frame_pixels() {
                match write_ppm(&path, pixels, w, h) {
                    Ok(()) => eprintln!("NEXT_GUI_DB_BROWSER_QA: wrote {path}"),
                    Err(e) => eprintln!("NEXT_GUI_DB_BROWSER_QA: write failed: {e}"),
                }
            }
            ctx.request_exit();
        }
    }
}

fn main() {
    let use_native =
        std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some() && GpuRenderer::adapter_available();
    let (width,height)=if std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB").is_ok_and(|tab|matches!(tab.as_str(),"plot-image-clipboard-narrow"|"plot-bars-grouped-narrow"|"plot-bars-stacked-narrow"|"docks-narrow"|"plot-hover-narrow"|"toolbar-browse-narrow"|"toolbar-sql-narrow"|"sql-results-end-narrow"|"cell-structure-narrow"|"cell-pragmas-narrow")) {(800,600)} else {(1400,860)};
    let attrs = if use_native {
        WindowAttrs::new("next-gui DB Browser", width, height).with_gpu_surface()
    } else {
        WindowAttrs::new("next-gui DB Browser", width, height)
    };
    let demo = Demo::new(attrs.width, attrs.height, attrs.present_backend);
    #[cfg(target_os = "macos")]
    let result = next_gui_platform::macos::run(attrs, demo);
    #[cfg(not(target_os = "macos"))]
    let result = next_gui_platform::run(attrs, demo);
    if let Err(e) = result {
        eprintln!("failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod interaction_tests {
    #[test]
    fn long_object_confirmation_retains_scroll_action_and_cancel_across_rebuild_and_resize() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                let name = "Quarterly database 日本語 'quoted' identifier ".repeat(80);
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .execute_batch(&format!(
                        "CREATE TABLE {}(v);INSERT INTO {} VALUES(7)",
                        crate::db::quote_ident(&name),
                        crate::db::quote_ident(&name)
                    ))
                    .unwrap();
                demo.state.refresh_schema().unwrap();
                demo.state.select_schema_node(10_000);
                demo.state.prefs.confirm_drops = true;
                demo.handle_menu(ui_build::mid::EDIT_DELETE);
                demo.do_rebuild();
                assert!(demo
                    .ui
                    .accessible_description(demo.ids.modal)
                    .unwrap()
                    .contains(&name));
                let top = demo.ui.modal_state(demo.ids.modal).unwrap();
                capture(&mut demo, theme_name, width, "long-modal-top");
                demo.ui.key_input(UiKey::Tab);
                demo.ui.key_input(UiKey::End);
                demo.ui.sync(&mut demo.scene);
                let bottom = demo.ui.modal_state(demo.ids.modal).unwrap();
                assert_ne!(bottom, top);
                assert!(demo.ui.take_modal_action().is_none());
                demo.do_rebuild();
                assert_eq!(demo.ui.modal_state(demo.ids.modal), Some(bottom.clone()));
                demo.win_w = if width == 800 { 1400 } else { 800 };
                demo.win_h = if height == 600 { 1000 } else { 600 };
                demo.do_rebuild();
                assert_ne!(demo.ui.modal_state(demo.ids.modal), Some(top));
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                demo.ui.key_input(UiKey::End);
                demo.ui.sync(&mut demo.scene);
                capture(&mut demo, theme_name, width, "long-modal-bottom");
                demo.ui.key_input(UiKey::Enter);
                demo.handle_modal();
                demo.do_rebuild();
                assert!(!demo.state.confirm_open);
                assert_eq!(
                    crate::db::count_rows(demo.state.conn.as_ref().unwrap(), &name).unwrap(),
                    1
                );
            }
        }
    }

    use super::*;

    fn remove_selected_field_or_index(demo: &mut Demo) {
        demo.handle_button(demo.ids.ov_btn_del);
        if matches!(demo.state.confirm_kind, ConfirmKind::DeleteColumn(_)) {
            demo.do_rebuild();
            assert!(demo.ui.key_input(UiKey::Tab));
            assert!(demo.ui.key_input(UiKey::Enter));
            demo.handle_modal();
        }
    }

    #[test]
    fn global_table_header_disclosure_reaches_every_page_and_saves_hidden_options_in_small_windows() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height) in [(1400,1000),(800,600)] {
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap(); demo.state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\"").unwrap();
                demo.state.refresh_schema().unwrap(); demo.state.open_create_table_wizard(); demo.do_rebuild();
                assert!(!demo.state.wizard_table_advanced); assert_eq!(demo.ids.ov_strict.0,0);
                let bounds=|demo:&Demo,id|demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
                for page in 0..4 {
                    demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,page); demo.snapshot_inputs(); demo.ui.sync(&mut demo.scene);
                    let name=bounds(&demo,demo.ids.ov_name); let tabs=bounds(&demo,demo.ids.ov_table_tabs);
                    assert!(name.y+name.height<=tabs.y && name.x>=0.0 && name.x+name.width<=width as f32);
                    demo.ui.set_focus(Some(demo.ids.ov_name)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text(&format!("table_{page}")); demo.snapshot_inputs();
                    assert_eq!(demo.state.wizard_table_name,format!("table_{page}"));
                }
                demo.ui.sync(&mut demo.scene);
                let button=bounds(&demo,demo.ids.ov_table_advanced); let hit=demo.ui.hit_widget(&demo.scene,button.x+8.0,button.y+8.0);
                assert_eq!(hit,Some(demo.ids.ov_table_advanced));
                assert!(!demo.ui.pointer_pressed(&mut demo.scene,button.x+8.0,button.y+8.0));
                demo.ui.pointer_released(); demo.snapshot_inputs(); demo.handle_button(hit.unwrap()); demo.do_rebuild();
                assert!(demo.state.wizard_table_advanced); assert_eq!(demo.ui.focused(),Some(demo.ids.ov_table_advanced));
                let schema=demo.state.table_schemas().iter().position(|name|*name=="aux.\"db").unwrap(); demo.ui.set_select_selected(demo.ids.ov_schema,Some(schema));
                demo.ui.set_checked(demo.ids.ov_strict,true); demo.ui.set_checked(demo.ids.ov_without_rowid,true);
                demo.ui.set_select_selected(demo.ids.ov_pk_conflict,Some(3)); demo.snapshot_inputs();
                assert_eq!(demo.state.wizard_schema,"aux.\"db");
                assert!(demo.rebuild); demo.do_rebuild();
                let preview=bounds(&demo,demo.ids.ov_table_preview); let save=bounds(&demo,demo.ids.ov_btn_ok);
                let tabs=bounds(&demo,demo.ids.ov_table_tabs); let option=bounds(&demo,demo.ids.ov_pk_conflict);
                assert!(option.y+option.height<=tabs.y && tabs.y+tabs.height<=preview.y && preview.y+preview.height<=save.y && save.y+save.height<=height as f32,
                    "{width}: option={option:?},tabs={tabs:?},preview={preview:?},save={save:?}");
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_HEADER_QA_OUT") {
                    demo.ui.sync(&mut demo.scene); demo.scene.mark_full_dirty(width as f32,height as f32); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(width,height,clear); soft.render(&commands,&dirty); write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{width}-metal.ppm"),b,width,height).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|((a>>s)&255).abs_diff((b>>s)&255))).max().unwrap(); assert!(delta<=2,"{theme_name}/{width}: {delta}");
                        eprintln!("Table header {theme_name}/{width}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Table header GPU comparison not_run: adapter unavailable"); }
                }
                demo.ui.set_focus(Some(demo.ids.ov_table_advanced)); assert!(demo.ui.key_input(UiKey::Enter)); let button=demo.ui.take_button_activate().unwrap(); demo.snapshot_inputs(); demo.handle_button(button); demo.do_rebuild();
                assert!(!demo.state.wizard_table_advanced); assert_eq!(demo.ids.ov_strict.0,0); assert!(demo.state.wizard_table_options.strict && demo.state.wizard_table_options.without_rowid);
                assert!(demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().contains("STRICT"));
                demo.ui.set_focus(Some(demo.ids.ov_name)); assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_table_name,"table_2");
                assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_table_name,"table_3");
                demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                let conn=demo.state.conn.as_ref().unwrap(); let options=dbmod::table_options_in(conn,"aux.\"db","table_3").unwrap(); assert!(options.strict && options.without_rowid);
                assert!(conn.execute("INSERT INTO \"aux.\"\"db\".table_3 VALUES(1,'kept')",[]).is_ok()); assert!(conn.prepare("SELECT rowid FROM \"aux.\"\"db\".table_3").is_err());
                demo.state.open_create_table_wizard(); assert!(!demo.state.wizard_table_advanced); assert!(demo.state.wizard_table_name_state.is_none());
                demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.ov_name)); assert!(!demo.ui.key_input(UiKey::Undo));
                demo.ui.key_input(UiKey::Char('!')); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
                demo.state.open_create_table_wizard(); demo.do_rebuild(); assert!(!demo.ui.key_input(UiKey::Redo));
            }
        }
    }

    #[test]
    fn single_line_wizard_undo_updates_constraint_identity_preview_and_survives_focused_rebuild() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap(); demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,name TEXT DEFAULT 'kept' CHECK(length(name)>0)); INSERT INTO records VALUES(7,'customer'); CREATE VIEW visible AS SELECT name FROM records;").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap(); demo.state.select_schema_node(10_000+row as u64);
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_col_sel=1; demo.state.wizard_scroll=(0.0,180.0); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_col_name)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text("customer_name"); demo.snapshot_inputs();
            assert_eq!(demo.state.wizard_cols[1].name,"customer_name"); assert!(demo.state.wizard_cols[1].check_expression.contains("customer_name"));
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_cols[1].name,"name");
            assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_col_name)); assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs();
            assert_eq!(demo.state.wizard_cols[1].name,"name");
            demo.ui.key_input(UiKey::SelectAll); demo.ui.ime_preedit("顾",Some((3,3))); demo.snapshot_inputs(); demo.do_rebuild();
            assert!(demo.ui.ime_commit("顾客名")); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_cols[1].name,"顾客名");
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_cols[1].name,"name");
            assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs();
            demo.ui.set_focus(Some(demo.ids.ov_default)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text("'new'"); demo.snapshot_inputs(); demo.do_rebuild();
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_cols[1].default_value,"'kept'");
            assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs();
            demo.ui.set_focus(Some(demo.ids.ov_name)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text("orders"); demo.snapshot_inputs(); demo.do_rebuild();
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); assert_eq!(demo.state.wizard_table_name,"records");
            assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs();
            assert!(demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().contains("顾客名"));
            demo.ui.sync(&mut demo.scene);
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_INPUT_HISTORY_QA_OUT") {
                demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let mut soft=SoftRenderer::new(1400,1000,clear_color(&demo.theme)); soft.render(&commands,&dirty);
                write_ppm(&format!("{prefix}-{theme_name}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear_color(&demo.theme)) {
                    gpu.render(&commands,&dirty); let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|((a>>shift)&255).abs_diff((b>>shift)&255))).max().unwrap_or(0);
                    eprintln!("Input history {theme_name}: full-frame Soft/Metal delta {delta}"); assert!(delta<=2); write_ppm(&format!("{prefix}-{theme_name}-metal.ppm"),gpu.frame_pixels().unwrap().0,1400,1000).unwrap();
                } else { eprintln!("Input history GPU comparison not_run: adapter unavailable"); }
            }
            demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap(); assert_eq!(conn.query_row("SELECT rowid,顾客名 FROM orders",[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?))).unwrap(),(7,"customer".into()));
            assert_eq!(conn.query_row("SELECT * FROM visible",[],|row|row.get::<_,String>(0)).unwrap(),"customer");
            assert!(conn.execute("UPDATE orders SET 顾客名=''",[]).is_err()); conn.execute("INSERT INTO orders(id) VALUES(8)",[]).unwrap();
            assert_eq!(conn.query_row("SELECT 顾客名 FROM orders WHERE id=8",[],|row|row.get::<_,String>(0)).unwrap(),"new");
        }
    }

    #[test]
    fn browse_buttons_follow_page_record_and_write_boundaries() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(
            "CREATE TABLE t(v INTEGER); WITH RECURSIVE n(v) AS \
             (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<501) INSERT INTO t SELECT v FROM n;"
        ).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.refresh_browse().unwrap();
        demo.state.write_changes().unwrap();
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_record_prev), Some(false));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_record_next), Some(true));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_write), Some(false));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_revert), Some(false));
        demo.handle_button(demo.ids.btn_record_prev);
        demo.handle_button(demo.ids.btn_revert);
        assert!(!demo.rebuild);
        assert!(!demo.state.confirm_open);
        demo.handle_button(demo.ids.btn_record_last);
        demo.do_rebuild();
        assert_eq!(demo.state.browse().offset, 500);
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_record_prev), Some(true));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_record_next), Some(false));
        demo.handle_button(demo.ids.btn_record_next);
        assert!(!demo.rebuild);

        demo.state.toggle_browse_view();
        demo.do_rebuild();
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_form_prev), Some(true));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_form_next), Some(false));
        demo.handle_button(demo.ids.btn_form_prev);
        demo.do_rebuild();
        assert_eq!(demo.state.browse().sel_row, Some(499));
        assert_eq!(demo.state.browse().offset, 0);
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_form_next), Some(true));
        demo.state.browse_mut().sel_row = Some(0);
        demo.state.sync_edit_cell_from_selection();
        demo.do_rebuild();
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_form_prev), Some(false));
        demo.handle_button(demo.ids.btn_form_prev);
        assert!(!demo.rebuild);

        demo.state.conn.as_ref().unwrap().execute("UPDATE t SET v=0 WHERE v=1", []).unwrap();
        demo.state.mark_dirty();
        demo.do_rebuild();
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_write), Some(true));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_revert), Some(true));
        demo.handle_button(demo.ids.btn_write);
        demo.do_rebuild();
        assert!(!demo.state.dirty);
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_write), Some(false));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_revert), Some(false));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 1400, 860).unwrap();
        }
        demo.state.browse_mut().filter = "0".into();
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        for id in [demo.ids.btn_record_prev, demo.ids.btn_record_next, demo.ids.btn_form_prev, demo.ids.btn_form_next] {
            assert_eq!(demo.ui.button_enabled(id), Some(false));
        }
    }

    #[test]
    fn write_buttons_are_disabled_when_disconnected_read_only_or_busy() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.dirty = true;
        for (read_only, busy) in [(true, false), (false, true)] {
            demo.state.read_only = read_only;
            demo.state.sql_busy = busy;
            demo.do_rebuild();
            assert_eq!(demo.ui.button_enabled(demo.ids.btn_write), Some(false));
            assert_eq!(demo.ui.button_enabled(demo.ids.btn_revert), Some(false));
            demo.handle_button(demo.ids.btn_write);
            demo.handle_button(demo.ids.btn_revert);
            assert!(demo.state.dirty);
            assert!(!demo.state.confirm_open);
        }
        demo.state.sql_busy = false;
        demo.state.read_only = false;
        demo.state.dirty = false;
        demo.state.close_db();
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        for id in [demo.ids.btn_write, demo.ids.btn_revert, demo.ids.btn_record_prev, demo.ids.btn_record_next] {
            assert_eq!(demo.ui.button_enabled(id), Some(false));
        }
    }

    #[test]
    fn log_dock_keeps_full_read_only_text_selection_filters_and_clear_scope() {
        use crate::sql_log::Source;
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.dock_log = true;
        demo.state.dock_focus = DockKind::Log;
        let sql = format!("SELECT '{}';\nSELECT 2;", "完整日志🙂".repeat(30));
        demo.state.run_sql_text_sync(&sql).unwrap();
        demo.do_rebuild();
        let area = demo.ids.log_area;
        assert!(demo.ui.text_area_value(area).unwrap().contains(&sql));
        demo.ui.set_focus(Some(area));
        demo.ui.select_text_area_range(area, 10..40);
        let selected = demo.ui.focused_text_for_copy().unwrap();
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused_text_for_copy(), Some(selected));
        let value = demo.ui.text_area_value(demo.ids.log_area).unwrap().to_owned();
        demo.ui.key_input(UiKey::Char('x'));
        assert!(!demo.ui.ime_commit("changed"));
        assert_eq!(demo.ui.text_area_value(demo.ids.log_area), Some(value.as_str()));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.ui.sync(&mut demo.scene);
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 1400, 860).unwrap();
        }
        demo.ui.set_select_selected(demo.ids.log_filter, Some(1));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.sql_log_filter, 1);
        assert!(demo.ui.text_area_value(demo.ids.log_area).unwrap().contains("Application"));
        demo.state.sql_log.push(Source::Application, "SELECT 'new trace while running';");
        assert!(demo.refresh_sql_log());
        assert!(demo.ui.text_area_value(demo.ids.log_area).unwrap().contains("new trace while running"));
        assert!(!demo.refresh_sql_log());
        demo.handle_button(demo.ids.log_clear);
        demo.do_rebuild();
        assert!(demo.state.sql_log.snapshot(Some(Source::Application)).0.is_empty());
        assert!(demo.state.sql_log.text(Some(Source::User)).contains(&sql));
        demo.ui.set_select_selected(demo.ids.log_filter, Some(3));
        demo.poll_selection_changes();
        demo.do_rebuild();
        demo.handle_button(demo.ids.log_clear);
        assert!(demo.state.sql_log.snapshot(None).0.is_empty());
        demo.win_w = 800;
        demo.win_h = 600;
        demo.state.sql_log_filter = 0;
        demo.state.sql_log.push(Source::User, &sql);
        demo.do_rebuild();
        let bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.log_area)).unwrap().bounds;
        assert!(bounds.x >= 0.0 && bounds.x + bounds.width <= 800.0 && bounds.y + bounds.height <= 600.0, "{bounds:?}");
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(800.0, 600.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(800, 600, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&PathBuf::from(path).with_extension("narrow.ppm").to_string_lossy(), renderer.frame_pixels().unwrap().0, 800, 600).unwrap();
        }
    }

    #[test]
    fn structure_toolbar_has_no_vertical_overflow_at_normal_or_narrow_width() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        for (width, height) in [(1400, 860), (800, 600)] {
            demo.win_w = width;
            demo.win_h = height;
            demo.state.tab = MainTab::Structure;
            demo.do_rebuild();
            let mut node = demo.scene.iter_paint_order().into_iter()
                .find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.btn_struct_new_table)).unwrap().id;
            let toolbar = loop {
                node = demo.scene.get(node).unwrap().parent.expect("toolbar viewport");
                if let Some(widget) = demo.ui.widget_from_node(node) {
                    if demo.ui.scroll_offset(widget).is_some() { break widget; }
                }
            };
            demo.ui.set_scroll_offset(toolbar, 10000.0, 10000.0);
            assert_eq!(demo.ui.scroll_offset(toolbar).unwrap().1, 0.0, "{width}x{height}");
        }
    }

    #[test]
    fn status_encoding_follows_database_sql_worker_readonly_and_close() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("utf16.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA encoding='UTF-16le'; CREATE TABLE t(v); INSERT INTO t VALUES('中文🙂');").unwrap();
        drop(conn);
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        let status = |demo: &Demo| {
            let built = build_ui(demo.font.clone(), demo.theme, demo.win_w, demo.win_h, &demo.state);
            built.ui.text_content(built.status).unwrap().to_owned()
        };
        demo.state.close_db();
        demo.state.open_readonly_path(path.to_str().unwrap()).unwrap();
        assert!(status(&demo).contains("UTF-16le"), "{}", status(&demo));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            let mut built = build_ui(demo.font.clone(), demo.theme, demo.win_w, demo.win_h, &demo.state);
            built.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = built.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&built.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
        demo.state.run_sql_text_sync("SELECT * FROM t").unwrap();
        assert!(status(&demo).contains("UTF-16le"));
        demo.state.close_db();
        demo.state.new_memory_db().unwrap();
        demo.state.run_sql_text_sync("PRAGMA encoding='UTF-16be'; CREATE TABLE t(v)").unwrap();
        assert!(status(&demo).contains("UTF-16be"));
        demo.state.start_sql_text("SELECT 1".into()).unwrap();
        assert!(status(&demo).contains("UTF-16be"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !demo.state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline, "SQL worker deadline");
            std::thread::yield_now();
        }
        demo.state.close_db();
        assert!(!status(&demo).contains("UTF-16"));
        demo.state.new_memory_db().unwrap();
        assert!(status(&demo).contains("UTF-8"));
    }

    #[test]
    fn normal_sql_page_fits_with_visible_or_hidden_toolbar() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        for visible in [true, false] {
            demo.state.toolbar_sql = visible;
            demo.do_rebuild();
            demo.ui.set_scroll_offset(demo.ids.page_scroll, 0.0, 10000.0);
            assert_eq!(demo.ui.scroll_offset(demo.ids.page_scroll), Some((0.0, 0.0)));
        }
        for tab in [MainTab::Structure, MainTab::Browse, MainTab::Pragmas, MainTab::ExecuteSql] {
            demo.state.tab = tab;
            demo.do_rebuild();
            demo.ui.set_scroll_offset(demo.ids.shell_scroll, 0.0, 10000.0);
            assert_eq!(demo.ui.scroll_offset(demo.ids.shell_scroll), Some((0.0, 0.0)), "{tab:?}");
        }
    }

    #[test]
    fn browse_shift_selection_copies_selected_rows_as_tsv() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.run_sql_text_sync("CREATE TABLE t(a,b); INSERT INTO t VALUES('one','x'),('two','y'),('three','z');").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.tab = MainTab::Browse;
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.browse_table));
        demo.ui.key_input(UiKey::Down);
        demo.ui.key_input(UiKey::SelectDown);
        assert_eq!(demo.ui.table_selected_rows(demo.ids.browse_table), Some((0, 1)));
        demo.ui.set_table_selected_col(demo.ids.browse_table, Some(0));
        demo.ui.key_input(UiKey::SelectRight);
        assert_eq!(demo.ui.table_selected_cols(demo.ids.browse_table), Some((0, 1)));
        assert_eq!(demo.ui.table_selected_text(demo.ids.browse_table).as_deref(), Some("one\tx\ntwo\ty"));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.table_selected_rows(demo.ids.browse_table), Some((0, 1)));
        assert_eq!(demo.ui.table_selected_cols(demo.ids.browse_table), Some((0, 1)));
    }

    #[test]
    fn browse_drag_commits_selection_after_release_and_keeps_the_editor_value() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.run_sql_text_sync("CREATE TABLE t(a,b,c); INSERT INTO t VALUES('a','b','c'),('d','e','f'),('g','h','i');").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.tab = MainTab::Browse;
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        let bounds = demo.scene.iter_paint_order().into_iter()
            .find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.browse_table)).unwrap().bounds;
        let x = bounds.x + 10.0;
        let y = bounds.y + demo.ui.table_style().header_height + 5.0;
        assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
        assert!(demo.ui.is_dragging());
        demo.poll_selection_changes();
        assert_eq!(demo.state.browse().sel_row, None);
        assert!(!demo.rebuild);
        let widths = demo.ui.table_column_widths(demo.ids.browse_table).unwrap();
        assert!(demo.ui.pointer_moved(x + widths[0] + widths[1], y + demo.ui.table_style().row_height * 2.0));
        demo.ui.sync(&mut demo.scene);
        demo.ui.pointer_released();
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.table_selected_rows(demo.ids.browse_table), Some((0, 2)));
        assert_eq!(demo.ui.table_selected_cols(demo.ids.browse_table), Some((0, 2)));
        assert_eq!(demo.state.edit_value, "i");
        assert!(!demo.state.has_cell_draft());
        assert_eq!(demo.ui.focused(), Some(demo.ids.browse_table));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
    }

    #[test]
    fn browse_tab_buttons_restore_directional_selection_and_reject_busy_switches() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.run_sql_text_sync("CREATE TABLE t(a,b,c); INSERT INTO t VALUES(0,1,2),(3,4,5),(6,7,8),(9,10,11);").unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.tab = MainTab::Browse;
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        demo.ui.set_table_selected_cells(demo.ids.browse_table, Some((3, 3)), Some((2, 2)));
        demo.ui.set_focus(Some(demo.ids.browse_table));
        for key in [UiKey::SelectUp, UiKey::SelectUp, UiKey::SelectLeft, UiKey::SelectLeft] {
            assert!(demo.ui.key_input(key));
        }
        demo.poll_selection_changes();
        demo.do_rebuild();
        demo.handle_button(demo.ids.btn_browse_new_tab);
        demo.do_rebuild();
        demo.ui.set_table_selected_cells(demo.ids.browse_table, Some((2, 2)), Some((1, 1)));
        demo.poll_selection_changes();
        demo.do_rebuild();
        demo.handle_button(demo.ids.browse_tab_btns[0]);
        demo.do_rebuild();
        assert_eq!(demo.ui.table_selected_rows(demo.ids.browse_table), Some((1, 3)));
        assert_eq!(demo.ui.table_selected_cols(demo.ids.browse_table), Some((0, 2)));
        assert_eq!(demo.ui.table_selection_anchor(demo.ids.browse_table), Some((3, Some(2))));
        assert_eq!(demo.state.edit_col, "a");
        assert_eq!(demo.state.edit_value, "3");
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
        demo.ui.set_focus(Some(demo.ids.browse_table));
        assert!(demo.ui.key_input(UiKey::SelectDown));
        demo.poll_selection_changes();
        assert_eq!(demo.ui.table_selected_rows(demo.ids.browse_table), Some((2, 3)));
        demo.do_rebuild();
        demo.handle_button(demo.ids.btn_browse_close_tab);
        demo.do_rebuild();
        assert_eq!(demo.state.browse_tabs.len(), 1);
        assert_eq!(demo.ui.table_selected_row(demo.ids.browse_table), Some(2));
        assert_eq!(demo.state.edit_value, "7");
        demo.state.start_sql_text("SELECT 1".into()).unwrap();
        demo.do_rebuild();
        for id in [demo.ids.btn_browse_new_tab, demo.ids.btn_browse_close_tab, demo.ids.browse_tab_btns[0]] {
            assert_eq!(demo.ui.button_enabled(id), Some(false));
            demo.handle_button(id);
        }
        assert_eq!(demo.state.browse_tabs.len(), 1);
        assert_eq!(demo.state.browse().sel_row, Some(2));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !demo.state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        demo.do_rebuild();
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_browse_new_tab), Some(true));
    }

    #[test]
    fn backwards_browse_selection_keeps_anchor_through_every_rebuild() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.run_sql_text_sync("CREATE TABLE t(a,b,c,d); INSERT INTO t VALUES(0,1,2,3),(4,5,6,7),(8,9,10,11),(12,13,14,15);").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.tab = MainTab::Browse;
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        demo.ui.set_table_selected_cells(demo.ids.browse_table, Some((3, 3)), Some((3, 3)));
        demo.ui.set_focus(Some(demo.ids.browse_table));
        for key in [UiKey::SelectUp, UiKey::SelectUp, UiKey::SelectLeft, UiKey::SelectLeft] {
            assert!(demo.ui.key_input(key));
            demo.poll_selection_changes();
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.browse_table));
            assert_eq!(demo.ui.table_selection_anchor(demo.ids.browse_table), Some((3, Some(3))));
        }
        assert_eq!(demo.ui.table_selected_rows(demo.ids.browse_table), Some((1, 3)));
        assert_eq!(demo.ui.table_selected_cols(demo.ids.browse_table), Some((1, 3)));
        assert_eq!(demo.ui.table_selected_row(demo.ids.browse_table), Some(1));
        assert_eq!(demo.ui.table_selected_col(demo.ids.browse_table), Some(1));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
        demo.state.refresh_browse().unwrap();
        assert!(demo.state.browse().sel_row_start.is_none());
        assert!(demo.state.browse().sel_col_start.is_none());
        demo.do_rebuild();
        assert_eq!(demo.ui.table_selected_text(demo.ids.browse_table), None);
    }

    #[test]
    fn browse_filter_actions_apply_all_inputs_clear_together_and_preserve_sort() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE t(n INTEGER, name TEXT);
            INSERT INTO t VALUES(1, 'alpha'), (2, 'alpha'), (3, 'beta');").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_tabs = vec![state::BrowseTabState::new(Some("t".into()))];
        demo.state.browse_mut().order_col = Some("n".into());
        demo.state.browse_mut().order_desc = true;
        demo.state.tab = MainTab::Browse;
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.browse_global_filter, " alpha   =2 ");
        demo.ui.set_text_input(demo.ids.browse_col_filters[0], "1~3");
        demo.ui.set_text_input(demo.ids.browse_filter, "n <> 3");
        demo.handle_button(demo.ids.btn_browse_go);
        assert_eq!(demo.state.browse().global_filters, ["alpha", "=2"]);
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows.len(), 1);
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][0], "2");
        assert_eq!(demo.state.browse().order_col.as_deref(), Some("n"));
        assert!(demo.state.browse().order_desc);
        demo.do_rebuild();
        demo.handle_button(demo.ids.btn_browse_clear_filters);
        assert!(demo.state.browse().filter.is_empty());
        assert!(demo.state.browse().global_filters.is_empty());
        assert!(demo.state.browse().col_filters.iter().all(String::is_empty));
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows.len(), 3);
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][0], "3");
        assert!(db::has_pending_changes(demo.state.conn.as_ref().unwrap()).unwrap());
    }

    #[test]
    fn schema_tree_navigation_retains_focus_and_viewport_after_rebuild() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        for i in 0..100 {
            demo.state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE t{i:03}(v);")).unwrap();
        }
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::Structure;
        demo.do_rebuild();
        demo.ui.set_tree_selected(demo.ids.tree, Some(10_080));
        demo.ui.set_focus(Some(demo.ids.tree));
        demo.ui.set_tree_scroll_offset(demo.ids.tree, 1800.0);
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.tree));
        assert!(demo.ui.tree_scroll_offset(demo.ids.tree).unwrap() > 0.0);
        assert_eq!(demo.ui.tree_selected(demo.ids.tree), Some(10_080));
        assert!(demo.ui.key_input(UiKey::Down));
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.selected.as_ref().unwrap().name, "t081");
        assert_eq!(demo.ui.focused(), Some(demo.ids.tree));
        let scroll = demo.ui.tree_scroll_offset(demo.ids.tree).unwrap();
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.tree_scroll_offset(demo.ids.tree), Some(scroll));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_TREE_QA_OUT") {
            demo.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
        demo.ui.set_tree_expanded(demo.ids.tree, 2, false);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.tree_expanded(demo.ids.tree, 2), Some(false));
        assert_eq!(demo.ui.tree_scroll_offset(demo.ids.tree), Some(0.0));
        assert!(demo.ui.key_input(UiKey::Home));
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.tree_selected(demo.ids.tree), Some(1));
        assert!(demo.state.selected.is_none());
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        demo.state.tab = MainTab::Structure;
        demo.do_rebuild();
        assert_eq!(demo.ui.tree_selected(demo.ids.tree), Some(1));
        assert_eq!(demo.ui.tree_expanded(demo.ids.tree, 2), Some(false));
        demo.state.conn.as_ref().unwrap().execute_batch("COMMIT; ATTACH ':memory:' AS scratch;
            ATTACH ':memory:' AS kept; CREATE TABLE kept.kept_table(v);").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.do_rebuild();
        let old_root = demo.state.schema_names.iter().position(|name| name == "kept").unwrap() as u64 * 5 + 1;
        demo.ui.set_tree_expanded(demo.ids.tree, old_root, false);
        demo.snapshot_inputs();
        demo.state.conn.as_ref().unwrap().execute_batch("DETACH scratch;").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.do_rebuild();
        let new_root = demo.state.schema_names.iter().position(|name| name == "kept").unwrap() as u64 * 5 + 1;
        assert_ne!(old_root, new_root);
        assert_eq!(demo.ui.tree_expanded(demo.ids.tree, new_root), Some(false));
    }

    #[test]
    fn imported_plot_styles_survive_selection_polling_and_rebuild() {
        use next_gui_ui::{PlotLine, PlotMarker};
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
        demo.state.plot.settings = project_xml::decode(NATIVE_PLOT_QA_XML).unwrap().plot_settings;
        demo.state.tab = MainTab::Browse;
        demo.state.dock_focus = DockKind::Plot;
        demo.state.dock_plot = true;
        demo.do_rebuild();
        assert_eq!(demo.ui.select_selected(demo.ids.plot.line), Some(2));
        assert_eq!(demo.ui.select_selected(demo.ids.plot.marker), Some(4));
        let settings = demo.state.plot.settings.clone();
        let points = demo.state.plot.data.series[0].points.clone();
        demo.poll_selection_changes();
        demo.do_rebuild();
        demo.ui.set_table_selected_row(demo.ids.plot.columns, Some(3));
        demo.ui.set_table_selected_col(demo.ids.plot.columns, Some(3));
        demo.poll_selection_changes();
        demo.do_rebuild();
        demo.poll_selection_changes();
        assert_eq!(demo.state.plot.settings, settings);
        assert!(std::sync::Arc::ptr_eq(&points, &demo.state.plot.data.series[0].points));
        assert_eq!(demo.ui.select_selected(demo.ids.plot.line), Some(5));
        assert_eq!(demo.ui.select_selected(demo.ids.plot.marker), Some(0));
        assert!(demo.state.plot.check(0, 2, true));
        demo.do_rebuild();
        assert_eq!(demo.state.plot.data.series[2].line, PlotLine::Impulse);
        demo.ui.set_select_selected(demo.ids.plot.line, Some(3));
        demo.ui.set_select_selected(demo.ids.plot.marker, Some(1));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert!(demo.state.plot.data.series.iter().all(|s| s.line == PlotLine::StepRight && s.marker == PlotMarker::Cross));
    }

    #[test]
    fn current_plot_controls_link_rows_preserve_drafts_and_follow_browse_and_sql_results() {
        use next_gui_ui::{PlotLine, PlotMarker};
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
        demo.state.browse_mut().filter = "expenses >= 700".into();
        demo.state.refresh_browse().unwrap();
        demo.state.sort_browse_column("observed".into(), false).unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.body_ratio = 0.52;
        demo.state.dock_focus = DockKind::Plot;
        demo.state.dock_plot = true;
        demo.do_rebuild();
        assert_eq!(demo.ui.table_cell_checked(demo.ids.plot.columns, 4, 2), None);
        for (row, column) in [(1, 1), (2, 2), (3, 3)] {
            demo.ui.set_table_selected_row(demo.ids.plot.columns, Some(row));
            demo.ui.set_table_selected_col(demo.ids.plot.columns, Some(column));
            demo.ui.set_focus(Some(demo.ids.plot.columns));
            assert!(demo.ui.key_input(UiKey::Space));
            demo.handle_field_check_change();
            demo.poll_selection_changes();
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.plot.columns));
            assert_eq!(demo.ui.table_cell_checked(demo.ids.plot.columns, row, column), Some(true));
        }
        assert_eq!(demo.state.plot.data.series.len(), 2);
        assert_eq!(demo.state.plot.data.series[0].points.len(), demo.state.browse().result.as_ref().unwrap().rows.len());
        assert_eq!(demo.state.plot.data.x_format, next_gui_ui::PlotAxisFormat::Date);
        assert!(demo.ids.plot.color.0 != 0);
        demo.ui.set_select_selected(demo.ids.plot.line, Some(4));
        demo.ui.set_select_selected(demo.ids.plot.marker, Some(4));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.plot.data.series[0].line, PlotLine::StepCenter);
        assert_eq!(demo.state.plot.data.series[0].marker, PlotMarker::Disc);
        let points = demo.state.plot.data.series[0].points.clone();
        demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().zoom(0.5, 0.5, 0.5);
        demo.snapshot_inputs();
        let view = demo.state.plot.data.view();
        demo.do_rebuild();
        assert!(std::sync::Arc::ptr_eq(&points, &demo.state.plot.data.series[0].points));
        assert_eq!(demo.ui.chart_plot(demo.ids.chart, 0).unwrap().view(), view);
        demo.ui.set_focus(Some(demo.ids.plot.legend));
        assert!(demo.ui.key_input(UiKey::Space));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.plot.legend));
        assert!(demo.ui.chart_plot(demo.ids.chart, 0).unwrap().show_legend);
        assert_eq!(demo.ui.chart_plot(demo.ids.chart, 0).unwrap().view(), view);
        assert!(std::sync::Arc::ptr_eq(&points, &demo.state.plot.data.series[0].points));
        assert!(demo.ui.key_input(UiKey::Space));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert!(!demo.ui.chart_plot(demo.ids.chart, 0).unwrap().show_legend);
        demo.handle_button(demo.ids.plot.fit);
        demo.do_rebuild();
        assert!(demo.ui.chart_plot(demo.ids.chart, 0).unwrap().state.view.is_none());
        assert!(demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().select_row(2, false));
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        assert_eq!(demo.state.browse().sel_row, Some(2));
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.edit_value, "unsaved cell");
        demo.snapshot_inputs();
        assert!(demo.state.has_cell_draft());
        assert!(demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().select_row(5, false));
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        assert_eq!(demo.state.browse().sel_row, Some(2));
        assert!(demo.state.error_msg.as_deref().unwrap().contains("cell draft"));
        assert_eq!(*demo.state.plot.data.state.selected_rows, std::collections::BTreeSet::from([2]));
        demo.state.discard_cell_draft();
        demo.state.error_msg = None;
        demo.do_rebuild();
        let dir = tempfile::tempdir().unwrap();
        let settings = demo.state.plot.settings.clone();
        for suffix in ["json", "sqbpro"] {
            let path = dir.path().join(format!("plot.{suffix}"));
            project::ProjectFile::from_state(&demo.state).save(&path).unwrap();
            assert_eq!(project::ProjectFile::load(&path).unwrap().plot_settings, settings);
        }
        for source in ["browse", "sql"] {
            if source == "sql" {
                *demo.state.sql_editor_mut() = "SELECT observed, expenses, revenue\nFROM quarterly_revenue\nORDER BY observed DESC;".into();
                demo.state.sql_result = Some(db::execute_sql(demo.state.conn.as_ref().unwrap(), "SELECT observed, expenses, revenue FROM quarterly_revenue ORDER BY observed DESC").unwrap());
                demo.state.tab = MainTab::ExecuteSql;
                demo.do_rebuild();
                assert!(demo.state.plot.data.series.is_empty());
                for (row, axis) in [(1, 1), (2, 2), (3, 3)] { assert!(demo.state.plot.check(row, axis, true)); }
                demo.do_rebuild();
                assert!(demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().select_row(3, false));
                demo.snapshot_inputs();
                demo.poll_selection_changes();
                assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(3));
            }
            for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                for (width, height) in [(1400, 1000), (800, 600)] {
                    demo.win_w = width;
                    demo.win_h = height;
                    demo.do_rebuild();
                    if let Ok(prefix) = std::env::var("NEXT_GUI_CURRENT_PLOT_QA_OUT") {
                        demo.scene.mark_full_dirty(width as f32, height as f32);
                        let dirty = demo.scene.take_dirty();
                        let commands = encode_scene(&demo.scene, &dirty);
                        let mut soft = SoftRenderer::new(width, height, clear_color(&demo.theme));
                        soft.render(&commands, &dirty);
                        write_ppm(&format!("{prefix}-{source}-{name}-{width}.ppm"), soft.frame_pixels().unwrap().0, width, height).unwrap();
                        if let Ok(mut gpu) = GpuRenderer::try_new(width, height, clear_color(&demo.theme)) {
                            gpu.render(&commands, &dirty);
                            let bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.chart)).unwrap().bounds;
                            let soft_pixels = soft.frame_pixels().unwrap().0;
                            let gpu_pixels = gpu.frame_pixels().unwrap().0;
                            let mut delta = 0;
                            let mut checked = 0;
                            for y in (bounds.y.ceil().max(0.0) as usize + 2)..((bounds.y + bounds.height).floor().min(height as f32) as usize).saturating_sub(2) {
                                for x in (bounds.x.ceil().max(0.0) as usize + 2)..((bounds.x + bounds.width).floor().min(width as f32) as usize).saturating_sub(2) {
                                    for shift in [0, 8, 16] { delta = delta.max((((soft_pixels[y * width as usize + x] >> shift) & 255) as i32 - ((gpu_pixels[y * width as usize + x] >> shift) & 255) as i32).abs()); }
                                    checked += 1;
                                }
                            }
                            assert!(checked > 1000);
                            assert!(delta <= 2, "{source}/{name}/{width}: delta {delta}");
                            eprintln!("Current plot {source}/{name}/{width}: {checked} interior pixels, actual Soft/Metal max channel delta {delta}");
                        } else { eprintln!("Current plot GPU comparison not_run: adapter unavailable"); }
                    }
                }
            }
        }
    }

    #[test]
    fn plot_namespace_selector_reads_the_selected_database_and_reports_errors() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.plot.settings.manual = true;
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(SCHEMA_NAMESPACE_QA_SQL).unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("WITH RECURSIVE n(x) AS (VALUES(2) UNION ALL SELECT x+1 FROM n WHERE x<75)
            INSERT INTO analytics.customers SELECT x,'APAC',100+x*12+((x%3)*30) FROM n;").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("analytics", "customers").unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.dock_plot = true;
        demo.state.dock_focus = DockKind::Plot;
        demo.do_rebuild();
        let index = demo.state.browse_tables().iter().position(|pair| *pair == ("analytics", "customers")).unwrap();
        demo.ui.set_select_selected(demo.ids.chart_select, Some(index));
        demo.ui.set_text_input(demo.ids.chart_col, "revenue");
        demo.handle_button(demo.ids.btn_chart);
        demo.do_rebuild();
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        assert_eq!(demo.state.chart_schema, "analytics");
        assert_eq!(demo.state.chart_table.as_deref(), Some("customers"));
        assert_eq!(demo.state.chart_points.len(), 75);
        assert_eq!(demo.state.chart_points[0].y, 125.5);
        assert!(demo.state.status_msg.contains("\"analytics\".\"customers\""));
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            for (width, height) in [(1400, 1000), (800, 600)] {
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_PLOT_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(width as f32, height as f32);
                    let dirty = demo.scene.take_dirty();
                    let mut soft = SoftRenderer::new(width, height, clear_color(&demo.theme));
                    soft.render(&encode_scene(&demo.scene, &dirty), &dirty);
                    write_ppm(&format!("{prefix}-{name}-{width}.ppm"), soft.frame_pixels().unwrap().0, width, height).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(width, height, clear_color(&demo.theme)) {
                        gpu.render(&encode_scene(&demo.scene, &dirty), &dirty);
                        let bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id).is_some_and(|id| demo.ui.is_chart(id))).unwrap().bounds;
                        let soft_pixels = soft.frame_pixels().unwrap().0;
                        let gpu_pixels = gpu.frame_pixels().unwrap().0;
                        let delta = ((bounds.y.ceil() as usize + 2)..(bounds.y + bounds.height).floor() as usize - 2).flat_map(|y|
                            ((bounds.x.ceil() as usize + 2)..(bounds.x + bounds.width).floor() as usize - 2).map(move |x| y * width as usize + x)
                        ).flat_map(|i| [0, 8, 16].map(|shift|
                            (((soft_pixels[i] >> shift) & 255) as i32 - ((gpu_pixels[i] >> shift) & 255) as i32).abs()
                        )).max().unwrap();
                        assert!(delta <= 2, "Chart {name}/{width} Soft/Metal channel delta {delta}");
                        eprintln!("Chart {name}/{width} Soft/Metal actual readback max channel delta {delta}");
                    } else { eprintln!("Chart {name}/{width} GPU comparison not_run: adapter unavailable"); }
                }
            }
        }
        let main = demo.state.browse_tables().iter().position(|pair| *pair == ("main", "customers")).unwrap();
        demo.ui.set_select_selected(demo.ids.chart_select, Some(main));
        demo.handle_button(demo.ids.btn_chart);
        demo.do_rebuild();
        assert!(demo.state.error_msg.as_deref().unwrap().contains("Unknown plot column"));
        assert!(demo.state.chart_points.is_empty());
        demo.ui.set_text_input(demo.ids.chart_col, "id");
        demo.handle_button(demo.ids.btn_chart);
        assert!(demo.state.error_msg.is_none());
        assert_eq!(demo.state.chart_schema, "main");
        assert_eq!(demo.state.chart_points.iter().map(|p|p.y).collect::<Vec<_>>(), [1.0]);
    }

    #[test]
    fn browse_namespace_selector_shortcuts_and_delete_keep_identity() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(SCHEMA_NAMESPACE_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.switch_browse_table("customers").unwrap();
        demo.do_rebuild();
        let index = demo.state.browse_tables().iter().position(|pair| *pair == ("analytics", "customers")).unwrap();
        demo.ui.set_select_selected(demo.ids.browse_select, Some(index));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.browse().schema, "analytics");
        assert_eq!(demo.state.browse().title, "\"analytics\".\"customers\"");
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][1], "APAC");
        demo.ui.set_focus(Some(demo.ids.browse_table));
        let command = Modifiers { ctrl: true, ..Modifiers::default() };
        assert!(demo.handle_browse_navigation_key(KeyCode::PageDown, command));
        demo.do_rebuild();
        assert_eq!(demo.state.browse().schema, "analytics");
        assert_eq!(demo.state.browse().table.as_deref(), Some("customer_summary"));
        assert!(demo.state.browse().rowids.is_empty());
        demo.ui.set_focus(Some(demo.ids.browse_table));
        assert!(demo.handle_browse_navigation_key(KeyCode::PageUp, command));
        demo.do_rebuild();
        assert_eq!(demo.state.browse().table.as_deref(), Some("customers"));
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            for (width, height) in [(1400, 1000), (800, 600)] {
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_NAMESPACE_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(width as f32, height as f32);
                    let dirty = demo.scene.take_dirty();
                    let mut soft = SoftRenderer::new(width, height, clear_color(&demo.theme));
                    soft.render(&encode_scene(&demo.scene, &dirty), &dirty);
                    write_ppm(&format!("{prefix}-{name}-{width}.ppm"), soft.frame_pixels().unwrap().0, width, height).unwrap();
                }
            }
        }
        demo.state.goto_browse_record(1).unwrap();
        demo.do_rebuild();
        demo.apply_confirmed_action(ConfirmKind::DeleteRow);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        let conn = demo.state.conn.as_ref().unwrap();
        for (schema, expected) in [("main", 1), ("temp", 0), ("analytics", 0)] {
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {}", dbmod::qualified_name(schema, "customers")), [], |r| r.get::<_, i64>(0)).unwrap(), expected);
        }
    }

    #[test]
    fn schema_namespace_selection_restores_identity_and_guards_main_actions() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(SCHEMA_NAMESPACE_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::Structure;
        let main = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "customers").unwrap();
        let aux = demo.state.schema.iter().position(|o| o.schema == "analytics" && o.name == "customers").unwrap();
        demo.state.select_schema_node(10_000 + main as u64);
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][1], "Main customer");
        demo.state.open_insert_row().unwrap();
        demo.state.insert_modes[1] = 2;
        demo.state.insert_values[1] = "Created in main".into();
        demo.state.apply_insert_row().unwrap();
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM main.customers", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM temp.customers", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        demo.do_rebuild();
        demo.ui.set_tree_selected(demo.ids.tree, Some(10_000 + aux as u64));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.tree_selected(demo.ids.tree), Some(10_000 + aux as u64));
        assert_eq!(demo.state.selected.as_ref().unwrap().schema, "analytics");
        assert!(demo.ui.text_area_value(demo.ids.ddl_area).unwrap().contains("revenue"));
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("namespace.sqbpro");
        crate::project::ProjectFile::from_state(&demo.state).save(&project_path).unwrap();
        let restored = crate::project::ProjectFile::load(&project_path).unwrap();
        assert_eq!(restored.selected_object.as_ref().unwrap().schema, "analytics");
        assert_eq!(restored.ddl_draft, demo.state.ddl_draft);
        assert_eq!(demo.state.browse().schema, "analytics");
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][1], "APAC");
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_run_ddl), Some(false));
        assert_eq!(demo.ui.button_enabled(demo.ids.btn_struct_modify), Some(true));
        demo.ui.set_button_enabled(demo.ids.btn_run_ddl, true);
        demo.handle_button(demo.ids.btn_run_ddl);
        assert!(demo.state.error_msg.as_deref().unwrap().contains("qualified"));
        demo.clear_error();
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.dock_focus = DockKind::Schema;
            demo.do_rebuild();
            demo.scene.mark_full_dirty(1400.0, 1000.0);
            let dirty = demo.scene.take_dirty();
            let commands = encode_scene(&demo.scene, &dirty);
            let clear = clear_color(&demo.theme);
            let mut soft = SoftRenderer::new(1400, 1000, clear);
            soft.render(&commands, &dirty);
            if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_SCHEMA_QA_OUT") {
                write_ppm(&format!("{prefix}-{name}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
            }
            if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                gpu.render(&commands, &dirty);
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_SCHEMA_QA_OUT") {
                    write_ppm(&format!("{prefix}-{name}-metal.ppm"), gpu.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                }
                let soft_pixels = soft.frame_pixels().unwrap().0;
                let gpu_pixels = gpu.frame_pixels().unwrap().0;
                let regions: Vec<_> = demo.scene.iter_paint_order().into_iter().filter(|n|
                    demo.ui.widget_from_node(n.id).is_some_and(|id| [demo.ids.tree, demo.ids.ddl_area].contains(&id))
                ).map(|n| n.bounds).collect();
                assert_eq!(regions.len(), 2);
                let max_delta = regions.iter().flat_map(|r|
                    ((r.y.ceil() as usize + 2)..(r.y + r.height).floor() as usize - 2).flat_map(move |y|
                        ((r.x.ceil() as usize + 2)..(r.x + r.width).floor() as usize - 2).map(move |x| y * 1400 + x)
                    )
                ).flat_map(|i| [0, 8, 16].map(|shift|
                    (((soft_pixels[i] >> shift) & 255) as i32 - ((gpu_pixels[i] >> shift) & 255) as i32).abs()
                )).max().unwrap();
                assert!(max_delta <= 2, "schema {name} tree/DDL channel delta {max_delta}");
                eprintln!("schema {name} tree/DDL Soft/Metal actual readback max channel delta {max_delta}");
            } else { eprintln!("schema {name} GPU comparison not_run: adapter unavailable"); }
        }
        demo.state.prefs.confirm_drops = true;
        demo.handle_menu(mid::EDIT_DELETE);
        assert!(demo.state.confirm_open);
        demo.state.confirm_open = false;
        demo.apply_confirmed_action(ConfirmKind::DropObject);
        assert!(!demo.state.schema.iter().any(|o| o.schema == "analytics" && o.name == "customers"));
        assert!(demo.state.schema.iter().any(|o| o.schema == "main" && o.name == "customers"));
        assert!(demo.state.schema.iter().any(|o| o.schema == "temp" && o.name == "customers"));
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT name FROM main.customers", [], |r| r.get::<_, String>(0)).unwrap(), "Main customer");
        demo.do_rebuild();
        demo.ui.set_tree_selected(demo.ids.tree, Some(1));
        demo.poll_selection_changes();
        assert!(demo.state.selected.is_none());
    }

    #[test]
    fn schema_selection_keeps_edited_ddl_until_a_decision() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE a(v); CREATE TABLE b(v);").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::Structure;
        demo.state.select_schema_node(10_000);
        demo.do_rebuild();
        let draft = "CREATE VIEW keep_this AS SELECT 42;";
        demo.ui.set_text_area_value(demo.ids.ddl_area, draft);
        demo.snapshot_inputs();
        demo.ui.set_tree_selected(demo.ids.tree, Some(10_001));
        demo.poll_selection_changes();
        assert_eq!(demo.state.ddl_draft, draft);
        assert_eq!(demo.state.selected.as_ref().unwrap().name, "a");
        assert!(demo.state.confirm_open);
        assert_eq!(demo.state.pending_document, Some(DocumentAction::SelectSchemaNode(10_001)));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.do_rebuild();
            demo.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
        demo.resolve_document_action(ModalAction::Tertiary);
        demo.do_rebuild();
        assert_eq!(demo.ui.tree_selected(demo.ids.tree), Some(10_000));
        assert_eq!(demo.state.ddl_draft, draft);
        demo.state.sql_tabs[0] = "SELECT 'other draft';".into();
        demo.state.run_sql_text_sync("INSERT INTO a VALUES (99)").unwrap();
        demo.request_document_action(DocumentAction::SelectSchemaNode(10_001));
        demo.resolve_document_action(ModalAction::Primary);
        assert_eq!(demo.state.selected.as_ref().unwrap().name, "b");
        assert_eq!(demo.state.ddl_draft, "CREATE TABLE b(v)");
        assert_eq!(demo.state.sql_tabs.last().unwrap(), draft);
        assert_eq!(demo.state.sql_tabs[0], "SELECT 'other draft';");
        assert!(demo.state.sql_tab_dirty(demo.state.sql_tabs.len() - 1));
        assert!(demo.state.dirty);
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT v FROM a", [], |r| r.get::<_, i64>(0)).unwrap(), 99);
        let project = crate::project::ProjectFile::from_state(&demo.state);
        assert_eq!(project.sql_tabs.last().unwrap(), draft);
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.ddl_area, "another edited DDL");
        demo.request_document_action(DocumentAction::SelectSchemaNode(10_000));
        demo.resolve_document_action(ModalAction::Secondary);
        assert_eq!(demo.state.selected.as_ref().unwrap().name, "a");
        assert_eq!(demo.state.ddl_draft, "CREATE TABLE a(v)");
        assert_eq!(demo.state.sql_tabs.len(), 2);
        assert!(demo.state.dirty);
        demo.do_rebuild();
        demo.request_document_action(DocumentAction::SelectSchemaNode(10_001));
        assert!(!demo.state.confirm_open);
        assert_eq!(demo.state.selected.as_ref().unwrap().name, "b");
    }

    #[test]
    fn document_transitions_cancel_keep_drafts_and_discard_rolls_back_before_close() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.sql_area, "SELECT 'keep me';");
        for action in [DocumentAction::NewMemory, DocumentAction::CloseDatabase, DocumentAction::CloseProject, DocumentAction::Exit] {
            demo.request_document_action(action.clone());
            assert_eq!(demo.state.pending_document, Some(action));
            assert!(demo.state.confirm_open);
            demo.resolve_document_action(ModalAction::Tertiary);
            assert!(demo.state.conn.is_some());
            assert_eq!(demo.state.sql_editor(), "SELECT 'keep me';");
            assert!(!demo.exit_requested);
        }
        demo.state.run_sql_text_sync("CREATE TABLE temporary_draft(v)").unwrap();
        demo.request_document_action(DocumentAction::CloseDatabase);
        demo.resolve_document_action(ModalAction::Secondary);
        assert!(demo.state.conn.is_none());
        assert!(!demo.state.dirty);
        assert!(!demo.state.has_unsaved_drafts());
        assert!(!demo.state.sql_editor().contains("keep me"));
    }

    #[test]
    fn closing_sql_tab_preserves_other_drafts_and_pending_database_edits() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.sql_tabs = vec!["SELECT 1".into(), "SELECT 2".into()];
        demo.state.sql_tab_idx = 0;
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.checkpoint_project();
        demo.state.run_sql_text_sync("CREATE TABLE keep_pending(value)").unwrap();
        demo.state.sql_tabs[1] = "SELECT 'keep other draft'".into();
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.sql_area, "SELECT 'close this draft'");
        demo.request_document_action(DocumentAction::CloseSqlTab(0));
        assert!(demo.state.confirm_open);
        demo.resolve_document_action(ModalAction::Tertiary);
        assert_eq!(demo.state.sql_tabs.len(), 2);
        assert!(demo.state.dirty);
        demo.request_document_action(DocumentAction::CloseSqlTab(0));
        demo.resolve_document_action(ModalAction::Secondary);
        assert_eq!(demo.state.sql_tabs, ["SELECT 'keep other draft'"]);
        assert!(demo.state.dirty);
        assert!(db::has_pending_changes(demo.state.conn.as_ref().unwrap()).unwrap());
        assert!(demo.state.conn.as_ref().unwrap().prepare("SELECT * FROM keep_pending").is_ok());

        let dir = tempfile::tempdir().unwrap();
        let sql = dir.path().join("query.sql");
        demo.state.save_sql_file(0, &sql).unwrap();
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.sql_area, "SELECT 'saved on close'");
        demo.request_document_action(DocumentAction::CloseSqlTab(0));
        demo.resolve_document_action(ModalAction::Primary);
        assert_eq!(std::fs::read_to_string(sql).unwrap(), "SELECT 'saved on close'");
        assert!(demo.state.dirty);
        assert!(!demo.state.sql_tab_dirty(0));
        demo.do_rebuild();
        demo.request_document_action(DocumentAction::CloseSqlTab(0));
        assert!(!demo.state.confirm_open, "A clean SQL tab does not prompt for unrelated database changes");
        assert!(demo.state.dirty);
    }

    #[test]
    fn preferences_cancel_keeps_frozen_columns_and_pragma_save_is_visible() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.frozen_cols = 2;
        demo.state.sync_prefs_drafts();
        demo.state.overlay = Some(OverlayKind::Preferences);
        demo.do_rebuild();
        if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_COMPLETION_QA_OUT") {
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let mut soft = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
            soft.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&format!("{prefix}-preferences.ppm"), soft.frame_pixels().unwrap().0, 1400, 860).unwrap();
        }
        demo.ui.set_text_input(demo.ids.ov_frozen, "8");
        let completion = demo.state.prefs.sql_completion;
        let upper_keywords = demo.state.prefs.sql_upper_keywords;
        demo.ui.set_checked(demo.ids.ov_prefs_completion, !completion);
        demo.ui.set_checked(demo.ids.ov_prefs_upper_keywords, !upper_keywords);
        demo.snapshot_inputs();
        assert_eq!(demo.state.prefs_sql_completion, !completion);
        assert_eq!(demo.state.prefs_sql_upper_keywords, !upper_keywords);
        demo.handle_button(demo.ids.ov_btn_cancel);
        assert_eq!(demo.state.frozen_cols, 2);
        assert_eq!(demo.state.prefs.sql_completion, completion);
        assert_eq!(demo.state.prefs.sql_upper_keywords, upper_keywords);
        demo.state.tab = MainTab::Pragmas;
        demo.do_rebuild();
        let b = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.btn_pragma_apply))
            .unwrap()
            .bounds;
        assert!(
            b.y + b.height <= 820.0,
            "Save Pragmas must fit above the status bar: {b:?}"
        );
    }

    #[test]
    fn exit_prompts_for_clean_database_sql_drafts_and_cell_navigation_is_guarded() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.checkpoint_project();
        demo.do_rebuild();
        assert!(demo.can_close());
        demo.ui
            .set_text_area_value(demo.ids.sql_area, "SELECT 'unsaved';");
        assert!(!demo.can_close());
        demo.handle_menu(mid::FILE_EXIT);
        assert!(!demo.exit_requested);
        assert!(demo.state.confirm_open);
        assert_eq!(demo.state.sql_editor(), "SELECT 'unsaved';");
        demo.state.confirm_open = false;
        demo.state.tab = MainTab::Browse;
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES(7)")
            .unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.refresh_browse().unwrap();
        demo.state.browse_mut().sel_row = Some(0);
        demo.state.sync_edit_cell_from_selection();
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.edit_value, "8");
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.poll_header_sort(false);
        assert!(
            !demo.rebuild,
            "Typing a cell draft must keep the editor session intact"
        );
        demo.handle_button(demo.ids.btn_browse_new_tab);
        assert_eq!(demo.state.browse_tabs.len(), 1);
        assert_eq!(demo.state.edit_value, "8");
        demo.handle_button(demo.ids.btn_edit_discard);
        demo.do_rebuild();
        assert_eq!(demo.state.edit_value, "7");
        assert!(!demo.state.has_cell_draft());
    }

    #[test]
    fn desktop_layout_adapts_to_narrow_window_and_keeps_splitter_preferences() {
        let mut demo = Demo::new(800, 600, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(800.0, 600.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(800, 600, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&PathBuf::from(path).with_extension("browse.ppm").to_string_lossy(), renderer.frame_pixels().unwrap().0, 800, 600).unwrap();
        }
        demo.snapshot_inputs();
        assert_eq!(demo.state.body_ratio, 0.45, "Resizing must not replace the preferred ratio");
        let bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.btn_edit_cell)).unwrap().bounds;
        assert!(bounds.x >= 0.0 && bounds.x + bounds.width < 800.0, "Apply must remain reachable at 800px: {bounds:?}");
        demo.ui.set_splitter_ratio(demo.ids.body_splitter, 0.60);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert!((demo.ui.splitter_ratio(demo.ids.body_splitter).unwrap() - 0.60).abs() < 0.01);
        let project = crate::project::ProjectFile::from_state(&demo.state);
        assert!((project.body_ratio - 0.60).abs() < 0.01);
        demo.state.tab = MainTab::Structure;
        demo.do_rebuild();
        demo.ui.set_splitter_ratio(demo.ids.structure_splitter, 0.45);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert!((demo.ui.splitter_ratio(demo.ids.structure_splitter).unwrap() - 0.45).abs() < 0.01);
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(800.0, 600.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(800, 600, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 800, 600).unwrap();
        }
    }

    #[test]
    fn icon_toolbars_fit_narrow_pages_keep_help_and_retain_actions_and_drafts() {
        fn bounds(demo: &Demo, id: WidgetId) -> next_gui_scene::Rect {
            demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(id)).unwrap().bounds
        }
        fn activate(demo: &mut Demo, id: WidgetId) {
            demo.ui.set_focus(Some(id));
            assert!(demo.ui.key_input(UiKey::Enter));
            assert_eq!(demo.ui.take_button_activate(), Some(id));
            demo.handle_button(id); demo.do_rebuild();
        }
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(CURRENT_PLOT_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("main", "quarterly_revenue").unwrap();
        demo.state.browse_mut().sel_row = Some(0); demo.state.browse_mut().sel_col = Some(1);
        demo.state.sync_edit_cell_from_selection();
        demo.state.dock_focus = DockKind::Plot; demo.state.body_ratio = 0.45;
        demo.state.sql_tabs[0] = "SELECT revenue FROM quarterly_revenue;".into();
        let draft = demo.state.sql_tabs[0].clone(); let cell = demo.state.edit_value.clone();
        let folder = std::env::var("NEXT_GUI_ICON_TOOLBAR_QA_OUT").ok().map(PathBuf::from);
        if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            demo.theme = crate::ui_build::browser_theme(theme, demo.state.prefs.font_size);
            for (width, height) in [(1400, 860), (800, 600)] {
                demo.win_w = width; demo.win_h = height;
                for (page, tab) in [("browse", MainTab::Browse), ("sql", MainTab::ExecuteSql)] {
                    demo.state.tab = tab; demo.do_rebuild();
                    let mut buttons = vec![demo.ids.btn_new, demo.ids.btn_open, demo.ids.btn_write, demo.ids.btn_revert,
                        demo.ids.toolbar_extra, demo.ids.btn_integrity, demo.ids.toolbar_project, demo.ids.btn_project_save, demo.ids.btn_theme];
                    if tab == MainTab::ExecuteSql {
                        buttons.extend([demo.ids.btn_sql_new_tab, demo.ids.btn_sql_open, demo.ids.btn_sql_save, demo.ids.btn_sql_save_as,
                            demo.ids.btn_sql_run, demo.ids.btn_sql_line, demo.ids.btn_sql_stop, demo.ids.btn_sql_find, demo.ids.btn_sql_comment]);
                    }
                    for id in buttons {
                        let rect = bounds(&demo, id);
                        assert_eq!(rect.width, rect.height); assert!(rect.width >= 24.0 && rect.width < 40.0);
                        assert!(rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.width <= width as f32 && rect.y + rect.height <= height as f32, "{page}-{width}: {rect:?}");
                        assert!(demo.ui.accessible_name(id).unwrap().chars().count() > 8);
                        assert_eq!(demo.ui.hit_widget(&demo.scene, rect.x + rect.width * 0.5, rect.y + rect.height * 0.5), Some(id), "Icon cannot be clipped or covered");
                    }
                    assert_eq!(demo.ui.button_enabled(demo.ids.btn_write), Some(false));
                    let help = bounds(&demo, if tab == MainTab::ExecuteSql {demo.ids.btn_sql_comment} else {demo.ids.btn_theme});
                    demo.ui.pointer_moved(help.x + help.width * 0.5, help.y + help.height * 0.5);
                    demo.ui.pointer_hover(&demo.scene, help.x + help.width * 0.5, help.y + help.height * 0.5); demo.ui.sync(&mut demo.scene);
                    let style = demo.ui.tooltip_style();
                    let tip = demo.scene.iter_paint_order().into_iter().rev().find(|n| n.visible && n.fill == style.bg && n.mask.is_none()).unwrap().bounds;
                    assert!(tip.width > help.width && tip.x >= 0.0 && tip.y >= 0.0 && tip.x + tip.width <= width as f32 && tip.y + tip.height <= height as f32);
                    if let Some(folder) = &folder { crate::plot_export::capture(&mut demo, folder, &format!("toolbar-{name}-{page}-{width}")); }
                }
            }
        }
        demo.ui.pointer_left();
        for (menu, absent) in [(mid::VIEW_TB_DB, 0), (mid::VIEW_TB_EXTRA, 1), (mid::VIEW_TB_PROJECT, 2), (mid::VIEW_TB_SQL, 3)] {
            demo.handle_menu(menu); demo.do_rebuild();
            let ids = [demo.ids.btn_new, demo.ids.toolbar_extra, demo.ids.toolbar_project, demo.ids.btn_sql_comment];
            for (index, id) in ids.into_iter().enumerate() { assert_eq!(id == WidgetId(0), index == absent); }
            demo.handle_menu(menu); demo.do_rebuild();
        }
        let old_surface = demo.theme.colors.surface; let button = demo.ids.btn_theme;
        activate(&mut demo, button);
        assert_ne!(demo.theme.colors.surface, old_surface);
        assert_eq!(demo.state.sql_tabs[0], draft); assert!(demo.state.edit_value.is_empty());
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        let button = demo.ids.btn_sql_comment; activate(&mut demo, button);
        assert_eq!(demo.state.sql_editor(), format!("-- {draft}"));
        assert_eq!(demo.ui.focused(), Some(demo.ids.sql_area));
        assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs();
        assert_eq!(demo.state.sql_editor(), draft);
        let button = demo.ids.btn_sql_new_tab; activate(&mut demo, button);
        assert_eq!(demo.state.sql_tabs.len(), 2); assert_eq!(demo.state.sql_tabs[0], draft);
        assert!(demo.state.edit_value.is_empty());
        demo.state.tab = MainTab::Browse; demo.do_rebuild();
        assert_eq!(demo.state.edit_value, cell);
        assert!(!demo.state.dirty);
    }

    #[test]
    fn hidden_docks_reclaim_main_width_and_keep_preferred_splitter_ratio() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::Browse;
        demo.state.body_ratio = 0.63;
        demo.state.dock_edit = false; demo.state.dock_schema = false;
        demo.state.dock_log = false; demo.state.dock_plot = false; demo.state.dock_remote = false;
        demo.do_rebuild();
        demo.snapshot_inputs();
        assert_eq!(demo.ids.body_splitter, WidgetId(0));
        assert_eq!(demo.ids.dock_tabs, WidgetId(0));
        let table = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.browse_table)).unwrap();
        let mut right = table.bounds.x + table.bounds.width;
        let mut parent = table.parent;
        while let Some(id) = parent {
            let node = demo.scene.get(id).unwrap();
            if let Some(clip) = node.clip { right = right.min(clip.x + clip.width); }
            parent = node.parent;
        }
        assert!(right > 1350.0, "Hidden docks must reclaim the clipped table width: {right}");
        assert_eq!(demo.state.body_ratio, 0.63);
        demo.state.dock_edit = true;
        demo.do_rebuild();
        assert!((demo.ui.splitter_ratio(demo.ids.body_splitter).unwrap() - 0.63).abs() < 0.01);
    }

    #[test]
    fn narrow_shell_and_image_view_survive_ui_rebuilds() {
        let mut demo = Demo::new(800, 500, PresentBackend::Softbuffer);
        demo.ui.set_scroll_offset(demo.ids.shell_scroll, 400.0, 300.0);
        demo.snapshot_inputs();
        let scroll = demo.state.shell_scroll;
        assert!(scroll.0 >= 0.0 && scroll.1 >= 0.0);
        demo.do_rebuild();
        assert_eq!(demo.ui.scroll_offset(demo.ids.shell_scroll), Some(scroll));
        demo.state.shell_scroll = (0.0, 0.0);
        demo.win_w = 1400;
        demo.win_h = 860;
        demo.state.edit_image_rgba = Some((400, 300, vec![255; 400 * 300 * 4]));
        demo.state.edit_mode = crate::state::EditCellMode::Image;
        demo.state.dock_focus = DockKind::EditCell;
        demo.do_rebuild();
        demo.handle_button(demo.ids.image_actual);
        demo.do_rebuild();
        assert_eq!(
            demo.ui.image_view(demo.ids.edit_image).unwrap().scale,
            Some(1.0)
        );
        demo.handle_button(demo.ids.image_in);
        demo.do_rebuild();
        assert_eq!(
            demo.ui.image_view(demo.ids.edit_image).unwrap().scale,
            Some(2.0)
        );
        demo.handle_button(demo.ids.image_fit);
        demo.do_rebuild();
        assert_eq!(demo.ui.image_view(demo.ids.edit_image).unwrap().scale, None);
    }

    #[test]
    fn rgba_images_preserve_pixels_alpha_clips_and_soft_gpu_agreement() {
        use next_gui_render::DrawCommand;
        use next_gui_scene::{DirtyRegion, Rect, RgbaImage};
        let image = RgbaImage::new(
            2,
            2,
            vec![
                255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 255, 255, 255, 255,
            ],
        )
        .unwrap();
        let clear = Color::rgb(20, 40, 60);
        let mut soft = SoftRenderer::new(16, 12, clear);
        let rect = Rect::new(-2.0, 1.0, 8.0, 8.0);
        let cmds = [DrawCommand::BlitRgba {
            rect,
            clip: Rect::new(0.0, 0.0, 5.0, 12.0),
            image: image.clone(),
        }];
        let mut dirty = DirtyRegion::new();
        dirty.add(Rect::new(0.0, 0.0, 16.0, 12.0));
        soft.render(&cmds, &dirty);
        assert_eq!(soft.pixel(0, 1), Some(0xff0000));
        assert_eq!(soft.pixel(2, 1), Some(0x09931d));
        assert_eq!(soft.pixel(0, 5), Some(clear.to_xrgb8888()));
        assert_eq!(soft.pixel(2, 5), Some(0xffffff));
        assert_eq!(soft.pixel(5, 1), Some(clear.to_xrgb8888()));
        let Ok(mut gpu) = GpuRenderer::try_new(16, 12, clear) else {
            eprintln!("skip RGBA GPU comparison: adapter unavailable");
            return;
        };
        eprintln!("RGBA comparison adapter: {}", gpu.backend_name());
        let compare = |soft: &SoftRenderer, gpu: &GpuRenderer| {
            for (a, b) in soft
                .frame_pixels()
                .unwrap()
                .0
                .iter()
                .zip(gpu.frame_pixels().unwrap().0)
            {
                for shift in [0, 8, 16] {
                    assert!(
                        (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs() <= 1,
                        "{a:06x} vs {b:06x}"
                    );
                }
            }
        };
        gpu.render(&cmds, &dirty);
        compare(&soft, &gpu);
        for rect in [
            Rect::new(1.25, 2.25, 7.5, 5.5),
            Rect::new(3.0, 2.0, 1.0, 1.0),
        ] {
            let cmds = [
                DrawCommand::BlitRgba {
                    rect,
                    clip: Rect::new(1.6, 1.6, 5.8, 7.8),
                    image: image.clone(),
                },
                DrawCommand::FillRect {
                    rect: Rect::new(13.0, 2.0, 1.0, 1.0),
                    color: Color::rgb(255, 0, 255),
                },
            ];
            soft.render(&cmds, &dirty);
            gpu.render(&cmds, &dirty);
            compare(&soft, &gpu);
            let mut patch = DirtyRegion::new();
            patch.add(Rect::new(2.0, 3.0, 3.0, 2.0));
            soft.render(&cmds, &patch);
            gpu.render(&cmds, &patch);
            compare(&soft, &gpu);
        }
    }

    #[test]
    fn selected_text_area_matches_soft_and_gpu_readback() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut area = WidgetId(0);
        let text: String = (1..=80).map(|i| format!("SELECT {i}, '中文';\n")).collect();
        ui.column(next_gui_ui::ColumnStyle { x: 0.0, y: 0.0, width: 400.0, height: 200.0, padding: 0.0, gap: 0.0, background: None, ..next_gui_ui::ColumnStyle::default() }, |c| {
            area = c.text_area(text, "SQL", next_gui_ui::TextAreaStyle { width: 400.0, height: 200.0, ..next_gui_ui::TextAreaStyle::default() });
        });
        ui.set_text_area_line_numbers(area, true);
        sql_syntax::apply(&mut ui, area, &Theme::dark());
        ui.select_text_area_range(area, 0..0);
        ui.find_in_text_area(area, "SELECT 40");
        let mut search = sql_search::Search::default();
        let highlights = search.apply(sql_search::Action::FindAll, ui.text_area_value(area).unwrap(), "中文", "", 0..0).unwrap().highlights;
        assert!(ui.set_text_area_highlights(area, highlights, Color::rgb(220, 160, 40)));
        ui.set_focus(Some(area));
        ui.sync(&mut scene);
        scene.mark_full_dirty(400.0, 200.0);
        let dirty = scene.take_dirty();
        let commands = encode_scene(&scene, &dirty);
        let clear = Color::rgb(20, 20, 20);
        let mut soft = SoftRenderer::new(400, 200, clear);
        soft.render(&commands, &dirty);
        let Ok(mut gpu) = GpuRenderer::try_new(400, 200, clear) else {
            eprintln!("GPU selection comparison not_run: adapter unavailable");
            return;
        };
        gpu.render(&commands, &dirty);
        let soft_pixels = soft.frame_pixels().unwrap().0;
        let gpu_pixels = gpu.frame_pixels().unwrap().0;
        // Compare editor contents; rounded outer chrome has separate renderer coverage tests.
        let max_delta = (6..194).flat_map(|y| (8..392).map(move |x| y * 400 + x)).flat_map(|i| {
            [0, 8, 16].map(|shift| (((soft_pixels[i] >> shift) & 255) as i32 - ((gpu_pixels[i] >> shift) & 255) as i32).abs())
        }).max().unwrap();
        assert!(max_delta <= 2, "Soft/GPU editor channel difference {max_delta}");
        eprintln!("Soft/GPU selected-text comparison passed, max channel delta {max_delta}");
    }

    #[test]
    fn sql_completion_auto_threshold_manual_keys_and_session_undo_do_not_execute_sql() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.prefs.sql_completion = true;
        demo.state.prefs.sql_upper_keywords = true;
        demo.state.sql_tabs[0].clear();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        for ch in ['S', 'E'] { assert!(demo.input_key_with_sql_completions(UiKey::Char(ch))); }
        assert!(!demo.ui.has_open_popup());
        demo.input_key_with_sql_completions(UiKey::Char('L'));
        assert!(demo.ui.text_area_completions_open(demo.ids.sql_area));
        assert!(demo.handle_sql_completion_key(KeyCode::Enter, Modifiers::default()));
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT"));
        assert!(!demo.state.sql_busy && !demo.state.dirty && demo.state.sql_result.is_none());
        demo.snapshot_inputs();
        demo.do_rebuild();
        demo.input_key_with_sql_completions(UiKey::Undo);
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SEL"));
        demo.state.prefs.sql_completion = false;
        demo.input_key_with_sql_completions(UiKey::Backspace);
        assert!(!demo.ui.has_open_popup());
        assert!(demo.handle_sql_completion_key(KeyCode::Space, Modifiers { ctrl: true, ..Modifiers::default() }));
        assert!(demo.ui.text_area_completions_open(demo.ids.sql_area));
        assert!(demo.handle_sql_completion_key(KeyCode::Tab, Modifiers::default()));
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT"));
        assert_eq!(demo.ui.focused(), Some(demo.ids.sql_area));
        demo.state.overlay = Some(OverlayKind::FindReplace);
        demo.snapshot_inputs();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.select_text_area_range(demo.ids.sql_area, 3..3);
        assert!(demo.show_sql_completions(true));
        demo.handle_sql_completion_key(KeyCode::Tab, Modifiers::default());
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT"), "replace the whole word at a middle caret");
        demo.ui.set_text_area_value(demo.ids.sql_area, "-- SEL");
        demo.ui.select_text_area_range(demo.ids.sql_area, 6..6);
        assert!(!demo.show_sql_completions(true));
        demo.state.prefs.sql_upper_keywords = false;
        demo.ui.set_text_area_value(demo.ids.sql_area, "sele");
        demo.ui.select_text_area_range(demo.ids.sql_area, 4..4);
        assert!(demo.show_sql_completions(true));
        demo.handle_sql_completion_key(KeyCode::Tab, Modifiers::default());
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("select"));
    }

    #[test]
    fn sql_completion_refreshes_metadata_and_works_from_cache_while_worker_owns_connection() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.run_sql_text_sync("CREATE TABLE completion_people(email_address); CREATE VIEW completion_broken AS SELECT * FROM missing").unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = "SELECT completion_people.ema".into();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        let end = demo.state.sql_editor().chars().count();
        demo.ui.select_text_area_range(demo.ids.sql_area, end..end);
        demo.state.start_sql_text("SELECT 1".into()).unwrap();
        assert!(demo.state.conn.is_none() && demo.state.sql_busy);
        assert!(demo.show_sql_completions(true));
        assert!(demo.ui.text_content(demo.ids.status).unwrap().contains("completion_broken"));
        demo.handle_sql_completion_key(KeyCode::Tab, Modifiers::default());
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT completion_people.email_address"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !demo.state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        demo.state.close_db();
        let (_, candidates) = demo.state.sql_completion.suggest("completion_peo", 14, true, true).unwrap();
        assert!(candidates.is_empty(), "closed databases must not leak old schema names");
    }

    #[test]
    fn sql_calltip_escape_rebuild_find_ime_and_undo_keep_the_editor_session() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        let before = (demo.state.conn.as_ref().unwrap().total_changes(), demo.state.conn.as_ref().unwrap().is_autocommit());
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = "SELECT coalesce(substr('中文', 1,".into();
        demo.do_rebuild();
        let area = demo.ids.sql_area;
        demo.ui.set_focus(Some(area));
        let end = demo.state.sql_editor().chars().count();
        demo.ui.select_text_area_range(area, end..end);
        demo.update_sql_hint();
        assert!(demo.ui.text_area_hint(area).unwrap().contains("Argument 3"));
        assert!(demo.handle_sql_completion_key(KeyCode::Escape, Modifiers::default()));
        demo.input_key_with_sql_completions(UiKey::Char(' '));
        demo.update_sql_hint();
        assert_eq!(demo.ui.text_area_hint(area), None);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_hint(demo.ids.sql_area), None);
        demo.input_key_with_sql_completions(UiKey::Char(')'));
        demo.update_sql_hint();
        assert_eq!(demo.ui.text_area_hint(demo.ids.sql_area), Some("coalesce(first, second, values…)\nArgument 1 · first\nFirst non-NULL value."));
        demo.input_key_with_sql_completions(UiKey::Undo);
        demo.update_sql_hint();
        assert!(demo.ui.text_area_hint(demo.ids.sql_area).unwrap().contains("Argument 3"));
        demo.ui.ime_preedit("中文", None);
        demo.update_sql_hint();
        assert_eq!(demo.ui.text_area_hint(demo.ids.sql_area), None);
        demo.ui.ime_clear_preedit();
        demo.state.overlay = Some(OverlayKind::FindReplace);
        demo.snapshot_inputs();
        demo.do_rebuild();
        demo.update_sql_hint();
        assert!(demo.handle_sql_completion_key(KeyCode::Escape, Modifiers::default()));
        assert_eq!(demo.state.overlay, Some(OverlayKind::FindReplace));
        assert_eq!(demo.ui.focused(), Some(demo.ids.sql_area));
        assert!(demo.state.sql_result.is_none());
        assert_eq!((demo.state.conn.as_ref().unwrap().total_changes(), demo.state.conn.as_ref().unwrap().is_autocommit()), before);
    }

    #[test]
    fn sql_calltip_many_overloads_are_individually_readable_without_editing() {
        let mut demo = Demo::new(800, 600, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        let conn = demo.state.conn.as_ref().unwrap();
        for arity in 1..=16 {
            conn.create_scalar_function("many_overloads", arity,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8, |_| Ok(1)).unwrap();
        }
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = "SELECT many_overloads(".into();
        demo.do_rebuild();
        let area = demo.ids.sql_area;
        demo.ui.set_focus(Some(area));
        let end = demo.state.sql_editor().chars().count();
        demo.ui.select_text_area_range(area, end..end);
        demo.update_sql_hint();
        let text = demo.ui.text_area_hint(area).unwrap();
        assert!(text.lines().count() <= 3, "overloads must not form a clipped wall of text: {text}");
        assert!(text.starts_with("many_overloads(arg1)\n"));
        assert_eq!(demo.ui.text_area_hint_page(area), Some((0, 16)));
        let shortcut = Modifiers { ctrl: true, shift: true, ..Modifiers::default() };
        for expected in 1..16 {
            assert!(demo.handle_sql_completion_key(KeyCode::Space, shortcut));
            demo.update_sql_hint();
            assert_eq!(demo.ui.text_area_hint_page(area), Some((expected, 16)));
        }
        demo.snapshot_inputs();
        demo.do_rebuild();
        let area = demo.ids.sql_area;
        assert_eq!(demo.ui.text_area_hint_page(area), Some((15, 16)));
        assert!(demo.ui.text_area_hint(area).unwrap().contains("arg16)"));
        assert_eq!(demo.ui.focused(), Some(area));
        assert_eq!(demo.ui.text_area_selection(area), Some(end..end));
        assert_eq!(demo.ui.text_area_value(area), Some("SELECT many_overloads("));
        assert!(demo.state.sql_result.is_none());
        assert!(demo.handle_sql_completion_key(KeyCode::Escape, Modifiers::default()));
        assert_eq!(demo.ui.text_area_hint_page(area), None);
        assert!(demo.handle_sql_completion_key(KeyCode::Space, shortcut));
        assert_eq!(demo.ui.text_area_hint_page(area), Some((0, 16)));
        demo.input_key_with_sql_completions(UiKey::Char(','));
        demo.update_sql_hint();
        assert_eq!(demo.ui.text_area_hint_page(area), Some((1, 16)));
        demo.ui.key_input(UiKey::Undo);
        assert_eq!(demo.ui.text_area_value(area), Some("SELECT many_overloads("));
    }

    #[test]
    fn sql_calltip_long_signature_scrolls_with_keyboard_without_editing() {
        let mut demo = Demo::new(800, 600, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().create_scalar_function("long_signature", 127,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8, |_| Ok(1)).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.prefs.font_size = 28.0;
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = "SELECT long_signature(".into();
        demo.do_rebuild();
        let area = demo.ids.sql_area;
        demo.ui.set_focus(Some(area));
        let end = demo.state.sql_editor().chars().count();
        demo.ui.select_text_area_range(area, end..end);
        demo.update_sql_hint();
        demo.ui.sync(&mut demo.scene);
        let before = demo.ui.text_area_scroll(area);
        for phase in ["top", "end"] {
            if phase == "end" {
                let shortcut = Modifiers { ctrl: true, shift: true, ..Modifiers::default() };
                assert!(demo.handle_sql_completion_key(KeyCode::PageDown, shortcut));
                assert!(demo.ui.scroll_text_area_hint(area, f32::MAX));
                demo.update_sql_hint();
                demo.ui.sync(&mut demo.scene);
            }
            let panel = demo.scene.iter_paint_order().into_iter().rev().find(|n|
                n.visible && n.fill == demo.ui.tooltip_style().bg && n.bounds.width > 100.0 && n.bounds.height > 400.0
            ).unwrap().bounds;
            assert!(panel.x >= 0.0 && panel.y >= 0.0 && panel.x + panel.width <= 800.0 && panel.y + panel.height <= 600.0);
            if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_CALLTIP_QA_OUT") {
                demo.scene.mark_full_dirty(800.0, 600.0);
                let dirty = demo.scene.take_dirty();
                let mut renderer = SoftRenderer::new(800, 600, clear_color(&demo.theme));
                renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
                write_ppm(&format!("{prefix}-long-{phase}.ppm"), renderer.frame_pixels().unwrap().0, 800, 600).unwrap();
            }
        }
        assert_eq!(demo.ui.text_area_scroll(area), before);
        assert_eq!(demo.ui.text_area_selection(area), Some(end..end));
        assert_eq!(demo.ui.text_area_value(area), Some("SELECT long_signature("));
        assert!(demo.state.sql_result.is_none());
    }

    #[test]
    fn sql_calltip_dark_light_and_metal_readback() {
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
            demo.state.new_memory_db().unwrap();
            demo.state.tab = MainTab::ExecuteSql;
            demo.theme = theme;
            demo.state.sql_tabs[0] = format!("{}SELECT coalesce(\n    substr('中文 SQL editor', 1,", "-- Function argument hints\n".repeat(24));
            demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.sql_area));
            let end = demo.state.sql_editor().chars().count();
            demo.ui.select_text_area_range(demo.ids.sql_area, end..end);
            demo.update_sql_hint();
            demo.ui.sync(&mut demo.scene);
            assert!(demo.ui.text_area_hint(demo.ids.sql_area).unwrap().contains("Argument 3"));
            let style = demo.ui.tooltip_style();
            let panel = demo.scene.iter_paint_order().into_iter().rev().find(|n|
                n.visible && n.fill == style.bg && n.bounds.width > 100.0 && n.bounds.height > 25.0
            ).unwrap().bounds;
            assert!(panel.x >= 0.0 && panel.y >= 0.0 && panel.x + panel.width <= 1400.0 && panel.y + panel.height <= 860.0);
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let commands = encode_scene(&demo.scene, &dirty);
            let clear = clear_color(&demo.theme);
            let mut soft = SoftRenderer::new(1400, 860, clear);
            soft.render(&commands, &dirty);
            if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_CALLTIP_QA_OUT") {
                write_ppm(&format!("{prefix}-{name}.ppm"), soft.frame_pixels().unwrap().0, 1400, 860).unwrap();
            }
            let Ok(mut gpu) = GpuRenderer::try_new(1400, 860, clear) else {
                eprintln!("calltip {name} GPU comparison not_run: adapter unavailable");
                continue;
            };
            gpu.render(&commands, &dirty);
            let soft_pixels = soft.frame_pixels().unwrap().0;
            let gpu_pixels = gpu.frame_pixels().unwrap().0;
            let max_delta = ((panel.y.ceil() as usize + 2)..(panel.y + panel.height).floor() as usize - 2).flat_map(|y|
                ((panel.x.ceil() as usize + 2)..(panel.x + panel.width).floor() as usize - 2).map(move |x| y * 1400 + x)
            ).flat_map(|i| [0, 8, 16].map(|shift|
                (((soft_pixels[i] >> shift) & 255) as i32 - ((gpu_pixels[i] >> shift) & 255) as i32).abs()
            )).max().unwrap();
            assert!(max_delta <= 2, "calltip {name} channel delta {max_delta}");
            eprintln!("calltip {name} Soft/Metal actual readback max channel delta {max_delta}");
        }
    }

    #[test]
    fn sql_completion_popup_dark_light_and_metal_readback() {
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch(
                "CREATE TABLE subscribers(subscription_id, \"subject line\", \"sub国际\"); ATTACH ':memory:' AS subarchive;"
            ).unwrap();
            demo.state.refresh_schema().unwrap();
            demo.state.tab = MainTab::ExecuteSql;
            demo.theme = theme;
            demo.state.sql_tabs[0] = format!("{}CREATE TABLE metrics(value NUMERIC) STRICT;\nSELECT sub", "-- completion demo 中文\n".repeat(24));
            demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.sql_area));
            let end = demo.state.sql_editor().chars().count();
            demo.ui.select_text_area_range(demo.ids.sql_area, end..end);
            demo.ui.sync(&mut demo.scene);
            assert!(demo.show_sql_completions(true));
            demo.ui.sync(&mut demo.scene);
            let style = demo.ui.context_menu_style();
            let panel = demo.scene.iter_paint_order().into_iter().rev().find(|n|
                n.visible && n.fill == style.popup_bg && n.bounds.width >= style.popup_width && n.bounds.height >= style.item_height
            ).unwrap().bounds;
            assert!(panel.x >= 0.0 && panel.y >= 0.0 && panel.x + panel.width <= 1400.0 && panel.y + panel.height <= 860.0);
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let commands = encode_scene(&demo.scene, &dirty);
            let clear = clear_color(&demo.theme);
            let mut soft = SoftRenderer::new(1400, 860, clear);
            soft.render(&commands, &dirty);
            if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_COMPLETION_QA_OUT") {
                write_ppm(&format!("{prefix}-{name}.ppm"), soft.frame_pixels().unwrap().0, 1400, 860).unwrap();
            }
            let Ok(mut gpu) = GpuRenderer::try_new(1400, 860, clear) else {
                eprintln!("completion {name} GPU comparison not_run: adapter unavailable");
                continue;
            };
            gpu.render(&commands, &dirty);
            let soft_pixels = soft.frame_pixels().unwrap().0;
            let gpu_pixels = gpu.frame_pixels().unwrap().0;
            let max_delta = ((panel.y.ceil() as usize + 2)..(panel.y + panel.height).floor() as usize - 2).flat_map(|y|
                ((panel.x.ceil() as usize + 2)..(panel.x + panel.width).floor() as usize - 2).map(move |x| y * 1400 + x)
            ).flat_map(|i| [0, 8, 16].map(|shift|
                (((soft_pixels[i] >> shift) & 255) as i32 - ((gpu_pixels[i] >> shift) & 255) as i32).abs()
            )).max().unwrap();
            assert!(max_delta <= 2, "completion {name} channel delta {max_delta}");
            eprintln!("completion {name} Soft/Metal actual readback max channel delta {max_delta}");
        }
    }

    #[test]
    fn numbered_sql_editor_retains_search_selection_and_undo_through_find_overlay() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = (1..=80).map(|i| match i {
            41 => format!("SELECT lower('中文 {i}'), :value, X'0102';\n"),
            42 => format!("/* 中文注释 */ SELECT {i};\n"),
            _ => format!("SELECT {i}, '中文 {i}';\n"),
        }).collect();
        demo.state.sql_result = None;
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        assert!(demo.ui.find_in_text_area(demo.ids.sql_area, "中文 40"));
        demo.ui.sync(&mut demo.scene);
        let selected = demo.ui.text_area_selection(demo.ids.sql_area).unwrap();
        assert!(demo.ui.text_area_scroll(demo.ids.sql_area).unwrap() > 0.0);
        assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("中文 40"));
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(selected.clone()));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, demo.win_w, demo.win_h).unwrap();
        }
        demo.ui.replace_text_area_selection(demo.ids.sql_area, "東京");
        sql_syntax::apply(&mut demo.ui, demo.ids.sql_area, &demo.theme);
        demo.ui.sync(&mut demo.scene);
        demo.snapshot_inputs();
        demo.state.overlay = Some(OverlayKind::FindReplace);
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        assert!(demo.ui.key_input(UiKey::Undo));
        assert_eq!(demo.ui.text_area_selected_text(demo.ids.sql_area).as_deref(), Some("中文 40"));
        assert!(demo.ui.text_area_value(demo.ids.sql_area).unwrap().contains("SELECT 40, '中文 40';"));
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(selected));
        let (x, y, _, _) = demo.ui.ime_cursor_area(&demo.scene).unwrap();
        assert!(x.is_finite() && y.is_finite());
        demo.snapshot_inputs();
        demo.theme = Theme::light();
        demo.do_rebuild();
        assert!(demo.scene.iter_paint_order().iter().any(|n| n.visible && n.fill == Color::rgb(104, 48, 156)),
            "theme rebuild reapplies the light keyword color after session restoration");
        assert!(!demo.state.dirty);
    }

    #[test]
    fn dock_tab_overflow_preserves_scroll_across_rebuild_and_reveals_selected_panels() {
        let mut demo = Demo::new(800, 600, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::Browse;
        demo.state.dock_focus = DockKind::Log;
        demo.state.body_ratio = 0.9;
        demo.do_rebuild();
        let header = |demo: &Demo| demo.ui.tabs_state(demo.ids.dock_tabs).unwrap();
        let bounds = demo.scene.iter_paint_order().into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.dock_tabs) && node.bounds.height > 32.0)
            .unwrap().bounds;
        let initial = header(&demo);
        assert!(demo.ui.pointer_pressed(&mut demo.scene, bounds.x + bounds.width - 8.0, bounds.y + bounds.height - 16.0));
        demo.ui.pointer_released();
        demo.ui.sync(&mut demo.scene);
        assert_eq!(demo.ui.tabs_selected(demo.ids.dock_tabs), Some(0));
        let scrolled = header(&demo);
        assert_ne!(scrolled, initial);
        demo.do_rebuild();
        assert_eq!(header(&demo), scrolled, "polling rebuild must retain manual tab scrolling");
        demo.ui.set_focus(Some(demo.ids.dock_tabs));
        assert!(demo.ui.key_input(UiKey::End));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.dock_focus, DockKind::Remote);
        assert_eq!(demo.ui.accessible_name(demo.ids.dock_tabs).as_deref(), Some("Remote"));
        assert_eq!(demo.ui.focused(),Some(demo.ids.dock_tabs));
        assert!(demo.ui.key_input(UiKey::Home));demo.snapshot_inputs();assert!(demo.rebuild);demo.do_rebuild();
        assert_eq!(demo.state.dock_focus,DockKind::Log);
        assert!(demo.ui.key_input(UiKey::End));demo.snapshot_inputs();assert!(demo.rebuild);demo.do_rebuild();
        assert_eq!(demo.state.dock_focus,DockKind::Remote);

        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            for (width, height, extension) in [(800, 600, "narrow.ppm"), (1400, 860, "ppm")] {
                demo.win_w = width; demo.win_h = height;
                demo.do_rebuild();
                demo.scene.mark_full_dirty(width as f32, height as f32);
                let dirty = demo.scene.take_dirty();
                let mut renderer = SoftRenderer::new(width, height, clear_color(&demo.theme));
                renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
                write_ppm(&PathBuf::from(&path).with_extension(extension).to_string_lossy(), renderer.frame_pixels().unwrap().0, width, height).unwrap();
            }
        }
        assert!(!demo.state.dirty);
    }

    #[test]
    fn sql_editor_page_navigation_and_selection_survive_rebuild() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        let original: String = (1..=80).map(|i| format!("SELECT {i:02}, '中文';\n")).collect();
        demo.state.sql_tabs[0] = original.clone();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        let origin = (original.lines().next().unwrap().chars().count() + 1) * 3 + 2;
        demo.ui.select_text_area_range(demo.ids.sql_area, origin..origin);
        assert!(!demo.handle_browse_navigation_key(KeyCode::PageDown, Modifiers::default()));
        assert!(demo.ui.key_input(map_key(KeyCode::PageDown, Modifiers::default()).unwrap()));
        let first = demo.ui.text_area_caret(demo.ids.sql_area).unwrap();
        assert!(first > origin + 30);
        assert!(demo.ui.key_input(map_key(KeyCode::PageDown, Modifiers { shift: true, ..Modifiers::default() }).unwrap()));
        let second = demo.ui.text_area_caret(demo.ids.sql_area).unwrap();
        assert!(second > first);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(first..second));
        assert!(demo.ui.text_area_scroll(demo.ids.sql_area).unwrap() > 0.0);
        assert_eq!(demo.state.sql_editor(), original);
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 1400, 860).unwrap();
        }
        assert!(demo.ui.key_input(map_key(KeyCode::PageUp, Modifiers { shift: true, ..Modifiers::default() }).unwrap()));
        assert_eq!(demo.ui.text_area_caret(demo.ids.sql_area), Some(first));
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(first..first));
    }

    #[test]
    fn sql_comment_changes_selected_lines_as_one_undoable_edit() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        let original = "SELECT '中文';\r\n  SELECT 2;\r\nSELECT 3;";
        demo.state.sql_tabs[0] = original.into();
        demo.do_rebuild();
        let start = "SELECT '中文';\r\n".chars().count();
        let end = start + "  SELECT 2;\r\n".chars().count();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.select_text_area_range(demo.ids.sql_area, start + 2..end);
        demo.handle_button(demo.ids.btn_sql_comment);
        let expected = "SELECT '中文';\r\n--   SELECT 2;\r\nSELECT 3;";
        assert_eq!(demo.state.sql_editor(), expected);
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.sql_area));
        assert!(demo.ui.key_input(UiKey::Undo));
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some(original));
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(start + 2..end));
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert!(demo.ui.key_input(UiKey::Redo));
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some(expected));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.snapshot_inputs();
            demo.do_rebuild();
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 1400, 860).unwrap();
        }
    }

    #[test]
    fn sql_comment_shortcut_keeps_file_drafts_and_tab_histories_separate() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("query.sql");
        let original = "-- SELECT '中文';\nSELECT 2;";
        std::fs::write(&path, original).unwrap();
        demo.state.open_sql_file(&path).unwrap();
        let file_tab = demo.state.sql_tab_idx;
        demo.do_rebuild();
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        let command = Modifiers { meta: true, ..Modifiers::default() };
        demo.ui.set_focus(Some(demo.ids.btn_sql_find));
        assert!(!demo.handle_sql_comment_key(KeyCode::Char('/'), command));
        demo.ui.set_focus(Some(demo.ids.sql_area));
        for modifiers in [Modifiers::default(), Modifiers { alt: true, ..command }, Modifiers { shift: true, ..command }] {
            assert!(!demo.handle_sql_comment_key(KeyCode::Char('/'), modifiers));
        }
        demo.state.overlay = Some(OverlayKind::FindReplace);
        assert!(!demo.handle_sql_comment_key(KeyCode::Char('/'), command));
        demo.state.overlay = None;
        demo.state.confirm_open = true;
        assert!(!demo.handle_sql_comment_key(KeyCode::Char('/'), command));
        demo.state.confirm_open = false;
        assert!(demo.handle_sql_comment_key(KeyCode::Char('/'), command));
        assert_eq!(demo.state.sql_editor(), "SELECT '中文';\nSELECT 2;");
        assert!(demo.state.sql_tab_dirty(file_tab));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        demo.do_rebuild();
        demo.action_new_sql_tab();
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.sql_area, "SELECT 99;");
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        assert!(demo.handle_sql_comment_key(KeyCode::Char('/'), Modifiers { ctrl: true, ..Modifiers::default() }));
        assert_eq!(demo.state.sql_editor(), "-- SELECT 99;");
        demo.do_rebuild();
        demo.snapshot_inputs();
        demo.state.switch_sql_tab(file_tab);
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        assert!(demo.ui.key_input(UiKey::Undo));
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some(original));
        demo.snapshot_inputs();
        assert!(!demo.state.sql_tab_dirty(file_tab));
        demo.state.switch_sql_tab(file_tab + 1);
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        assert!(demo.ui.key_input(UiKey::Undo));
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT 99;"));
        assert!(!demo.state.dirty, "comment edits must not change the database");
    }

    #[test]
    fn sql_long_document_viewport_restores_and_find_reveals_after_resize() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = (1..=80).map(|i| format!("SELECT {i};\n")).collect();
        demo.do_rebuild();
        demo.ui.set_text_area_scroll(demo.ids.sql_area, 150.0);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_scroll(demo.ids.sql_area), Some(150.0));
        demo.state.overlay = Some(OverlayKind::FindReplace);
        demo.do_rebuild();
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        demo.ui.set_text_area_scroll(demo.ids.sql_area, 0.0);
        assert!(demo.ui.find_in_text_area(demo.ids.sql_area, "SELECT 40"));
        demo.ui.sync(&mut demo.scene);
        demo.snapshot_inputs();
        let overlay_scroll = demo.ui.text_area_scroll(demo.ids.sql_area).unwrap();
        demo.state.overlay = None;
        demo.do_rebuild();
        assert!(demo.ui.text_area_scroll(demo.ids.sql_area).unwrap() > overlay_scroll);
        assert_eq!(demo.ui.text_area_selected_text(demo.ids.sql_area).as_deref(), Some("SELECT 40"));
    }

    #[test]
    fn compact_find_keeps_editor_and_single_toolbar_visible() {
        let mut demo = Demo::new(800, 600, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.toolbar_extra = true;
        demo.state.toolbar_project = true;
        demo.state.find_search.options.regex = true;
        demo.open_sql_find();
        demo.do_rebuild();
        let node = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.sql_area)).unwrap();
        let bounds = node.bounds;
        let mut parent = node.parent;
        let viewport = loop {
            let node = demo.scene.get(parent.unwrap()).unwrap();
            if demo.ui.widget_from_node(node.id).is_some_and(|id| demo.ui.scroll_offset(id).is_some()) { break node.bounds; }
            parent = node.parent;
        };
        assert!(bounds.y >= viewport.y && bounds.y + bounds.height <= viewport.y + viewport.height, "editor={bounds:?} viewport={viewport:?}");
        assert!(bounds.height >= 100.0);
        assert_ne!(demo.ids.toolbar_extra, WidgetId(0));
        assert_ne!(demo.ids.toolbar_project, WidgetId(0));
        assert!(demo.state.toolbar_extra && demo.state.toolbar_project);
        demo.handle_button(demo.ids.ov_btn_ok);
        demo.do_rebuild();
        assert_ne!(demo.ids.toolbar_extra, WidgetId(0));
        assert_ne!(demo.ids.toolbar_project, WidgetId(0));
    }

    #[test]
    fn sql_search_options_all_highlights_scope_undo_errors_and_function_shortcuts() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        let original = "SELECT 'alpha ALPHA alpha_x 中文 中文';\nSELECT 'alpha';";
        demo.state.sql_tabs[0] = original.into();
        demo.do_rebuild();
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        demo.handle_button(demo.ids.btn_sql_find);
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_find, "alpha");
        demo.ui.set_checked(demo.ids.ov_find_options[1], true);
        demo.handle_button(demo.ids.ov_btn_find_all);
        assert_eq!(demo.state.find_message, "3 match(es)");
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(0..0));
        assert!(!demo.state.find_search.options.case_sensitive);
        assert!(demo.state.find_search.options.whole_words);
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            for (width, height, suffix) in [(1400, 860, "ppm"), (800, 600, "narrow.ppm")] {
                demo.win_w = width; demo.win_h = height;
                demo.do_rebuild();
                demo.scene.mark_full_dirty(width as f32, height as f32);
                let dirty = demo.scene.take_dirty();
                let mut renderer = SoftRenderer::new(width, height, clear_color(&demo.theme));
                renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
                write_ppm(&PathBuf::from(&path).with_extension(suffix).to_string_lossy(), renderer.frame_pixels().unwrap().0, width, height).unwrap();
            }
            demo.win_w = 1400; demo.win_h = 860; demo.do_rebuild();
        }
        assert!(demo.handle_sql_search_key(KeyCode::Function(3), Modifiers::default()));
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selected_text(demo.ids.sql_area).as_deref(), Some("alpha"));
        assert!(demo.handle_sql_search_key(KeyCode::Function(3), Modifiers { shift: true, ..Modifiers::default() }));
        demo.do_rebuild();
        assert!(demo.ui.text_area_selection(demo.ids.sql_area).unwrap().start > 35);
        assert!(!demo.state.find_search.options.backwards);
        assert!(!demo.handle_sql_search_key(KeyCode::Function(3), Modifiers { ctrl: true, ..Modifiers::default() }));
        demo.ui.set_checked(demo.ids.ov_find_options[0], true);
        demo.handle_button(demo.ids.ov_btn_find_all);
        assert_eq!(demo.state.find_message, "2 match(es)");
        demo.do_rebuild();
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..original.lines().next().unwrap().chars().count());
        demo.ui.set_checked(demo.ids.ov_find_options[4], true);
        demo.ui.set_checked(demo.ids.ov_find_options[5], true);
        demo.ui.set_text_input(demo.ids.ov_find, "(中文)");
        demo.ui.set_text_input(demo.ids.ov_replace, r"東京🙂\1");
        demo.handle_button(demo.ids.ov_btn_repl_all);
        demo.do_rebuild();
        assert_eq!(demo.state.sql_editor(), original.replace("中文", "東京🙂中文"));
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.sql_editor(), original);
        demo.ui.set_checked(demo.ids.ov_find_options[4], false);
        demo.ui.set_text_input(demo.ids.ov_find, "[");
        let selection = demo.ui.text_area_selection(demo.ids.sql_area);
        demo.handle_button(demo.ids.ov_btn_repl_all);
        assert!(demo.state.error_msg.as_deref().unwrap().contains("Invalid search pattern"));
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), selection);
        assert_eq!(demo.state.sql_editor(), original);
        demo.state.find_search.options.selection_only = true;
        demo.state.find_search.options.regex = false;
        demo.state.find_search.reset(original, 0..35);
        demo.state.sql_tabs.push(original.into());
        demo.state.switch_sql_tab(1);
        let error = demo.state.find_search.apply(sql_search::Action::Find, original, "alpha", "", 0..0).err().unwrap();
        assert!(error.contains("Select text"), "{error}");
    }

    #[test]
    fn sql_selection_find_replace_and_undo_survive_tabs_and_rebuild() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.state.sql_tabs[0] = "SELECT '中文';\nSELECT '中文';".into();
        demo.do_rebuild();
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
        demo.snapshot_inputs();
        demo.state.find_text = "中文".into();
        demo.state.replace_text = "東京".into();
        demo.state.overlay = Some(OverlayKind::FindReplace);
        demo.do_rebuild();
        demo.handle_button(demo.ids.ov_btn_find);
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(8..10));
        demo.handle_button(demo.ids.ov_btn_find);
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(21..23));
        demo.handle_button(demo.ids.ov_btn_repl);
        demo.do_rebuild();
        assert_eq!(demo.state.sql_editor(), "SELECT '中文';\nSELECT '東京';");
        demo.handle_button(demo.ids.ov_btn_repl_all);
        demo.do_rebuild();
        assert_eq!(demo.state.sql_editor(), "SELECT '東京';\nSELECT '東京';");
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.sql_editor(), "SELECT '中文';\nSELECT '東京';");
        demo.state.overlay = None;
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.key_input(UiKey::Undo);
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT '中文';\nSELECT '中文';"));
        demo.snapshot_inputs();
        demo.handle_button(demo.ids.btn_sql_new_tab);
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.key_input(UiKey::Undo);
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some(""));
        demo.handle_button(demo.ids.sql_tab_btns[0]);
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area), Some(21..23));
        demo.ui.set_focus(Some(demo.ids.sql_area));
        demo.ui.key_input(UiKey::Redo);
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some("SELECT '中文';\nSELECT '東京';"));
        demo.ui.set_text_area_value(demo.ids.sql_area, "SELECT 42;\nINVALID SQL;");
        demo.ui.select_text_area_range(demo.ids.sql_area, 0..10);
        demo.handle_button(demo.ids.btn_sql_run);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !demo.state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(demo.state.sql_result.as_ref().unwrap().rows[0][0], "42");
        assert_eq!(demo.state.sql_editor(), "SELECT 42;\nINVALID SQL;");
    }

    #[test]
    fn index_key_order_selector_preserves_operands_comments_and_other_keys() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE \"order table\"(desc TEXT, asc INTEGER, v TEXT); INSERT INTO \"order table\" VALUES('d',1,'v')").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.open_create_index_wizard();
        demo.state.wizard_index_table = "order table".into();
        demo.state.wizard_index_name = "order index".into();
        let expression = "coalesce(v, '; ASC') COLLATE NOCASE DESC -- DESC,;)";
        demo.state.wizard_index_cols = format!("desc,\n{expression}");
        demo.do_rebuild();
        assert_eq!(demo.ui.select_selected(demo.ids.ov_idx_order), Some(0));
        assert_eq!(demo.ui.accessible_name(demo.ids.ov_idx_order).as_deref(), Some("Selected index key sort order"));
        demo.ui.set_select_selected(demo.ids.ov_idx_order, Some(1));
        demo.snapshot_inputs();
        let descending_first = demo.state.wizard_index_cols.clone();
        assert_eq!(crate::index_sql::keys(&descending_first).unwrap()[0], "desc\nDESC");
        assert_eq!(crate::index_sql::keys(&descending_first).unwrap()[1], expression);
        demo.ui.set_focus(Some(demo.ids.ov_idx_cols));
        demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_index_cols, format!("desc,\n{expression}"));
        assert_eq!(demo.ui.select_selected(demo.ids.ov_idx_order), Some(0));
        demo.ui.key_input(UiKey::Redo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_index_cols, descending_first);
        demo.ui.set_list_selected(demo.ids.ov_list, Some(1));
        demo.snapshot_inputs();
        assert_eq!(demo.ui.select_selected(demo.ids.ov_idx_order), Some(1));
        assert_eq!(demo.state.wizard_index_cols, descending_first);
        demo.ui.set_select_selected(demo.ids.ov_idx_order, Some(0));
        demo.snapshot_inputs();
        let mixed_order = demo.state.wizard_index_cols.clone();
        let keys = crate::index_sql::keys(&mixed_order).unwrap();
        assert_eq!(keys[0], "desc\nDESC");
        assert_eq!(keys[1], "coalesce(v, '; ASC') COLLATE NOCASE  -- DESC,;)\nASC");
        demo.ui.set_list_selected(demo.ids.ov_list, Some(0));
        demo.snapshot_inputs();
        assert_eq!(demo.ui.select_selected(demo.ids.ov_idx_order), Some(1));
        assert_eq!(demo.state.wizard_index_cols, mixed_order);
        demo.ui.set_focus(Some(demo.ids.ov_idx_order));
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.ov_idx_order));
        assert_eq!(demo.ui.select_selected(demo.ids.ov_idx_order), Some(1));
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        let conn = demo.state.conn.as_ref().unwrap();
        let mut stmt = conn.prepare("PRAGMA main.index_xinfo('order index')").unwrap();
        let metadata = stmt.query_map([], |row| Ok((row.get::<_, Option<String>>(2)?, row.get::<_, bool>(3)?, row.get::<_, String>(4)?, row.get::<_, bool>(5)?))).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(metadata.into_iter().filter(|(_, _, _, key)| *key).map(|(name, desc, coll, _)| (name, desc, coll)).collect::<Vec<_>>(),
            [(Some("desc".into()), true, "BINARY".into()), (None, false, "NOCASE".into())]);
        drop(stmt);
        demo.state.open_modify_index_wizard().unwrap();
        demo.do_rebuild();
        assert_eq!(demo.state.wizard_index_cols, mixed_order);
        demo.ui.set_list_selected(demo.ids.ov_list, Some(1));
        demo.snapshot_inputs();
        assert_eq!(demo.ui.select_selected(demo.ids.ov_idx_order), Some(0));
        assert_eq!(demo.state.wizard_index_cols, mixed_order);
    }

    #[test]
    fn index_key_rows_share_sql_undo_preview_and_qualified_columns() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(SCHEMA_NAMESPACE_QA_SQL).unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("ALTER TABLE analytics.customers ADD COLUMN \"列,名\" TEXT GENERATED ALWAYS AS (region) VIRTUAL").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.open_create_index_wizard();
        demo.state.wizard_schema = "analytics".into();
        demo.state.wizard_index_table = "customers".into();
        demo.state.wizard_index_name = "ordered.\"index".into();
        demo.do_rebuild();
        assert_eq!(demo.state.index_wizard_columns().unwrap(), ["id", "region", "revenue", "列,名"]);
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE pragma_table_xinfo(name TEXT, type TEXT)").unwrap();
        assert_eq!(demo.state.index_wizard_column_details().unwrap(), [("id".into(), "INTEGER".into()), ("region".into(), "TEXT".into()), ("revenue".into(), "REAL".into()), ("列,名".into(), "TEXT".into())]);
        assert!(dbmod::index_table_columns(demo.state.conn.as_ref().unwrap(), "analytics", "missing").is_err());
        assert_eq!(demo.ui.button_enabled(demo.ids.ov_btn_del), Some(false));
        demo.ui.set_list_selected(demo.ids.ov_idx_available, Some(3));
        demo.handle_button(demo.ids.ov_btn_add);
        assert_eq!(demo.state.wizard_index_cols, "\"列,名\"");
        demo.handle_button(demo.ids.ov_idx_expression);
        assert_eq!(demo.ui.text_area_selected_text(demo.ids.ov_idx_cols).as_deref(), Some("NULL"));
        let expression = "coalesce(region, '空,;)') COLLATE NOCASE DESC -- keep ,";
        demo.ui.paste_into_focused_text(expression);
        demo.snapshot_inputs();
        let before_move = demo.state.wizard_index_cols.clone();
        demo.ui.set_list_selected(demo.ids.ov_list, Some(1));
        demo.handle_button(demo.ids.ov_btn_up);
        let moved = demo.state.wizard_index_cols.clone();
        assert_eq!(crate::index_sql::keys(&moved).unwrap(), [expression, "\"列,名\""]);
        assert_eq!(demo.ui.button_enabled(demo.ids.ov_btn_up), Some(false));
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.ov_idx_cols));
        demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_index_cols, before_move);
        demo.ui.key_input(UiKey::Redo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_index_cols, moved);
        demo.ui.set_list_selected(demo.ids.ov_list, Some(0));
        demo.handle_button(demo.ids.ov_btn_down);
        assert_eq!(demo.state.wizard_index_cols, before_move);
        demo.ui.set_list_selected(demo.ids.ov_list, Some(0));
        remove_selected_field_or_index(&mut demo);
        assert_eq!(demo.state.wizard_index_cols, expression);
        demo.ui.set_focus(Some(demo.ids.ov_idx_cols));
        demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_index_cols, before_move);
        demo.handle_button(demo.ids.ov_idx_edit_key);
        assert_eq!(demo.ui.text_area_selected_text(demo.ids.ov_idx_cols).as_deref(), Some("\"列,名\""));
        demo.ui.set_text_area_value(demo.ids.ov_idx_where, "revenue > 100");
        demo.ui.set_checked(demo.ids.ov_idx_unique, true);
        demo.snapshot_inputs();
        let preview = demo.ui.text_area_value(demo.ids.ov_idx_preview).unwrap().to_string();
        assert_eq!(preview, demo.state.index_wizard_sql());
        assert!(preview.starts_with("CREATE UNIQUE INDEX \"analytics\".\"ordered.\"\"index\" ON \"customers\""));
        assert!(preview.ends_with("\n) WHERE revenue > 100"));
        assert!(!demo.ui.replace_text_area_range(demo.ids.ov_idx_preview, 0..0, "DELETE"));
        demo.ui.select_text_area_range(demo.ids.ov_idx_preview, 0..12);
        demo.ui.set_focus(Some(demo.ids.ov_idx_preview));
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.ov_idx_preview));
        assert_eq!(demo.ui.text_area_selection(demo.ids.ov_idx_preview), Some(0..12));
        assert!(!demo.state.dirty);
        let objects = dbmod::list_schema(demo.state.conn.as_ref().unwrap()).unwrap();
        assert!(!objects.iter().any(|o| o.name == "ordered.\"index"));
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        let conn = demo.state.conn.as_ref().unwrap();
        let sql: String = conn.query_row("SELECT sql FROM analytics.sqlite_schema WHERE name=?1", ["ordered.\"index"], |row| row.get(0)).unwrap();
        assert_eq!(crate::index_sql::definition(&sql).unwrap(), (before_move, "revenue > 100".into()));
        let index: (i64, i64) = conn.query_row("SELECT count(*), max(desc) FROM pragma_index_xinfo(?1, 'analytics') WHERE key=1", ["ordered.\"index"], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(index, (2, 1));
        for i in 0..20 {
            conn.execute_batch(&format!("ALTER TABLE analytics.customers ADD COLUMN col{i}")).unwrap();
        }
        demo.state.refresh_schema().unwrap();
        demo.state.open_create_index_wizard();
        demo.state.wizard_schema = "analytics".into();
        demo.state.wizard_index_table = "customers".into();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.ov_idx_available));
        demo.ui.key_input(UiKey::End);
        let available_offset = demo.ui.list_scroll_offset(demo.ids.ov_idx_available).unwrap();
        assert!(available_offset > 0.0);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.ov_idx_available));
        assert_eq!(demo.ui.list_selected(demo.ids.ov_idx_available), Some(23));
        assert_eq!(demo.ui.list_scroll_offset(demo.ids.ov_idx_available), Some(available_offset));
        for _ in 0..8 { demo.handle_button(demo.ids.ov_btn_add); }
        let keys_offset = demo.ui.list_scroll_offset(demo.ids.ov_list).unwrap();
        assert!(keys_offset > 0.0);
        demo.ui.set_focus(Some(demo.ids.ov_list));
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.ov_list));
        assert_eq!(demo.ui.list_selected(demo.ids.ov_list), Some(7));
        assert_eq!(demo.ui.list_scroll_offset(demo.ids.ov_list), Some(keys_offset));
        demo.state.open_create_index_wizard();
        demo.do_rebuild();
        let temp = demo.state.index_tables().iter().position(|pair| *pair == ("temp", "customers")).unwrap();
        demo.ui.set_select_selected(demo.ids.ov_idx_table, Some(temp));
        demo.snapshot_inputs();
        assert!(demo.rebuild);
        demo.do_rebuild();
        assert_eq!(demo.state.index_wizard_columns().unwrap(), ["temp_value"]);
        demo.handle_button(demo.ids.ov_btn_add);
        assert_eq!(demo.state.wizard_index_cols, "\"temp_value\"");
        demo.ui.set_text_area_value(demo.ids.ov_idx_cols, "coalesce(");
        demo.snapshot_inputs();
        assert_eq!(demo.ui.button_enabled(demo.ids.ov_btn_add), Some(false));
        demo.handle_button(demo.ids.ov_btn_add);
        assert_eq!(demo.state.wizard_index_cols, "coalesce(");
    }

    #[test]
    fn table_move_schema_selector_preview_failure_and_save_keep_the_draft_and_rows() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS analytics;
                CREATE TABLE main.records(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL CHECK(length(name)>0),amount REAL DEFAULT 0,total REAL AS(amount*1.2) STORED);
                INSERT INTO main.records(id,name,amount) VALUES(7,'First customer',12.5),(11,'Second customer',24);
                CREATE INDEX main.by_name ON records(name COLLATE NOCASE);
                CREATE VIEW main.visible AS SELECT id,name,amount,total FROM main.records;CREATE VIEW main.customer_summary AS SELECT name,amount FROM main.visible;
                CREATE TRIGGER main.edit_visible INSTEAD OF UPDATE ON visible BEGIN UPDATE records SET name=new.name,amount=new.amount WHERE id=old.id;END;
                CREATE TABLE analytics.occupied(untouched);INSERT INTO analytics.occupied VALUES(42);").unwrap();
            demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.schema=="main" && object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            demo.handle_button(demo.ids.ov_table_advanced);demo.do_rebuild();
            let schema=demo.state.table_schemas().iter().position(|name|*name=="analytics").unwrap();assert_ne!(demo.ids.ov_schema.0,0);demo.ui.set_select_selected(demo.ids.ov_schema,Some(schema));demo.ui.set_text_input(demo.ids.ov_name,"occupied");demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}
            assert_eq!(demo.state.wizard_schema,"analytics");assert!(demo.state.table_wizard_sql().unwrap().contains("\"analytics\".\"occupied\""));demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.as_deref().unwrap().contains("Target already exists"));assert!(demo.state.overlay.is_some());assert_eq!(demo.state.wizard_table_name,"occupied");assert_eq!(demo.state.wizard_col_sources.len(),4);
            demo.state.error_msg=None;demo.do_rebuild();demo.ui.set_text_input(demo.ids.ov_name,"customer_archive");demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();
                assert_eq!(demo.ui.select_selected(demo.ids.ov_schema),Some(schema));assert!(demo.state.table_wizard_sql().unwrap().contains("\"analytics\".\"customer_archive\""));
                let bounds=|id|demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
                let grid=bounds(demo.ids.ov_fields_table);let tabs=bounds(demo.ids.ov_table_tabs);let style=next_gui_ui::TableStyle::from_theme(&demo.theme);
                assert!(grid.y+style.header_height+style.row_height<=tabs.y+tabs.height,"{theme_name} {width}: first row clipped: {grid:?}, {tabs:?}");
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_MOVE_QA_OUT") {
                    demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name} {width}: {delta}");eprintln!("Table move {theme_name} {width}: Soft/Metal delta {delta}");}else{eprintln!("Table move GPU comparison not_run: adapter unavailable");}
                }
            }
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());assert_eq!(demo.state.browse().schema,"analytics");assert_eq!(demo.state.browse().table.as_deref(),Some("customer_archive"));assert_eq!(demo.state.browse().result.as_ref().unwrap().rows.len(),2);
            let conn=demo.state.conn.as_ref().unwrap();assert!(conn.prepare("SELECT * FROM main.records").is_err());assert_eq!(conn.query_row("SELECT untouched FROM analytics.occupied",[],|row|row.get::<_,i64>(0)).unwrap(),42);assert_eq!(conn.query_row("SELECT sum(id) FROM analytics.customer_archive",[],|row|row.get::<_,i64>(0)).unwrap(),18);assert!(conn.execute("UPDATE analytics.customer_archive SET name=''",[]).is_err());assert_eq!(conn.query_row("SELECT total FROM analytics.customer_archive WHERE id=7",[],|row|row.get::<_,f64>(0)).unwrap(),15.0);
            assert!(conn.prepare("SELECT * FROM main.visible").is_err());assert_eq!(conn.query_row("SELECT sum(id) FROM analytics.visible",[],|row|row.get::<_,i64>(0)).unwrap(),18);conn.execute("UPDATE analytics.visible SET name='changed',amount=10 WHERE id=7",[]).unwrap();assert_eq!(conn.query_row("SELECT total FROM analytics.customer_archive WHERE id=7",[],|row|row.get::<_,f64>(0)).unwrap(),12.0);assert_eq!(conn.query_row("SELECT name FROM analytics.customer_summary WHERE amount=10",[],|row|row.get::<_,String>(0)).unwrap(),"changed");
        }
    }

    #[test]
    fn schema_wizard_selectors_keep_qualified_targets_types_and_scroll() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(SCHEMA_NAMESPACE_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.handle_menu(mid::EDIT_CREATE_TABLE);
        demo.do_rebuild();
        demo.handle_button(demo.ids.ov_table_advanced); demo.do_rebuild();
        let schema = demo.state.table_schemas().iter().position(|s| *s == "analytics").unwrap();
        demo.ui.set_select_selected(demo.ids.ov_schema, Some(schema));
        demo.ui.set_text_input(demo.ids.ov_name, "created.\"table");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        assert_eq!(demo.state.browse().schema, "analytics");
        assert_eq!(demo.state.browse().table.as_deref(), Some("created.\"table"));
        demo.do_rebuild();
        demo.handle_menu(mid::EDIT_CREATE_INDEX);
        demo.do_rebuild();
        let table = demo.state.index_tables().iter().position(|pair| *pair == ("main", "customers")).unwrap();
        demo.ui.set_select_selected(demo.ids.ov_idx_table, Some(table));
        demo.ui.set_text_input(demo.ids.ov_name, "created.\"index");
        let keys = "lower(name) COLLATE NOCASE DESC,\nlength(name) -- key comment";
        assert_eq!(demo.ui.accessible_name(demo.ids.ov_idx_cols).as_deref(), Some("Index columns and expressions"));
        assert_eq!(demo.ui.accessible_name(demo.ids.ov_idx_where).as_deref(), Some("Partial index WHERE condition"));
        demo.ui.set_text_area_value(demo.ids.ov_idx_cols, keys);
        demo.ui.set_text_area_value(demo.ids.ov_idx_where, "id > 0\nAND id < 100");
        for (index, value) in [keys, "id > 0\nAND id < 100"].into_iter().enumerate() {
            let area = [demo.ids.ov_idx_cols, demo.ids.ov_idx_where][index];
            demo.ui.set_focus(Some(area));
            demo.ui.select_text_area_range(area, value.chars().count()..value.chars().count());
            demo.ui.paste_into_focused_text("\n");
            demo.snapshot_inputs();
            let selection = demo.ui.text_area_selection(area);
            demo.do_rebuild();
            let area = [demo.ids.ov_idx_cols, demo.ids.ov_idx_where][index];
            assert_eq!(demo.ui.focused(), Some(area));
            assert_eq!(demo.ui.text_area_selection(area), selection);
            demo.ui.key_input(UiKey::Undo);
            assert_eq!(demo.ui.text_area_value(area), Some(value));
            demo.snapshot_inputs();
        }
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        assert_eq!(demo.state.selected.as_ref().unwrap().schema, "main");
        demo.do_rebuild();
        demo.handle_button(demo.ids.btn_struct_modify);
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_value(demo.ids.ov_idx_where), Some("id > 0\nAND id < 100"));
        assert_eq!(demo.ui.text_area_value(demo.ids.ov_idx_cols), Some(keys));
        let table = demo.state.index_tables().iter().position(|pair| *pair == ("analytics", "customers")).unwrap();
        demo.ui.set_select_selected(demo.ids.ov_idx_table, Some(table));
        demo.ui.set_text_area_value(demo.ids.ov_idx_cols, "missing_column");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_some());
        assert!(dbmod::list_schema(demo.state.conn.as_ref().unwrap()).unwrap().iter().any(|o| o.name == "created.\"index"));
        demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.ov_idx_cols, "region COLLATE NOCASE DESC");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        assert_eq!(demo.state.selected.as_ref().unwrap().schema, "analytics");
        assert!(demo.state.ddl_draft.contains("WHERE id > 0"));
        assert!(!demo.state.schema.iter().any(|o| o.schema == "main" && o.name == "created.\"index"));
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE analytics.custom_types(untyped, amount DECIMAL(10,2));").unwrap();
        demo.state.refresh_schema().unwrap();
        let table = demo.state.schema.iter().position(|o| o.schema == "analytics" && o.name == "custom_types").unwrap();
        demo.state.select_schema_node(10_000 + table as u64);
        demo.do_rebuild();
        demo.handle_button(demo.ids.btn_struct_modify);
        demo.do_rebuild();
        assert_eq!(demo.state.wizard_cols[0].type_name, "");
        demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
        demo.snapshot_inputs();
        demo.do_rebuild();
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_cols[0].type_name, "");
        assert_eq!(demo.state.wizard_cols[1].type_name, "DECIMAL(10,2)");
        assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type), Some("DECIMAL(10,2)"));
        demo.ui.set_autocomplete_value(demo.ids.ov_col_type, "INTEGER");
        demo.snapshot_inputs();
        assert_ne!(demo.state.wizard_cols[1].type_name, "DECIMAL(10,2)");
        demo.ui.set_autocomplete_value(demo.ids.ov_col_type, "DECIMAL(10,2)");
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_cols[1].type_name, "DECIMAL(10,2)");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        for (kind, index) in [("table", false), ("index", true)] {
            for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
                for (width, height) in [(1400, 1000), (800, 600)] {
                    demo.win_w = width;
                    demo.win_h = height;
                    demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                    if index {
                        demo.state.open_create_index_wizard();
                        demo.state.wizard_index_table = "customers".into();
                        demo.state.wizard_index_name = "idx_customer_region".into();
                        demo.state.wizard_index_cols = "lower(region) COLLATE NOCASE DESC,\nrevenue -- keep column comment".into();
                        demo.state.wizard_index_where = "revenue >= 100\nAND region IS NOT NULL".into();
                    }
                    else {
                        demo.state.open_create_table_wizard();
                        demo.state.wizard_cols[1].type_name = "DECIMAL(10,2)".into();
                        demo.state.wizard_cols.push(dbmod::ColDef::new("untyped", ""));
                        demo.state.wizard_col_sel = 1;
                    }
                    demo.state.wizard_schema = "analytics".into();
                    demo.do_rebuild();
                    let phases: &[&str] = if width == 800 && index { &["top", "middle", "bottom"] }
                        else if width == 800 { &["top", "bottom", "type-options", "type-ime"] }
                        else if !index { &["top", "type-options", "type-ime"] } else { &["top"] };
                    for phase in phases {
                        if !index && matches!(*phase, "type-options" | "type-ime") {
                            demo.ui.set_focus(Some(demo.ids.ov_col_type));
                            demo.ui.set_autocomplete_value(demo.ids.ov_col_type, "");
                            demo.ui.key_input(UiKey::Down);
                            if *phase == "type-ime" {
                                demo.ui.ime_preedit("中文类型", Some((6, 6)));
                                assert!(!demo.ui.has_open_popup());
                            }
                            demo.ui.sync(&mut demo.scene);
                            let focus = demo.ui.ime_cursor_area(&demo.scene).unwrap();
                            assert!(focus.0 >= 0.0 && focus.0 + focus.2 <= width as f32);
                        }
                        if matches!(*phase, "middle" | "bottom") {
                            assert!(demo.ui.set_scroll_offset(demo.ids.ov_scroll, 0.0, if *phase == "middle" { 200.0 } else { 10000.0 }));
                            demo.ui.sync(&mut demo.scene);
                            let offset = demo.ui.scroll_offset(demo.ids.ov_scroll).unwrap();
                            assert!(offset.1 > 0.0);
                            demo.snapshot_inputs();
                            demo.do_rebuild();
                            assert_eq!(demo.ui.scroll_offset(demo.ids.ov_scroll), Some(offset));
                        }
                        let bounds = |id| demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(id)).unwrap().bounds;
                        let viewport = bounds(demo.ids.ov_scroll);
                        let save = bounds(demo.ids.ov_btn_ok);
                        assert!(viewport.y + viewport.height <= save.y && save.y + save.height <= height as f32);
                        if *phase == "bottom" {
                            let field = bounds(if index { demo.ids.ov_idx_preview } else { demo.ids.ov_generated });
                            assert!(field.y >= viewport.y && field.y + field.height <= viewport.y + viewport.height);
                        }
                        if *phase == "middle" {
                            let field = bounds(demo.ids.ov_idx_cols);
                            assert!(field.y >= viewport.y && field.y + field.height <= viewport.y + viewport.height, "{field:?} within {viewport:?}");
                        }
                        if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT") {
                            demo.scene.mark_full_dirty(width as f32, height as f32);
                            let dirty = demo.scene.take_dirty();
                            let commands = encode_scene(&demo.scene, &dirty);
                            let clear = clear_color(&demo.theme);
                            let mut soft = SoftRenderer::new(width, height, clear);
                            soft.render(&commands, &dirty);
                            write_ppm(&format!("{prefix}-{kind}-{theme_name}-{width}-{phase}.ppm"), soft.frame_pixels().unwrap().0, width, height).unwrap();
                            if let Ok(mut gpu) = GpuRenderer::try_new(width, height, clear) {
                                gpu.render(&commands, &dirty);
                                let a = soft.frame_pixels().unwrap().0;
                                let b = gpu.frame_pixels().unwrap().0;
                                write_ppm(&format!("{prefix}-{kind}-{theme_name}-{width}-{phase}-metal.ppm"), b, width, height).unwrap();
                                let delta = ((viewport.y.ceil() as usize + 2)..(viewport.y + viewport.height).floor() as usize - 2).flat_map(|y|
                                    ((viewport.x.ceil() as usize + 2)..(viewport.x + viewport.width).floor() as usize - 2).map(move |x| y * width as usize + x)
                                ).flat_map(|i| [0, 8, 16].map(|shift| (((a[i] >> shift) & 255) as i32 - ((b[i] >> shift) & 255) as i32).abs())).max().unwrap();
                                assert!(delta <= 2, "Wizard {kind}/{theme_name}/{width}/{phase} full viewport channel delta {delta}");
                                eprintln!("Wizard {kind}/{theme_name}/{width}/{phase} Soft/Metal actual readback: full viewport including primitive edges {delta}");
                            } else { eprintln!("Wizard GPU comparison not_run: adapter unavailable"); }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn table_wizard_accepts_custom_and_empty_types_with_rebuild_safe_ime() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.open_create_table_wizard();
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_name, "custom_types");
        demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
        demo.snapshot_inputs();
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_col_name, "amount");
        demo.ui.set_focus(Some(demo.ids.ov_col_type));
        demo.ui.key_input(UiKey::SelectAll);
        assert!(demo.ui.paste_into_focused_text("DECIMAL(10,2)"));
        demo.snapshot_inputs();
        demo.ui.key_input(UiKey::Home);
        demo.ui.ime_preedit("中文", Some((3, 3)));
        demo.ui.sync(&mut demo.scene);
        let before = demo.ui.ime_cursor_area(&demo.scene).unwrap();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.ov_col_type));
        assert_eq!(demo.ui.ime_cursor_area(&demo.scene), Some(before));
        assert!(demo.ui.ime_commit("类型"));
        assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type), Some("类型DECIMAL(10,2)"));
        demo.ui.key_input(UiKey::SelectAll);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert!(demo.ui.paste_into_focused_text("DECIMAL(10,2)"));
        demo.snapshot_inputs();
        demo.handle_button(demo.ids.ov_btn_add);
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_col_name, "untyped");
        demo.ui.set_autocomplete_value(demo.ids.ov_col_type, "");
        demo.snapshot_inputs();
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        let types: Vec<_> = dbmod::table_columns(demo.state.conn.as_ref().unwrap(), "custom_types").unwrap().into_iter().map(|col| col.type_name).collect();
        assert_eq!(types, ["INTEGER", "DECIMAL(10,2)", ""]);
        let customers = dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "customers").unwrap();
        demo.state.open_create_table_wizard();
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_name, "bad_type");
        demo.ui.set_autocomplete_value(demo.ids.ov_col_type, "TEXT); DELETE FROM customers; --");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_some());
        assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type), Some("TEXT); DELETE FROM customers; --"));
        assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "customers").unwrap(), customers);
        assert!(!dbmod::list_schema(demo.state.conn.as_ref().unwrap()).unwrap().iter().any(|obj| obj.name == "bad_type"));
    }

    #[test]
    fn modify_table_tracks_column_identity_through_buttons_rebuild_and_save() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "customers" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000 + row as u64);
            demo.state.open_modify_table_wizard().unwrap();
            let sources = demo.state.wizard_col_sources.clone();
            let original = dbmod::browse_table(demo.state.conn.as_ref().unwrap(), "customers", &[], None, 500, 0).unwrap().rows;
            demo.do_rebuild();
            demo.ui.set_text_input(demo.ids.ov_col_name, "customer_key");
            demo.handle_button(demo.ids.ov_btn_down);
            demo.do_rebuild();
            assert_eq!(demo.state.wizard_col_sources[1], sources[0]);
            assert_eq!(demo.ui.text_input_value(demo.ids.ov_col_name), Some("customer_key"));
            demo.handle_button(demo.ids.ov_btn_up);
            demo.do_rebuild();
            assert_eq!(demo.state.wizard_col_sources, sources);
            demo.handle_button(demo.ids.ov_btn_add);
            demo.do_rebuild();
            assert_eq!(demo.state.wizard_col_sources.last(), Some(&None));
            remove_selected_field_or_index(&mut demo);
            demo.do_rebuild();
            assert_eq!(demo.state.wizard_col_sources, sources);
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(0));
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.text_input_value(demo.ids.ov_col_name), Some("customer_key"));
            for phase in ["draft", "saved"] {
                if phase == "saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                    assert!(demo.state.overlay.is_none());
                    demo.state.tab = MainTab::Browse;
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_RENAME_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Column rename {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Column rename GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(dbmod::table_column_names(conn, "customers").unwrap()[0], "customer_key");
            assert_eq!(dbmod::browse_table(conn, "customers", &[], None, 500, 0).unwrap().rows, original);
            demo.state.open_modify_table_wizard().unwrap();
            assert_eq!(demo.state.wizard_col_sources[0].as_deref(), Some("customer_key"));
        }
    }

    #[test]
    fn modify_table_controls_save_custom_types_constraints_and_reordering_atomically() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            dbmod::seed_demo(demo.state.conn.as_ref().unwrap()).unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "customers" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000 + row as u64);
            demo.state.open_modify_table_wizard().unwrap();
            demo.state.wizard_col_sel = 3;
            demo.do_rebuild();
            demo.ui.set_autocomplete_value(demo.ids.ov_col_type, "VARYING CHARACTER(48)");
            demo.ui.set_checked(demo.ids.ov_nn, true);
            demo.ui.set_checked(demo.ids.ov_unique, true);
            demo.ui.set_text_input(demo.ids.ov_default, "'Unknown city'");
            for _ in 0..3 {
                demo.handle_button(demo.ids.ov_btn_up);
                demo.do_rebuild();
            }
            assert_eq!(demo.state.wizard_col_sources[0].as_deref(), Some("city"));
            assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type), Some("VARYING CHARACTER(48)"));
            for phase in ["draft", "saved"] {
                if phase == "saved" {
                    demo.ui.set_text_input(demo.ids.ov_default, "0); DELETE FROM customers; --");
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_some());
                    assert!(demo.state.overlay.is_some());
                    assert_eq!(dbmod::table_column_names(demo.state.conn.as_ref().unwrap(), "customers").unwrap()[0], "id");
                    assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "customers").unwrap(), 3);
                    demo.do_rebuild();
                    assert_eq!(demo.ui.text_input_value(demo.ids.ov_default), Some("0); DELETE FROM customers; --"));
                    demo.ui.set_text_input(demo.ids.ov_default, "'Unknown city'");
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                    demo.state.tab = MainTab::Browse;
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_REBUILD_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Table rebuild {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Table rebuild GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            let columns = dbmod::table_column_defs_in(conn, "main", "customers").unwrap();
            assert_eq!(columns[0].name, "city");
            assert_eq!(columns[0].type_name, "VARYING CHARACTER (48)");
            assert!(columns[0].notnull && columns[0].unique);
            assert_eq!(columns[0].default_value, "'Unknown city'");
            assert_eq!(conn.query_row("SELECT name FROM customers WHERE id=1", [], |row| row.get::<_,String>(0)).unwrap(), "Alice");
            assert!(conn.execute("INSERT INTO customers(name,city) VALUES('duplicate','Shanghai')", []).is_err());
            assert!(conn.execute("INSERT INTO customers(name,city) VALUES('null',NULL)", []).is_err());
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_unique), Some(true));
            demo.ui.set_checked(demo.ids.ov_unique, false);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO customers(name,city) VALUES('duplicate','Shanghai')", []).unwrap();
        }
    }

    #[test]
    fn foreign_key_page_preserves_sql_sessions_validates_saves_and_follows_local_and_self_identities() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parent(p TEXT,q INTEGER,PRIMARY KEY(p,q)); INSERT INTO parent VALUES('first',1),('second',2); CREATE TABLE lookup(q INTEGER PRIMARY KEY); INSERT INTO lookup VALUES(1),(2); CREATE TABLE child(id INTEGER PRIMARY KEY,p TEXT,q INTEGER,previous INTEGER,CONSTRAINT pair FOREIGN KEY(p,q) REFERENCES parent(p,q),FOREIGN KEY(previous) REFERENCES child(id) DEFERRABLE INITIALLY DEFERRED,CHECK(q>0)); INSERT INTO child VALUES(7,'first',1,NULL),(11,'second',2,7);").unwrap();
            demo.state.refresh_schema().unwrap();
            let row=demo.state.schema.iter().position(|object|object.name=="child" && object.kind==ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_page=2; demo.do_rebuild();
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_fk_clause).as_deref(),Some("Foreign key REFERENCES clause"));
            let original=demo.ui.text_area_value(demo.ids.ov_fk_clause).unwrap().to_string();
            let clause="REFERENCES parent(p,q)\nON UPDATE CASCADE ON DELETE SET NULL\nDEFERRABLE INITIALLY DEFERRED";
            demo.ui.set_text_input(demo.ids.ov_fk_name,"pair.\"名");
            let area=demo.ids.ov_fk_clause; demo.ui.set_focus(Some(area)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text(clause); demo.snapshot_inputs();
            demo.ui.set_scroll_offset(demo.ids.ov_fks_scroll,0.0,10000.0); demo.ui.select_text_area_range(area,0..0);
            demo.ui.ime_preedit("中文",Some((3,6))); demo.ui.sync(&mut demo.scene);
            let candidate=demo.ui.ime_cursor_area(&demo.scene).expect("FK SQL field is visible"); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_fk_clause)); assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(candidate));
            assert!(demo.ui.ime_commit("-- 中文\n")); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            let columns=demo.ids.ov_fk_columns; demo.ui.set_focus(Some(columns)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text("\"p\", \"q\""); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fks_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_fk_clause,"REFERENCES child(id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED"); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fks_table,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_fk_clause)); demo.ui.key_input(UiKey::Undo); assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),Some(clause));
            demo.ui.set_focus(Some(demo.ids.ov_fk_columns)); demo.ui.key_input(UiKey::Undo); assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_columns),Some("p, q"));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
            demo.handle_button(demo.ids.ov_fk_down); demo.do_rebuild(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),Some(clause));
            demo.handle_button(demo.ids.ov_fk_up); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_fk_add); demo.do_rebuild(); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_some()); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_fk_clause,"REFERENCES missing(id)"); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some()); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),Some("REFERENCES missing(id)")); assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"child").unwrap(),2);
            demo.handle_button(demo.ids.ov_fk_del); demo.do_rebuild(); demo.handle_button(demo.ids.ov_fk_add); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_fk_columns,"q"); demo.ui.set_text_area_value(demo.ids.ov_fk_clause,"REFERENCES lookup(q)"); demo.snapshot_inputs();
            demo.ui.set_focus(Some(demo.ids.ov_table_tabs)); demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,0); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_input(demo.ids.ov_col_name,"parent_key"); demo.ui.set_text_input(demo.ids.ov_name,"renamed.名"); demo.snapshot_inputs();
            assert!(demo.state.wizard_foreign_keys[0].columns.contains("parent_key")); assert!(demo.state.wizard_foreign_keys[0].clause.contains("parent (p, q)"));
            assert!(demo.state.wizard_foreign_keys[1].clause.contains("\"renamed.名\""));
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,2); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_table_selected_row(demo.ids.ov_fks_table,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            for phase in ["draft","saved"] {
                if phase=="saved" { demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg); demo.do_rebuild(); }
                else { demo.ui.set_scroll_offset(demo.ids.ov_fks_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene); }
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_FK_QA_OUT") {
                    demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("Table FK {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Table FK GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn=demo.state.conn.as_ref().unwrap(); conn.execute("UPDATE parent SET p='changed' WHERE p='first'",[]).unwrap();
            assert_eq!(conn.query_row("SELECT parent_key FROM \"renamed.名\" WHERE id=7",[],|row|row.get::<_,String>(0)).unwrap(),"changed");
            assert!(conn.execute("UPDATE \"renamed.名\" SET q=0 WHERE id=7",[]).is_err());
            demo.state.open_modify_table_wizard().unwrap(); assert_eq!(demo.state.wizard_foreign_keys.len(),3);
            demo.state.wizard_table_page=2; demo.win_w=800; demo.win_h=600; demo.do_rebuild();
            demo.ui.set_scroll_offset(demo.ids.ov_fks_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene);
            let remembered=demo.ui.scroll_offset(demo.ids.ov_fks_scroll).unwrap(); assert!(remembered.1>0.0);
            demo.snapshot_inputs(); demo.ui.set_focus(Some(demo.ids.ov_table_tabs)); demo.do_rebuild();
            assert_eq!(demo.ui.scroll_offset(demo.ids.ov_fks_scroll),Some(remembered));
            let bounds=|id| demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
            let field=bounds(demo.ids.ov_fk_clause); let viewport=bounds(demo.ids.ov_fks_scroll); let preview=bounds(demo.ids.ov_table_preview); let save=bounds(demo.ids.ov_btn_ok);
            assert!(field.y+0.001>=viewport.y && field.y+field.height<=viewport.y+viewport.height+0.001 && viewport.y+viewport.height<=preview.y && preview.y+preview.height<=save.y,"{field:?}/{viewport:?}/{preview:?}/{save:?}");
            demo.handle_button(demo.ids.ov_btn_cancel); demo.state.open_create_table_wizard(); demo.state.wizard_table_page=2; demo.do_rebuild();
            assert!(demo.state.wizard_foreign_keys.is_empty()); demo.handle_button(demo.ids.ov_fk_add); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_fk_clause,"REFERENCES lookup(q)"); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap(); conn.execute("INSERT INTO new_table VALUES(1,'valid')",[]).unwrap(); assert!(conn.execute("INSERT INTO new_table VALUES(9,'invalid')",[]).is_err());
        }
    }

    #[test]
    fn edited_table_foreign_key_previews_and_saves_resolve_direct_local_swaps_and_self_targets() {
        for schema in ["main","temp","aux.\"db"] {
            let mut state=state::AppState::new_with_demo(); state.new_memory_db().unwrap();
            let conn=state.conn.as_ref().unwrap();
            conn.execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {}(p TEXT,q INTEGER,PRIMARY KEY(p,q)); INSERT INTO {} VALUES('kept',2); CREATE TABLE {}(id INTEGER PRIMARY KEY,p TEXT,q INTEGER,previous INTEGER,FOREIGN KEY(p,q) REFERENCES parent(p,q),FOREIGN KEY(previous) REFERENCES child(id) DEFERRABLE INITIALLY DEFERRED); INSERT INTO {} VALUES(7,'kept',2,NULL);",dbmod::qualified_name(schema,"parent"),dbmod::qualified_name(schema,"parent"),dbmod::qualified_name(schema,"child"),dbmod::qualified_name(schema,"child"))).unwrap();
            state.refresh_schema().unwrap(); let row=state.schema.iter().position(|object|object.schema==schema && object.name=="child").unwrap();
            state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
            state.wizard_table_options.foreign_keys=Some(state.wizard_foreign_keys.clone());
            state.wizard_cols[1].name="q".into(); state.wizard_cols[2].name="p".into(); state.wizard_table_name="swapped".into();
            let preview=state.table_wizard_sql().unwrap(); state.apply_table_wizard().unwrap();
            let conn=state.conn.as_ref().unwrap();
            let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='swapped'",dbmod::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::table_foreign_keys(&preview).unwrap(),table_sql::table_foreign_keys(&saved).unwrap());
            let keys=table_sql::table_foreign_keys(&saved).unwrap(); assert_eq!(keys[0].columns,"\"q\", \"p\""); assert!(keys[0].clause.contains("parent (p, q)")); assert!(keys[1].clause.contains("\"swapped\""));
            assert_eq!(conn.query_row(&format!("SELECT q,p FROM {}",dbmod::qualified_name(schema,"swapped")),[],|row|Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?))).unwrap(),("kept".into(),2));
            assert!(conn.execute(&format!("UPDATE {} SET q='missing'",dbmod::qualified_name(schema,"swapped")),[]).is_err());
        }
    }

    #[test]
    fn table_check_reference_names_follow_swaps_reorders_and_reject_invalid_sql_renames() {
        let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(a INTEGER,b INTEGER,col TEXT,CHECK(a<b)); INSERT INTO records VALUES(2,6,'kept'); COMMIT; BEGIN; INSERT INTO records VALUES(3,8,'pending');").unwrap();
        demo.state.refresh_schema().unwrap();
        let row=demo.state.schema.iter().position(|object|object.name=="records" && object.kind==ObjectKind::Table).unwrap();
        demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.do_rebuild();
        demo.ui.set_text_area_value(demo.ids.ov_check_expr,"records.a + 1 < b AND 'a' != 'b'"); demo.snapshot_inputs();
        demo.ui.set_text_input(demo.ids.ov_col_name,"b"); demo.snapshot_inputs();
        assert!(!demo.state.table_wizard_names_valid());
        assert!(demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().contains("Duplicate column name"));
        demo.ui.set_focus(Some(demo.ids.ov_check_expr)); assert!(!demo.ui.ime_commit("changed"));
        demo.ui.set_text_input(demo.ids.ov_col_name,""); demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_constraint_columns,["a","b","col"]);
        demo.ui.set_text_input(demo.ids.ov_col_name,"b"); demo.snapshot_inputs();
        demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_col_name,"a"); demo.snapshot_inputs();
        let expression=demo.ui.text_area_value(demo.ids.ov_check_expr).unwrap().to_string();
        assert!(expression.contains("\"records\".\"b\"") && expression.contains("< \"a\"") && expression.contains("'a' <> 'b'"),"{expression}");
        demo.ui.set_text_input(demo.ids.ov_name,"changed.\"名"); demo.snapshot_inputs();
        assert!(demo.state.wizard_checks[0].expression.contains("\"changed.\"\"名\".\"b\""));
        let valid=demo.state.wizard_checks[0].expression.clone();
        demo.ui.set_text_area_value(demo.ids.ov_check_expr,"b +"); demo.snapshot_inputs();
        demo.ui.set_text_input(demo.ids.ov_col_name,"rejected"); demo.snapshot_inputs();
        assert_eq!(demo.ui.text_input_value(demo.ids.ov_col_name),Some("a"));
        assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some("b +"));
        assert!(demo.state.error_msg.as_deref().unwrap().contains("Fix the constraint SQL before renaming"));
        demo.ui.set_text_area_value(demo.ids.ov_check_expr,valid); demo.snapshot_inputs(); demo.clear_error();
        demo.ui.set_text_input(demo.ids.ov_col_name,"a2"); demo.snapshot_inputs();
        assert!(demo.state.wizard_checks[0].expression.contains("< \"a2\""));
        demo.ui.set_text_input(demo.ids.ov_col_name,"a"); demo.snapshot_inputs();
        demo.handle_button(demo.ids.ov_btn_add); demo.do_rebuild();
        assert_eq!(demo.ui.text_input_value(demo.ids.ov_col_name),Some("col_2"));
        demo.handle_button(demo.ids.ov_btn_up); demo.do_rebuild();
        assert_eq!(demo.state.wizard_constraint_columns,demo.state.wizard_cols.iter().map(|column|column.name.clone()).collect::<Vec<_>>());
        remove_selected_field_or_index(&mut demo); demo.do_rebuild();
        for phase in ["draft","saved"] {
            if phase=="saved" { demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg); demo.do_rebuild(); }
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_CHECK_RENAME_QA_OUT") {
                for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
                    demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
                    demo.state.wizard_table_page=3; demo.do_rebuild();
                    demo.ui.set_scroll_offset(demo.ids.ov_checks_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("CHECK rename {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("CHECK rename GPU comparison not_run: adapter unavailable"); }
                }
            }
        }
        let conn=demo.state.conn.as_ref().unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM \"changed.\"\"名\" WHERE b+1<a",[],|row|row.get::<_,i64>(0)).unwrap(),2);
        assert!(conn.execute("UPDATE \"changed.\"\"名\" SET b=99",[]).is_err());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM records WHERE a=2 AND b=6",[],|row|row.get::<_,i64>(0)).unwrap(),1);
    }

    #[test]
    fn qualified_check_previews_match_native_rename_and_both_rebuild_paths() {
        for schema in ["main","temp","aux.\"db"] {
            for mode in ["native","table","column"] {
                let mut state=state::AppState::new_with_demo(); state.new_memory_db().unwrap();
                let conn=state.conn.as_ref().unwrap();
                conn.execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {}(v INTEGER CHECK(records.v>0),CHECK(records.v<10)); INSERT INTO {} VALUES(2);",dbmod::qualified_name(schema,"records"),dbmod::qualified_name(schema,"records"))).unwrap();
                state.refresh_schema().unwrap();
                let row=state.schema.iter().position(|object|object.schema==schema && object.name=="records").unwrap();
                state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
                state.wizard_cols[0].name="amount".into(); state.wizard_table_name="changed.名".into();
                if mode=="table" { state.store_wizard_checks(); }
                if mode=="column" { state.wizard_cols[0].check_expression="\"changed.名\".amount>1".into(); }
                let preview=state.table_wizard_sql().unwrap();
                state.apply_table_wizard().unwrap();
                let conn=state.conn.as_ref().unwrap();
                let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='changed.名'",dbmod::quote_ident(schema)),[],|row|row.get(0)).unwrap();
                assert_eq!(table_sql::table_checks(&preview).unwrap(),table_sql::table_checks(&saved).unwrap(),"{schema}/{mode}");
                let mut preview_columns=vec![dbmod::ColDef::new("amount","INTEGER")];
                table_sql::read_column_constraints(&preview,&mut preview_columns).unwrap();
                assert_eq!(preview_columns[0].check_expression,dbmod::table_column_defs_in(conn,schema,"changed.名").unwrap()[0].check_expression,"{schema}/{mode}");
                let target=dbmod::qualified_name(schema,"changed.名");
                assert_eq!(conn.query_row(&format!("SELECT amount FROM {target}"),[],|row|row.get::<_,i64>(0)).unwrap(),2);
                assert!(conn.execute(&format!("UPDATE {target} SET amount=0"),[]).is_err());
                assert!(conn.execute(&format!("UPDATE {target} SET amount=10"),[]).is_err());
                assert_eq!(table_sql::table_checks(&saved).unwrap()[0].expression.contains("\"changed.名\"."),mode=="native");
            }
        }
    }

    #[test]
    fn edited_table_check_preview_and_save_resolve_direct_qualified_identity_changes() {
        for schema in ["main","temp","aux.\"db"] {
            let mut state=state::AppState::new_with_demo(); state.new_memory_db().unwrap();
            let conn=state.conn.as_ref().unwrap();
            conn.execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {}(a INTEGER,b INTEGER,CHECK(a<b)); INSERT INTO {} VALUES(2,6);",dbmod::qualified_name(schema,"records"),dbmod::qualified_name(schema,"records"))).unwrap();
            state.refresh_schema().unwrap();
            let row=state.schema.iter().position(|object|object.schema==schema && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
            state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
            state.wizard_checks[0].expression="a+1<b".into(); state.store_wizard_checks();
            state.wizard_cols[0].name="b".into(); state.wizard_cols[1].name="a".into(); state.wizard_table_name="swapped".into();
            let preview=state.table_wizard_sql().unwrap();
            state.apply_table_wizard().unwrap();
            let conn=state.conn.as_ref().unwrap();
            let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='swapped'",dbmod::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::table_checks(&preview).unwrap(),table_sql::table_checks(&saved).unwrap());
            assert_eq!(conn.query_row(&format!("SELECT b,a FROM {}",dbmod::qualified_name(schema,"swapped")),[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))).unwrap(),(2,6));
            assert!(conn.execute(&format!("UPDATE {} SET b=99",dbmod::qualified_name(schema,"swapped")),[]).is_err());
        }
    }

    #[test]
    fn table_check_page_retains_independent_sessions_and_validates_actual_save_and_create() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(v INTEGER CONSTRAINT positive CHECK(v>0),note TEXT,CONSTRAINT limit_value CHECK(v<10),CHECK(length(note)>0)); INSERT INTO records VALUES(2,'first'),(4,'second');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row=demo.state.schema.iter().position(|object|object.name=="records" && object.kind==ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_page=3; demo.do_rebuild();
            assert_eq!(demo.ui.tabs_selected(demo.ids.ov_table_tabs),Some(3));
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_check_expr).as_deref(),Some("CHECK expression"));
            assert_eq!(demo.ui.text_input_value(demo.ids.ov_check_name),Some("limit_value"));
            let original=demo.ui.text_area_value(demo.ids.ov_check_expr).unwrap().to_string();
            let expression="v < 20\nAND v != 11";
            demo.ui.set_text_input(demo.ids.ov_check_name,"range.\"名");
            let area=demo.ids.ov_check_expr; demo.ui.set_focus(Some(area)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text(expression); demo.snapshot_inputs();
            demo.ui.set_scroll_offset(demo.ids.ov_checks_scroll,0.0,10000.0); demo.ui.select_text_area_range(area,0..0);
            demo.ui.ime_preedit("中文",Some((3,6))); demo.ui.sync(&mut demo.scene);
            let candidate=demo.ui.ime_cursor_area(&demo.scene).expect("table CHECK editor is visible");
            demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_check_expr)); assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(candidate));
            assert!(demo.ui.ime_commit("-- 中文\n")); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_checks_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_check_expr,"note IS NOT NULL"); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_checks_table,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_check_expr)); demo.ui.key_input(UiKey::Undo);
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some(expression));
            demo.handle_button(demo.ids.ov_check_down); demo.do_rebuild(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some(expression));
            demo.handle_button(demo.ids.ov_check_up); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_check_add); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some(""));
            demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_some()); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_check_expr,"v < 1"); demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some()); assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),2);
            demo.do_rebuild(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some("v < 1"));
            demo.handle_button(demo.ids.ov_check_del); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_check_add); demo.do_rebuild(); demo.ui.set_text_area_value(demo.ids.ov_check_expr,"v != 13"); demo.snapshot_inputs();
            demo.ui.set_focus(Some(demo.ids.ov_table_tabs)); demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,0); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.wizard_table_page,0);
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,3); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some("v != 13"));
            for phase in ["draft","saved"] {
                if phase=="saved" { demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg); demo.do_rebuild(); }
                else { demo.ui.set_scroll_offset(demo.ids.ov_checks_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene); }
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_CHECK_QA_OUT") {
                    demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("Table checks {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Table checks GPU comparison not_run: adapter unavailable"); }
                }
            }
            assert!(demo.state.conn.as_ref().unwrap().execute("UPDATE records SET v=13",[]).is_err());
            assert!(demo.state.conn.as_ref().unwrap().execute("UPDATE records SET v=0",[]).is_err());
            demo.state.open_modify_table_wizard().unwrap(); assert_eq!(demo.state.wizard_checks.len(),4);
            demo.state.wizard_table_page=3; demo.win_w=800; demo.win_h=600; demo.do_rebuild();
            demo.ui.set_scroll_offset(demo.ids.ov_checks_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene);
            let remembered=demo.ui.scroll_offset(demo.ids.ov_checks_scroll).unwrap(); assert!(remembered.1>0.0);
            demo.snapshot_inputs(); demo.ui.set_focus(Some(demo.ids.ov_table_tabs)); demo.do_rebuild();
            assert_eq!(demo.ui.scroll_offset(demo.ids.ov_checks_scroll),Some(remembered));
            let bounds=|id| demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
            let field=bounds(demo.ids.ov_check_expr); let viewport=bounds(demo.ids.ov_checks_scroll); let preview=bounds(demo.ids.ov_table_preview); let save=bounds(demo.ids.ov_btn_ok);
            assert!(field.y+0.001>=viewport.y && field.y+field.height<=viewport.y+viewport.height+0.001 && viewport.y+viewport.height<=preview.y && preview.y+preview.height<=save.y,"{field:?} / {viewport:?} / {preview:?} / {save:?}");
            demo.handle_button(demo.ids.ov_btn_cancel);
            demo.state.open_create_table_wizard(); demo.state.wizard_table_page=3; demo.do_rebuild();
            assert!(demo.state.wizard_checks.is_empty()); demo.handle_button(demo.ids.ov_check_add); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_check_expr,"id > 0"); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(-1,'invalid')",[]).is_err());
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(1,'valid')",[]).unwrap();
        }
    }

    #[test]
    fn table_sql_preview_updates_live_keeps_read_only_sessions_and_fits_bounded_windows() {
        let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(qty INTEGER DEFAULT 2,total INTEGER CONSTRAINT generation AS(qty*4) STORED,note TEXT CONSTRAINT note_check CHECK(length(note)>0)); INSERT INTO records(qty,note) VALUES(3,'kept');").unwrap();
        demo.state.refresh_schema().unwrap();
        let row=demo.state.schema.iter().position(|object|object.name=="records" && object.kind==ObjectKind::Table).unwrap();
        demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
        let area=demo.ids.ov_table_preview;
        assert_eq!(demo.ui.accessible_name(area).as_deref(),Some("Generated CREATE TABLE SQL preview"));
        demo.ui.set_text_input(demo.ids.ov_col_name,"amount"); demo.ui.set_text_input(demo.ids.ov_default,"4");
        demo.ui.set_text_input(demo.ids.ov_name,"prepared"); demo.ui.set_checked(demo.ids.ov_nn,true); demo.ui.set_checked(demo.ids.ov_strict,true);
        demo.snapshot_inputs();
        let preview=demo.ui.text_area_value(area).unwrap().to_string();
        assert!(preview.contains("\"prepared\"") && preview.contains("\"amount\"") && preview.contains("note_check") && preview.contains("NOT NULL") && preview.contains("STRICT"),"{preview}");
        demo.ui.set_focus(Some(area)); demo.ui.select_text_area_range(area,0..26);
        let selected=demo.ui.focused_text_for_copy().unwrap();
        demo.ui.key_input(UiKey::Char('x')); assert!(!demo.ui.ime_commit("changed"));
        assert_eq!(demo.ui.text_area_value(area),Some(preview.as_str()));
        demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.ui.focused(),Some(demo.ids.ov_table_preview)); assert_eq!(demo.ui.focused_text_for_copy(),Some(selected.clone()));
        demo.ui.set_text_input(demo.ids.ov_col_name,"total"); demo.snapshot_inputs();
        assert!(demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().contains("Duplicate column name"));
        demo.ui.set_text_input(demo.ids.ov_col_name,"amount"); demo.snapshot_inputs();
        assert_eq!(demo.ui.text_area_value(demo.ids.ov_table_preview),Some(preview.as_str()));
        demo.ui.set_focus(Some(demo.ids.ov_table_preview)); demo.ui.select_text_area_range(demo.ids.ov_table_preview,0..26); demo.snapshot_inputs();
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width; demo.win_h=height; demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size); demo.do_rebuild();
                assert_eq!(demo.ui.focused_text_for_copy(),Some(selected.clone()));
                let bounds=|demo:&Demo,id| demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
                let field=bounds(&demo,demo.ids.ov_table_preview); let save=bounds(&demo,demo.ids.ov_btn_ok); let viewport=bounds(&demo,demo.ids.ov_scroll);
                assert!(viewport.y+viewport.height<=field.y && field.y+field.height<=save.y && save.y+save.height<=height as f32,"{width}: {viewport:?}, {field:?}, {save:?}");
                demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene);
                let last=bounds(&demo,demo.ids.ov_generated);
                assert!(last.y>=viewport.y && last.y+last.height<=viewport.y+viewport.height,"{width}: {last:?}, {viewport:?}");
                assert_eq!(bounds(&demo,demo.ids.ov_table_preview),field);
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_PREVIEW_QA_OUT") {
                    demo.scene.mark_full_dirty(width as f32,height as f32); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(width,height,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{width}-metal.ppm"),b,width,height).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{width}: {delta}"); eprintln!("Table preview {theme_name}/{width}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Table preview GPU comparison not_run: adapter unavailable"); }
                }
                demo.snapshot_inputs();
            }
        }
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT qty FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),3);
        demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
        assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT total FROM prepared WHERE amount=3",[],|row|row.get::<_,i64>(0)).unwrap(),12);
        demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
        assert_eq!(demo.ui.text_area_value(demo.ids.ov_table_preview),Some(preview.as_str()));
        demo.handle_button(demo.ids.ov_btn_cancel);
    }

    #[test]
    fn generated_column_controls_preserve_sessions_validate_save_and_disable_cell_writes() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(qty INTEGER,total INTEGER CONSTRAINT generation AS(qty*4) STORED,note TEXT); INSERT INTO records(qty,note) VALUES(3,'kept');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row=demo.state.schema.iter().position(|object|object.name=="records" && object.kind==ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap();
            demo.state.wizard_col_sel=1; demo.do_rebuild();
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_generation).as_deref(),Some("Column generation mode"));
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_generated).as_deref(),Some("Generated column expression"));
            assert_eq!(demo.ui.select_selected(demo.ids.ov_generation),Some(2));
            let original=demo.ui.text_area_value(demo.ids.ov_generated).unwrap().to_string();
            demo.ui.set_focus(Some(demo.ids.ov_generation)); demo.ui.set_select_selected(demo.ids.ov_generation,Some(1));
            demo.snapshot_inputs(); assert!(demo.rebuild); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_generation));
            let expression="qty * 5\n+ 1";
            demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene);
            let area=demo.ids.ov_generated;
            demo.ui.set_focus(Some(area)); demo.ui.key_input(UiKey::SelectAll); demo.ui.paste_into_focused_text(expression); demo.snapshot_inputs();
            demo.ui.select_text_area_range(area,0..0); demo.ui.ime_preedit("中文",Some((3,6))); demo.ui.sync(&mut demo.scene);
            let candidate=demo.ui.ime_cursor_area(&demo.scene).expect("generated expression is visible");
            demo.do_rebuild(); assert_eq!(demo.ui.focused(),Some(demo.ids.ov_generated));
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(candidate));
            assert!(demo.ui.ime_commit("-- 中文\n")); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_generated),Some(expression));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(2)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_generation),Some(0));
            demo.ui.set_focus(Some(demo.ids.ov_generated)); demo.ui.key_input(UiKey::Char('x'));
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_generated),Some(""));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_generated)); demo.ui.key_input(UiKey::Undo);
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_generated),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
            demo.handle_button(demo.ids.ov_btn_up); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_generated),Some(expression));
            demo.handle_button(demo.ids.ov_btn_down); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_btn_add); demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_generation),Some(0));
            remove_selected_field_or_index(&mut demo); demo.do_rebuild();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_generated,"random()"); demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some());
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT total FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),12);
            demo.do_rebuild(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_generated),Some("random()"));
            demo.ui.set_text_area_value(demo.ids.ov_generated,expression); demo.snapshot_inputs();
            for phase in ["draft","saved"] {
                if phase=="saved" {
                    demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                    demo.state.tab=MainTab::Browse; demo.state.goto_browse_record(1).unwrap();
                    demo.state.browse_mut().sel_col=Some(1); demo.state.sync_edit_cell_from_selection(); demo.do_rebuild();
                } else { demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,10000.0); }
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_GENERATED_QA_OUT") {
                    for area in [demo.ids.ov_check,demo.ids.ov_foreign_key,demo.ids.ov_generated] { sql_syntax::apply(&mut demo.ui,area,&demo.theme); }
                    demo.ui.sync(&mut demo.scene); demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty();
                    let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("Generated {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Generated GPU comparison not_run: adapter unavailable"); }
                }
            }
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT total FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),16);
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_col_sel=1; demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_generation),Some(1));
            demo.ui.set_select_selected(demo.ids.ov_generation,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_generated),Some(""));
            demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            demo.state.conn.as_ref().unwrap().execute("UPDATE records SET qty=4",[]).unwrap();
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT total FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),16);
            demo.state.open_create_table_wizard(); demo.state.wizard_col_sel=1; demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_generation),Some(0));
            demo.ui.set_select_selected(demo.ids.ov_generation,Some(2)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_generated,"id * 2"); demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table(id) VALUES(4)",[]).unwrap();
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT name FROM new_table",[],|row|row.get::<_,String>(0)).unwrap(),"8");
            demo.state.switch_browse_table("new_table").unwrap(); demo.state.tab=MainTab::Browse;
            demo.state.dock_edit=true; demo.state.dock_focus=DockKind::EditCell;
            demo.state.goto_browse_record(1).unwrap(); demo.state.browse_mut().sel_col=Some(1); demo.state.sync_edit_cell_from_selection(); demo.do_rebuild();
            assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(false));
            demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Char('x')); assert!(!demo.ui.ime_commit("changed"));
            assert_eq!(demo.ui.text_area_value(demo.ids.edit_value),Some("8"));
            demo.state.browse_mut().sel_col=Some(0); demo.state.sync_edit_cell_from_selection(); demo.do_rebuild();
            assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell),Some(true));
        }
    }

    #[test]
    fn table_preview_splitter_drags_live_preserves_sessions_and_fits_small_windows() {
        let bounds=|demo:&Demo,id|demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height) in [(1400,1000),(800,600)] {
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
                let extras=(0..20).map(|index|format!("c{index} TEXT")).collect::<Vec<_>>().join(",");
                demo.state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE records(id INTEGER PRIMARY KEY,b TEXT CHECK(length(b)>0),{extras}); INSERT INTO records(id,b) VALUES(7,'kept'); CREATE VIEW visible AS SELECT b FROM records;")).unwrap();demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.state.wizard_col_sel=1;demo.do_rebuild();
                demo.ui.set_text_input(demo.ids.ov_col_name,"label");demo.snapshot_inputs();demo.do_rebuild();assert_eq!(demo.state.wizard_cols[1].name,"label");
                for advanced in [false,true] {
                    demo.state.wizard_table_ratio=0.72;demo.state.wizard_table_advanced=advanced;demo.do_rebuild();assert!(demo.ui.is_splitter(demo.ids.ov_table_splitter));
                    demo.ui.select_text_area_range(demo.ids.ov_table_preview,0..6);demo.ui.set_focus(Some(demo.ids.ov_table_preview));demo.snapshot_inputs();demo.ui.sync(&mut demo.scene);
                    let tabs=bounds(&demo,demo.ids.ov_table_tabs);let preview=bounds(&demo,demo.ids.ov_table_preview);let save=bounds(&demo,demo.ids.ov_btn_ok);
                    assert!(tabs.height>50.0 && preview.height>24.0);assert!(tabs.y+tabs.height<=preview.y && preview.y+preview.height<=save.y && save.y+save.height<=height as f32);
                    let (x,y)=(tabs.x+tabs.width*0.5,tabs.y+tabs.height+3.0);assert_eq!(demo.ui.hit_widget(&demo.scene,x,y),Some(demo.ids.ov_table_splitter),"{theme_name}/{width}/advanced={advanced}: tabs={tabs:?},preview={preview:?},hit={:?}",demo.ui.hit_widget(&demo.scene,x,y).and_then(|id|demo.ui.accessible_name(id)));
                    let ratio=demo.ui.splitter_ratio(demo.ids.ov_table_splitter).unwrap();assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));demo.snapshot_inputs();assert!(!demo.rebuild);
                    let delta=(tabs.height*0.08).clamp(5.0,24.0);
                    for step in [1.0,2.0] {assert!(demo.ui.pointer_moved(x,y-delta*step));demo.ui.sync(&mut demo.scene);assert!(demo.ui.is_dragging());}
                    let smaller=bounds(&demo,demo.ids.ov_table_tabs);let larger=bounds(&demo,demo.ids.ov_table_preview);assert!(smaller.height<tabs.height && larger.height>preview.height);
                    assert!(demo.ui.splitter_ratio(demo.ids.ov_table_splitter).unwrap()<ratio);demo.ui.pointer_released();demo.snapshot_inputs();let ratio=demo.state.wizard_table_ratio;demo.do_rebuild();
                    assert_eq!(demo.ui.splitter_ratio(demo.ids.ov_table_splitter),Some(ratio));assert_eq!(demo.ui.text_area_selected_text(demo.ids.ov_table_preview).as_deref(),Some("CREATE"));assert!(!demo.ui.paste_into_focused_text("DELETE FROM records"));
                    for page in [1,2,3,0] {demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,page);demo.snapshot_inputs();demo.do_rebuild();assert_eq!(demo.ui.splitter_ratio(demo.ids.ov_table_splitter),Some(ratio));}
                    demo.win_w=if width==800 {1400} else {800};demo.win_h=if width==800 {1000} else {600};demo.do_rebuild();assert_eq!(demo.ui.splitter_ratio(demo.ids.ov_table_splitter),Some(ratio));
                    demo.win_w=width;demo.win_h=height;demo.do_rebuild();demo.ui.sync(&mut demo.scene);
                    let tabs=bounds(&demo,demo.ids.ov_table_tabs);let preview=bounds(&demo,demo.ids.ov_table_preview);let save=bounds(&demo,demo.ids.ov_btn_ok);assert!(tabs.y+tabs.height<=preview.y && preview.y+preview.height<=save.y && save.y+save.height<=height as f32);
                    if advanced {if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_SPLITTER_QA_OUT") {
                        demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                        if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{width}: {delta}");eprintln!("Table splitter {theme_name}/{width}: Soft/Metal delta {delta}");}
                        else {eprintln!("Table splitter GPU comparison not_run: adapter unavailable");}
                    }}
                    for delta in [-10000.0,10000.0] {
                        let tabs=bounds(&demo,demo.ids.ov_table_tabs);let (x,y)=(tabs.x+tabs.width*0.5,tabs.y+tabs.height+3.0);
                        assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));assert!(demo.ui.pointer_moved(x,y+delta));demo.ui.sync(&mut demo.scene);
                        let tabs=bounds(&demo,demo.ids.ov_table_tabs);let preview=bounds(&demo,demo.ids.ov_table_preview);let span=tabs.height+preview.height;let scale=(span/124.0).min(1.0);
                        assert!(tabs.height+0.5>=76.0*scale && preview.height+0.5>=48.0*scale);assert!(tabs.y+tabs.height<=preview.y);
                        demo.ui.pointer_released();demo.snapshot_inputs();demo.do_rebuild();assert!(bounds(&demo,demo.ids.ov_table_preview).y>=bounds(&demo,demo.ids.ov_table_tabs).y+bounds(&demo,demo.ids.ov_table_tabs).height);
                    }
                }
                demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT rowid+id FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),14);assert_eq!(conn.query_row("SELECT label FROM visible",[],|row|row.get::<_,String>(0)).unwrap(),"kept");assert!(conn.execute("UPDATE records SET label=''",[]).is_err());
                demo.state.open_create_table_wizard();assert_eq!(demo.state.wizard_table_ratio,0.72);
            }
        }
    }

    #[test]
    fn constraint_grids_keep_drafts_focus_viewports_sql_copy_and_real_save() {
        let table_id=|demo:&Demo,page:usize|[demo.ids.ov_keys_table,demo.ids.ov_fks_table,demo.ids.ov_checks_table][page];
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            let mut sql="CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,value TEXT CONSTRAINT inline_value CHECK(length(value)>0),p INTEGER CONSTRAINT inline_parent REFERENCES parent(id),amount INTEGER DEFAULT 0".to_string();
            for index in 0..18 {sql.push_str(&format!(",CONSTRAINT key_{index} UNIQUE(value),CONSTRAINT parent_{index} FOREIGN KEY(p) REFERENCES parent(id) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED,CONSTRAINT check_{index} CHECK(amount >= 0)"));}
            sql.push_str("); INSERT INTO records VALUES(7,'alpha',1,3); CREATE VIEW visible AS SELECT id,value,amount FROM records; CREATE TABLE audit(value); CREATE TRIGGER track AFTER UPDATE OF value ON records BEGIN INSERT INTO audit VALUES(new.value); END;");
            demo.state.conn.as_ref().unwrap().execute_batch(&sql).unwrap();demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            for page in 0..3 {
                demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,page+1);demo.snapshot_inputs();demo.do_rebuild();let table=table_id(&demo,page);
                let count=if page==2 {3} else {4};assert_eq!(demo.ui.table_column_widths(table).unwrap().len(),count);
                let name=[demo.ids.ov_key_name,demo.ids.ov_fk_name,demo.ids.ov_check_name][page];demo.ui.set_text_input(name,format!("edited_name_{page}"));
                demo.ui.set_focus(Some(table));assert!(demo.ui.key_input(UiKey::Down));demo.snapshot_inputs();assert!(demo.rebuild);demo.do_rebuild();let table=table_id(&demo,page);
                assert_eq!(demo.ui.focused(),Some(table));assert_eq!(demo.ui.table_selected_row(table),Some(1));
                let name=match page {0=>demo.state.wizard_index_constraints[0].name.as_deref(),1=>demo.state.wizard_foreign_keys[0].name.as_deref(),_=>demo.state.wizard_checks[0].name.as_deref()};assert_eq!(name,Some(format!("edited_name_{page}").as_str()));
                demo.ui.set_table_selected_col(table,Some(count-1));let rows=match page {0=>ui_build::table_index_rows(&demo.state),1=>ui_build::table_foreign_key_rows(&demo.state),_=>ui_build::table_check_rows(&demo.state)};
                let copied=format!("\"{}\"",rows[1][count-1].replace('"',"\"\""));assert_eq!(demo.ui.focused_text_for_copy(),Some(copied));
                assert!(demo.ui.set_table_column_width(table,0,420.0));assert!(demo.ui.set_table_column_width(table,count-1,900.0));demo.ui.set_table_scroll_offset(table,65.0,52.0);demo.snapshot_inputs();demo.do_rebuild();let table=table_id(&demo,page);
                assert_eq!(demo.ui.table_column_widths(table).unwrap()[0],420.0);assert_eq!(demo.ui.table_selected_col(table),Some(count-1));assert_eq!(demo.ui.table_scroll_offset(table),Some((65.0,52.0)));
                demo.ui.set_table_scroll_offset(table,0.0,0.0);demo.ui.sync(&mut demo.scene);let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(table)).unwrap().bounds;let style=next_gui_ui::TableStyle::from_theme(&demo.theme);
                assert!(demo.ui.pointer_pressed(&mut demo.scene,bounds.x+30.0,bounds.y+style.header_height+style.row_height*2.5));demo.ui.pointer_released();demo.snapshot_inputs();demo.do_rebuild();let table=table_id(&demo,page);assert_eq!(demo.ui.table_selected_row(table),Some(2));assert_eq!(demo.ui.focused(),Some(table));
                demo.ui.set_table_selected_row(table,Some(16));demo.ui.reveal_table_selection(table);demo.snapshot_inputs();demo.do_rebuild();let table=table_id(&demo,page);assert_eq!(demo.ui.table_selected_row(table),Some(16));assert!(demo.ui.table_scroll_offset(table).unwrap().1>0.0);
                for (width,height) in [(1400,1000),(800,600)] {
                    demo.win_w=width;demo.win_h=height;demo.do_rebuild();let table=table_id(&demo,page);demo.ui.set_table_selected_col(table,Some(count-1));demo.ui.reveal_table_selection(table);demo.ui.sync(&mut demo.scene);
                    let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(table)).unwrap().bounds;assert!(bounds.width>500.0);assert!(demo.ui.table_scroll_offset(table).unwrap().0>0.0);
                    if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_CONSTRAINT_GRID_QA_OUT") {
                        if width==1400 || page==2 {
                            demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-{page}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                            if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{page}/{width}: {delta}");eprintln!("Constraint grid {theme_name}/{page}/{width}: Soft/Metal delta {delta}");}
                            else {eprintln!("Constraint grid GPU comparison not_run: adapter unavailable");}
                        }
                    }
                }
                demo.win_w=1400;demo.win_h=1000;demo.do_rebuild();
            }
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);let conn=demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.query_row("SELECT rowid+id+amount FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),17);assert!(conn.execute("UPDATE records SET amount=-1",[]).is_err());assert!(conn.execute("INSERT INTO records(value,p) VALUES('alpha',1)",[]).is_err());
            conn.execute("UPDATE records SET value='beta'",[]).unwrap();assert_eq!(conn.query_row("SELECT value FROM visible",[],|row|row.get::<_,String>(0)).unwrap(),"beta");assert_eq!(conn.query_row("SELECT value FROM audit",[],|row|row.get::<_,String>(0)).unwrap(),"beta");
            conn.execute("INSERT INTO records(value,p) VALUES('gamma',1)",[]).unwrap();assert_eq!(conn.last_insert_rowid(),8);let sql=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='records'",[],|row|row.get::<_,String>(0)).unwrap();for page in 0..3 {assert!(sql.contains(&format!("edited_name_{page}")));}
            demo.state.open_create_table_wizard();assert!(demo.state.wizard_constraint_views.iter().all(|view|view.scroll==(0.0,0.0) && view.widths.is_empty() && view.column.is_none()));
        }
    }

    #[test]
    fn key_type_cells_validate_atomically_cancel_autoincrement_and_preserve_ordered_key_migrations() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,a TEXT,b INTEGER,g INTEGER GENERATED ALWAYS AS(id*2) STORED CONSTRAINT generated_unique UNIQUE,CONSTRAINT pair UNIQUE(b DESC,a COLLATE NOCASE ASC) ON CONFLICT FAIL); INSERT INTO records(id,a,b) VALUES(7,'West',1),(11,'East',2); CREATE VIEW visible AS SELECT id,a,b FROM records; CREATE TABLE audit(value); CREATE TRIGGER edited AFTER UPDATE OF a ON records BEGIN INSERT INTO audit VALUES(new.a); END;").unwrap();demo.state.refresh_schema().unwrap();let object=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+object as u64);demo.state.open_modify_table_wizard().unwrap();demo.state.wizard_table_page=1;demo.do_rebuild();
            let finish=|demo:&mut Demo| {demo.handle_table_cell_edit();if demo.rebuild {demo.do_rebuild();}demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
            let begin=|demo:&mut Demo,row| {demo.ui.set_table_selected_row(demo.ids.ov_keys_table,Some(row));demo.ui.set_table_selected_col(demo.ids.ov_keys_table,Some(1));demo.ui.set_focus(Some(demo.ids.ov_keys_table));demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.set_focus(Some(demo.ids.ov_keys_table));assert!(demo.ui.key_input(UiKey::Edit));finish(demo);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_constraint_editor));assert!(demo.ui.has_open_popup());};
            let column_flags=|demo:&Demo|demo.state.wizard_cols.iter().map(|column|(column.name.clone(),column.pk,column.auto_increment,column.unique,column.notnull)).collect::<Vec<_>>();
            let original=demo.state.wizard_index_constraints.clone();let columns=column_flags(&demo);let id=original.iter().position(|key|key.primary).unwrap();let pair=original.iter().position(|key|key.name.as_deref()==Some("pair")).unwrap();let generated=original.iter().position(|key|key.name.as_deref()==Some("generated_unique")).unwrap();
            begin(&mut demo,pair);demo.ui.key_input(UiKey::Up);finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints,original);assert_eq!(column_flags(&demo),columns);assert_eq!(demo.ui.select_selected(demo.ids.ov_constraint_editor),Some(1));assert!(demo.state.error_msg.as_deref().unwrap().contains("only one primary key"));assert!(demo.ui.text_content(demo.ids.status).unwrap().contains("only one primary key"));demo.clear_error();demo.ui.key_input(UiKey::Escape);finish(&mut demo);
            begin(&mut demo,id);demo.ui.key_input(UiKey::Down);finish(&mut demo);assert!(!demo.state.wizard_cols[0].pk && !demo.state.wizard_cols[0].auto_increment && demo.state.wizard_cols[0].unique);assert_eq!(demo.state.wizard_index_constraints[id].name,original[id].name);
            assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(demo.state.wizard_constraint_edit.is_some());assert_eq!(demo.ui.focused(),Some(demo.ids.ov_constraint_editor));assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints,original);assert_eq!(column_flags(&demo),columns);assert!(demo.state.wizard_cols[0].auto_increment);
            begin(&mut demo,id);demo.ui.key_input(UiKey::Down);finish(&mut demo);demo.ui.key_input(UiKey::Enter);finish(&mut demo);demo.ui.key_input(UiKey::Enter);finish(&mut demo);assert!(demo.state.wizard_constraint_edit.is_none());
            demo.state.wizard_table_options.without_rowid=true;demo.do_rebuild();demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.as_deref().unwrap().contains("PRIMARY KEY"));assert_eq!(demo.state.overlay,Some(OverlayKind::EditTable));assert!(demo.state.conn.as_ref().unwrap().query_row("SELECT sql FROM sqlite_schema WHERE name='records'",[],|row|row.get::<_,String>(0)).unwrap().contains("AUTOINCREMENT"));demo.state.wizard_table_options.without_rowid=false;demo.clear_error();demo.do_rebuild();
            let before=demo.state.wizard_index_constraints.clone();let flags=column_flags(&demo);begin(&mut demo,generated);demo.ui.key_input(UiKey::Up);finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints,before);assert_eq!(column_flags(&demo),flags);assert!(demo.state.error_msg.as_deref().unwrap().contains("Generated columns"));demo.clear_error();demo.ui.key_input(UiKey::Escape);finish(&mut demo);
            begin(&mut demo,pair);demo.ui.key_input(UiKey::Up);finish(&mut demo);let key=&demo.state.wizard_index_constraints[pair];assert!(key.primary);assert_eq!(key.columns,original[pair].columns);assert_eq!(key.name,original[pair].name);assert_eq!(key.conflict,original[pair].conflict);assert!(demo.state.wizard_cols[1].pk && demo.state.wizard_cols[2].pk);assert_eq!(demo.ui.select_selected(demo.ids.ov_key_conflict),Some(3));assert_eq!(demo.state.wizard_table_options.primary_key_conflict,Some(sqlite3_parser::ast::ResolveType::Fail));assert!(demo.state.error_msg.is_none());assert!(demo.ui.text_content(demo.ids.status).unwrap().contains("Constraint type: PRIMARY KEY (draft)"));assert!(demo.ui.text_content(demo.ids.status).unwrap().contains("SQLCipher"));
            demo.ui.key_input(UiKey::Enter);finish(&mut demo);assert!(demo.state.wizard_constraint_edit.is_some());demo.win_w=800;demo.win_h=600;demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.ov_constraint_editor));assert_eq!(demo.ui.select_selected(demo.ids.ov_constraint_editor),Some(0));
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_KEY_TYPE_QA_OUT") {
                demo.scene.mark_full_dirty(800.0,600.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(800,600,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-800.ppm"),soft.frame_pixels().unwrap().0,800,600).unwrap();
                if let Ok(mut gpu)=GpuRenderer::try_new(800,600,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}: {delta}");eprintln!("Key type {theme_name}/800: Soft/Metal delta {delta}");} else {eprintln!("Key type GPU comparison not_run: adapter unavailable");}
            }
            demo.ui.key_input(UiKey::Enter);finish(&mut demo);demo.win_w=1400;demo.win_h=1000;demo.do_rebuild();begin(&mut demo,pair);
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_KEY_TYPE_QA_OUT") {
                demo.scene.mark_full_dirty(1400.0,1000.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(1400,1000,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-1400.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}: {delta}");eprintln!("Key type {theme_name}/1400: Soft/Metal delta {delta}");} else {eprintln!("Key type GPU comparison not_run: adapter unavailable");}
            }
            demo.ui.key_input(UiKey::Enter);finish(&mut demo);assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);assert_eq!(demo.state.wizard_constraint_edit.as_ref().unwrap().column,2);assert_eq!(demo.ui.text_input_value(demo.ids.ov_constraint_editor),Some("pair"));demo.ui.key_input(UiKey::Escape);finish(&mut demo);
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT sum(rowid+id+b+g) FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),75);
            let sql=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='records'",[],|row|row.get::<_,String>(0)).unwrap();assert!(!sql.contains("AUTOINCREMENT"));for fragment in ["CONSTRAINT \"pair\" PRIMARY KEY","DESC","COLLATE NOCASE ASC","ON CONFLICT FAIL"] {assert!(sql.contains(fragment),"{sql}");}
            assert!(conn.execute("INSERT INTO records(id,a,b) VALUES(20,'west',1)",[]).is_err());conn.execute("UPDATE records SET a='North' WHERE id=7",[]).unwrap();assert_eq!(conn.query_row("SELECT a FROM visible WHERE id=7",[],|row|row.get::<_,String>(0)).unwrap(),"North");assert_eq!(conn.query_row("SELECT value FROM audit",[],|row|row.get::<_,String>(0)).unwrap(),"North");conn.execute("INSERT INTO records(id,a,b) VALUES(20,'South',3)",[]).unwrap();assert_eq!(conn.last_insert_rowid(),12);
        }
    }

    #[test]
    fn constraint_text_cells_keep_drafts_history_cancel_scope_and_database_semantics() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parents(p INTEGER,q INTEGER,PRIMARY KEY(p,q)); INSERT INTO parents VALUES(1,3); CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,value TEXT CONSTRAINT inline_check CHECK(length(value)>0),n INTEGER,p INTEGER,q INTEGER,CONSTRAINT ordered_unique UNIQUE(value COLLATE NOCASE ASC,n DESC) ON CONFLICT FAIL,CONSTRAINT parent_key FOREIGN KEY(p,q) REFERENCES parents(p,q) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED,CONSTRAINT positive CHECK(n>0)); INSERT INTO records VALUES(7,'Alpha',2,1,3); CREATE VIEW visible AS SELECT id,value FROM records; CREATE TABLE audit(value); CREATE TRIGGER edited AFTER UPDATE OF value ON records BEGIN INSERT INTO audit VALUES(new.value); END;").unwrap();
            demo.state.refresh_schema().unwrap();let object=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+object as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            let finish=|demo:&mut Demo| {demo.handle_table_cell_edit();if demo.rebuild {demo.do_rebuild();}demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
            let begin=|demo:&mut Demo,page,row,column,key| {
                demo.state.wizard_table_page=page+1;demo.do_rebuild();
                let table=[demo.ids.ov_keys_table,demo.ids.ov_fks_table,demo.ids.ov_checks_table][page];
                demo.ui.set_table_selected_row(table,Some(row));demo.ui.set_table_selected_col(table,Some(column));demo.ui.set_focus(Some(table));demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}
                let table=[demo.ids.ov_keys_table,demo.ids.ov_fks_table,demo.ids.ov_checks_table][page];demo.ui.set_focus(Some(table));assert!(demo.ui.key_input(key));finish(demo);
                assert_eq!(demo.ui.focused(),Some(if (page,column)==(1,2) {demo.ids.reference_editor.parent} else {demo.ids.ov_constraint_editor}));assert_eq!(demo.state.wizard_constraint_edit.as_ref().map(|edit|(edit.page,edit.row,edit.column)),Some((page,row,column)));
            };
            let unique=demo.state.wizard_index_constraints.iter().position(|key|!key.primary).unwrap();let original=demo.state.wizard_index_constraints[unique].clone();
            begin(&mut demo,0,unique,2,UiKey::Edit);demo.ui.paste_into_focused_text("unique \"updated\"");demo.snapshot_inputs();
            assert_eq!(demo.state.wizard_index_constraints[unique].name.as_deref(),Some("unique \"updated\""));assert!(demo.ui.key_input(UiKey::Undo));finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints[unique],original);
            assert!(demo.ui.key_input(UiKey::Redo));finish(&mut demo);demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.ov_constraint_editor));assert!(demo.ui.key_input(UiKey::Undo));finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints[unique],original);
            demo.ui.key_input(UiKey::Redo);finish(&mut demo);assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints[unique],original);
            begin(&mut demo,0,unique,2,UiKey::Char('u'));demo.ui.paste_into_focused_text("nique_final");finish(&mut demo);assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert_eq!(demo.state.wizard_index_constraints[unique].name.as_deref(),Some("unique_final"));
            begin(&mut demo,1,0,1,UiKey::Enter);demo.ui.paste_into_focused_text("parent_final");finish(&mut demo);assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);assert_eq!(demo.state.wizard_constraint_edit.as_ref().unwrap().column,2);
            let clause="REFERENCES parents(p,q)\nON UPDATE CASCADE ON DELETE CASCADE\nDEFERRABLE INITIALLY DEFERRED";
            let suffix="ON UPDATE CASCADE ON DELETE CASCADE\nDEFERRABLE INITIALLY DEFERRED";
            demo.ui.key_input(UiKey::Tab);demo.ui.key_input(UiKey::Tab);demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text(suffix);finish(&mut demo);assert_eq!(table_sql::foreign_key_reference(&demo.state.wizard_foreign_keys[0].clause).unwrap(),table_sql::foreign_key_reference(clause).unwrap());assert!(!demo.state.wizard_foreign_keys[0].inline);
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_CONSTRAINT_EDIT_QA_OUT") {
                demo.scene.mark_full_dirty(1400.0,1000.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(1400,1000,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-references.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}: {delta}");eprintln!("Constraint references {theme_name}: Soft/Metal delta {delta}");} else {eprintln!("Constraint references GPU comparison not_run: adapter unavailable");}
            }
            demo.win_w=800;demo.win_h=600;demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.reference_editor.clauses));assert_eq!(demo.ui.text_input_value(demo.ids.reference_editor.clauses),Some(suffix));assert!(demo.ui.key_input(UiKey::Undo));finish(&mut demo);assert_ne!(table_sql::foreign_key_reference(&demo.state.wizard_foreign_keys[0].clause).unwrap(),table_sql::foreign_key_reference(clause).unwrap());demo.ui.key_input(UiKey::Redo);finish(&mut demo);
            let node=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.reference_editor.parent)).unwrap();assert!(node.bounds.width>30.0);demo.win_w=1400;demo.win_h=1000;demo.do_rebuild();demo.ui.key_input(UiKey::Enter);finish(&mut demo);
            let inline=demo.state.wizard_checks.iter().position(|check|check.column.is_some()).unwrap();let original=demo.state.wizard_checks[inline].clone();
            begin(&mut demo,2,inline,0,UiKey::Edit);demo.ui.paste_into_focused_text("(");finish(&mut demo);assert!(demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().starts_with("--"));
            demo.ui.key_input(UiKey::Escape);finish(&mut demo);assert_eq!(demo.state.wizard_checks[inline],original);assert!(!demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().starts_with("--"));
            begin(&mut demo,2,inline,0,UiKey::Edit);let expression="length(value)>0\nAND value != 'blocked'";demo.ui.paste_into_focused_text(expression);finish(&mut demo);assert_eq!(demo.state.wizard_cols[1].check_expression,expression);
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_CONSTRAINT_EDIT_QA_OUT") {
                demo.scene.mark_full_dirty(1400.0,1000.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(1400,1000,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-check.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}: {delta}");eprintln!("Constraint CHECK {theme_name}: Soft/Metal delta {delta}");} else {eprintln!("Constraint CHECK GPU comparison not_run: adapter unavailable");}
            }
            assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);assert_eq!(demo.state.wizard_constraint_edit.as_ref().unwrap().column,1);demo.ui.paste_into_focused_text("inline_final");finish(&mut demo);
            assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert_eq!(demo.state.wizard_checks[inline].name.as_deref(),Some("inline_final"));
            begin(&mut demo,2,0,1,UiKey::Edit);demo.ui.paste_into_focused_text("table_final");finish(&mut demo);assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);assert_eq!(demo.state.wizard_constraint_edit.as_ref().unwrap().row,1);assert_eq!(demo.state.wizard_checks[0].name.as_deref(),Some("table_final"));
            demo.ui.key_input(UiKey::Escape);finish(&mut demo);let table=demo.ids.ov_checks_table;demo.ui.set_table_selected_row(table,Some(inline));demo.ui.set_table_selected_col(table,Some(1));demo.ui.reveal_table_selection(table);demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);
            let grid=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.ov_checks_table)).unwrap().bounds;let style=next_gui_ui::TableStyle::from_theme(&demo.theme);let x=grid.x+450.0;let y=grid.y+style.header_height+style.row_height*(inline as f32+0.5);assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));demo.ui.pointer_released();demo.snapshot_inputs();finish(&mut demo);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_constraint_editor));demo.ui.paste_into_focused_text("pointer_name");finish(&mut demo);
            demo.ui.set_focus(Some(demo.ids.ov_btn_ok));demo.snapshot_inputs();finish(&mut demo);assert!(demo.state.wizard_constraint_edit.is_none());assert_eq!(demo.state.wizard_checks[inline].name.as_deref(),Some("pointer_name"));
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);let conn=demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.query_row("SELECT rowid+id+n FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),16);
            let sql=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='records'",[],|row|row.get::<_,String>(0)).unwrap();for name in ["unique_final","parent_final","pointer_name","COLLATE NOCASE","DESC","ON CONFLICT FAIL","DEFERRABLE INITIALLY DEFERRED"] {assert!(sql.contains(name),"{sql}");}
            assert!(conn.execute("UPDATE records SET value='blocked'",[]).is_err());assert!(conn.execute("INSERT INTO records(value,n,p,q) VALUES('alpha',2,1,3)",[]).is_err());conn.execute("UPDATE records SET value='Beta'",[]).unwrap();assert_eq!(conn.query_row("SELECT value FROM visible",[],|row|row.get::<_,String>(0)).unwrap(),"Beta");assert_eq!(conn.query_row("SELECT value FROM audit",[],|row|row.get::<_,String>(0)).unwrap(),"Beta");
            conn.execute("INSERT INTO records(value,n,p,q) VALUES('Gamma',4,1,3)",[]).unwrap();assert_eq!(conn.last_insert_rowid(),8);conn.execute("UPDATE records SET p=8,q=9 WHERE id=8",[]).unwrap();assert!(conn.execute_batch("COMMIT").is_err());conn.execute("INSERT INTO parents VALUES(8,9)",[]).unwrap();assert_eq!(conn.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|row|row.get::<_,i64>(0)).unwrap(),0);
            demo.state.open_create_table_wizard();assert!(demo.state.wizard_constraint_edit.is_none());
        }
    }

    #[test]
    fn field_removal_confirms_and_updates_keys_without_losing_qualified_rows_or_dependencies() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(untouched);INSERT INTO records VALUES(42);ATTACH ':memory:' AS analytics;
                CREATE TABLE analytics.parent(id INTEGER PRIMARY KEY);INSERT INTO analytics.parent VALUES(7),(11);
                CREATE TABLE analytics.records(id INTEGER CONSTRAINT identity PRIMARY KEY AUTOINCREMENT CONSTRAINT old_id UNIQUE CONSTRAINT positive CHECK(id>0) REFERENCES parent(id),
                    a TEXT,b TEXT CONSTRAINT nonempty CHECK(length(b)>0),p INTEGER REFERENCES parent(id) ON UPDATE CASCADE,g INTEGER AS(length(a)) STORED,
                    CONSTRAINT pair UNIQUE(id DESC,a COLLATE NOCASE ASC) ON CONFLICT FAIL,CONSTRAINT separate UNIQUE(b));
                INSERT INTO analytics.records(id,a,b,p) VALUES(7,'first','west',7),(11,'second','east',11);
                CREATE VIEW analytics.visible AS SELECT a,b,p,g FROM records;CREATE TABLE analytics.audit(value);
                CREATE TRIGGER analytics.edited AFTER UPDATE OF b ON records BEGIN INSERT INTO audit VALUES(new.b);END;").unwrap();
            demo.state.refresh_schema().unwrap();
            let index = demo.state.schema.iter().position(|object| object.schema == "analytics" && object.name == "records").unwrap();
            demo.state.select_schema_node(10_000 + index as u64); demo.state.open_modify_table_wizard().unwrap(); demo.do_rebuild();
            let original = demo.state.wizard_index_constraints.clone(); let sources = demo.state.wizard_col_sources.clone();
            demo.handle_button(demo.ids.ov_btn_del); demo.do_rebuild();
            assert_eq!(demo.state.confirm_kind, ConfirmKind::DeleteColumn(0)); assert_eq!(demo.ui.focused(), Some(demo.ids.modal));
            for (width, height) in [(1400,1000),(800,600)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FIELD_REMOVE_QA_OUT") {
                    demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name} {width}: {delta}");eprintln!("Fields removal {theme_name} {width}: Soft/Metal delta {delta}");} else {eprintln!("Fields removal GPU comparison not_run: adapter unavailable");}
                }
            }
            // Enter defaults to Cancel; Escape also leaves the complete draft unchanged.
            assert!(demo.ui.key_input(UiKey::Enter));demo.handle_modal();demo.do_rebuild();
            assert!(!demo.state.confirm_open);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_fields_table));assert_eq!(demo.state.wizard_index_constraints, original);assert_eq!(demo.state.wizard_col_sources,sources);
            demo.handle_button(demo.ids.ov_btn_del);demo.do_rebuild();assert!(demo.ui.key_input(UiKey::Escape));demo.handle_modal();demo.do_rebuild();assert_eq!(demo.state.wizard_cols.len(),5);
            remove_selected_field_or_index(&mut demo);demo.do_rebuild();assert!(!demo.state.confirm_open);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_fields_table));assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert_eq!(demo.state.wizard_cols.iter().map(|column|column.name.as_str()).collect::<Vec<_>>(),["a","b","p","g"]);
            assert_eq!(demo.state.wizard_col_sources,sources[1..]);assert!(demo.state.wizard_cols.iter().all(|column|!column.pk && !column.auto_increment));
            let pair=demo.state.wizard_index_constraints.iter().find(|key|key.name.as_deref()==Some("pair")).unwrap();assert_eq!(table_sql::index_constraint_columns(pair).unwrap().iter().map(|(name,_)|name.as_str()).collect::<Vec<_>>(),["a"]);
            assert!(pair.columns.contains("COLLATE NOCASE ASC"));assert_eq!(pair.conflict,Some(sqlite3_parser::ast::ResolveType::Fail));assert!(demo.state.wizard_cols[0].unique);
            assert_eq!(demo.state.wizard_checks.len(),1);assert_eq!(demo.state.wizard_checks[0].name.as_deref(),Some("nonempty"));assert_eq!(demo.state.wizard_foreign_keys.len(),1);
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(id) FROM analytics.records",[],|row|row.get::<_,i64>(0)).unwrap(),2);
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT sum(rowid) FROM analytics.records",[],|row|row.get::<_,i64>(0)).unwrap(),18);
            assert_eq!(conn.query_row("SELECT sum(g) FROM analytics.visible",[],|row|row.get::<_,i64>(0)).unwrap(),11);
            assert!(conn.execute("INSERT INTO analytics.records(a,b) VALUES('FIRST','north')",[]).is_err());assert!(conn.execute("UPDATE analytics.records SET b='' WHERE rowid=7",[]).is_err());
            conn.execute("UPDATE analytics.parent SET id=12 WHERE id=7",[]).unwrap();assert_eq!(conn.query_row("SELECT p FROM analytics.records WHERE rowid=7",[],|row|row.get::<_,i64>(0)).unwrap(),12);
            conn.execute("UPDATE analytics.records SET b='changed' WHERE rowid=7",[]).unwrap();assert_eq!(conn.query_row("SELECT value FROM analytics.audit",[],|row|row.get::<_,String>(0)).unwrap(),"changed");
            assert_eq!(conn.query_row("SELECT untouched FROM main.records",[],|row|row.get::<_,i64>(0)).unwrap(),42);
            demo.state.open_create_table_wizard();demo.do_rebuild();demo.handle_button(demo.ids.ov_btn_del);assert!(!demo.state.confirm_open);assert_eq!(demo.state.wizard_cols.len(),1);assert!(demo.state.wizard_index_constraints.is_empty());
        }
    }

    #[test]
    fn field_removal_keeps_ordered_composite_metadata_and_invalid_drafts_atomic() {
        for schema in ["main","temp","analytics"] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.state.new_memory_db().unwrap();
            let conn=demo.state.conn.as_ref().unwrap();conn.execute_batch(&format!("ATTACH ':memory:' AS analytics;
                CREATE TABLE {schema}.parent(x TEXT,y INTEGER PRIMARY KEY,UNIQUE(x,y));INSERT INTO {schema}.parent VALUES('one',1),('two',2);
                CREATE TABLE {schema}.records(a TEXT,b NUMERIC,c TEXT,CONSTRAINT ordered PRIMARY KEY(a DESC,b ASC) ON CONFLICT FAIL,
                    CONSTRAINT pair UNIQUE(a COLLATE NOCASE,b DESC),CONSTRAINT linked FOREIGN KEY(a,b) REFERENCES parent(x,y) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED);
                INSERT INTO {schema}.records(rowid,a,b,c) VALUES(43,'one',1,'west'),(91,'two',2,'east');CREATE VIEW {schema}.visible AS SELECT b,c FROM records;")).unwrap();
            demo.state.refresh_schema().unwrap();let index=demo.state.schema.iter().position(|object|object.schema==schema && object.name=="records").unwrap();demo.state.select_schema_node(10_000+index as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            let columns=format!("{:?}",demo.state.wizard_cols);let sources=demo.state.wizard_col_sources.clone();let keys=demo.state.wizard_index_constraints.clone();let foreign=demo.state.wizard_foreign_keys.clone();
            demo.state.wizard_index_constraints[0].columns="a, (".into();let malformed=demo.state.wizard_index_constraints.clone();assert!(demo.state.remove_wizard_column(0).is_err());
            assert_eq!(demo.state.wizard_index_constraints,malformed);assert_eq!(format!("{:?}",demo.state.wizard_cols),columns);assert_eq!(demo.state.wizard_col_sources,sources);assert_eq!(demo.state.wizard_foreign_keys,foreign);demo.state.wizard_index_constraints=keys;
            remove_selected_field_or_index(&mut demo);demo.do_rebuild();assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let key=&demo.state.wizard_index_constraints[0];assert!(key.primary);assert!(key.columns.contains("b ASC"));assert_eq!(key.conflict,Some(sqlite3_parser::ast::ResolveType::Fail));assert!(demo.state.wizard_cols[0].pk);
            assert!(demo.state.wizard_index_constraints[1].columns.contains("b DESC"));assert!(demo.state.wizard_cols[0].unique);
            assert_eq!(table_sql::foreign_key_columns(&demo.state.wizard_foreign_keys[0]).unwrap()[0].0,"b");assert_eq!(demo.state.wizard_foreign_keys[0].clause,foreign[0].clause);
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_some());assert_eq!(demo.state.conn.as_ref().unwrap().query_row(&format!("SELECT sum(rowid) FROM {schema}.records"),[],|row|row.get::<_,i64>(0)).unwrap(),134);
            demo.state.wizard_foreign_keys[0].clause="REFERENCES parent(y) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED".into();demo.state.store_wizard_foreign_keys();demo.do_rebuild();demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row(&format!("SELECT sum(rowid) FROM {schema}.records"),[],|row|row.get::<_,i64>(0)).unwrap(),134);assert_eq!(conn.query_row(&format!("SELECT sum(b) FROM {schema}.visible"),[],|row|row.get::<_,i64>(0)).unwrap(),3);
            conn.execute(&format!("UPDATE {schema}.parent SET y=3 WHERE y=1"),[]).unwrap();assert_eq!(conn.query_row(&format!("SELECT b FROM {schema}.records WHERE rowid=43"),[],|row|row.get::<_,i64>(0)).unwrap(),3);
            assert!(conn.execute(&format!("INSERT INTO {schema}.records(b,c) VALUES(2,'duplicate')"),[]).is_err());
        }
    }

    #[test]
    fn field_check_text_cells_preserve_named_checks_cancel_history_and_qualified_data() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(untouched);INSERT INTO records VALUES(42);ATTACH ':memory:' AS analytics;
                CREATE TABLE analytics.records(id INTEGER PRIMARY KEY AUTOINCREMENT,a TEXT CONSTRAINT positive CHECK(length(a)>0) CONSTRAINT blocked CHECK(a!='blocked'),b TEXT,g INTEGER AS(length(a)) STORED,CONSTRAINT generated_check CHECK(g>0));
                INSERT INTO analytics.records(id,a,b) VALUES(7,'first','west'),(11,'second','east');CREATE VIEW analytics.visible AS SELECT id,a,b,g FROM records;
                CREATE TABLE analytics.audit(value);CREATE TRIGGER analytics.edited AFTER UPDATE OF a ON records BEGIN INSERT INTO audit VALUES(new.a);END;").unwrap();
            demo.state.refresh_schema().unwrap();let index=demo.state.schema.iter().position(|object|object.schema=="analytics" && object.name=="records").unwrap();demo.state.select_schema_node(10_000+index as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            let finish=|demo:&mut Demo| {demo.handle_table_cell_edit();if demo.rebuild {demo.do_rebuild();}demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
            let begin=|demo:&mut Demo,row| {
                demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(row));demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(7));demo.ui.set_focus(Some(demo.ids.ov_fields_table));demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}
                demo.ui.set_focus(Some(demo.ids.ov_fields_table));assert!(demo.ui.key_input(UiKey::Edit));finish(demo);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_field_editor));assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,7);
            };
            let original=demo.state.wizard_checks.clone();begin(&mut demo,1);demo.ui.paste_into_focused_text("(");finish(&mut demo);
            assert!(demo.ui.text_area_value(demo.ids.ov_table_preview).unwrap().starts_with("--"));demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_some());assert_eq!(demo.state.overlay,Some(OverlayKind::EditTable));
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM analytics.records",[],|row|row.get::<_,i64>(0)).unwrap(),2);
            if demo.rebuild {demo.do_rebuild();}demo.ui.key_input(UiKey::Undo);finish(&mut demo);assert_eq!(demo.state.wizard_checks,original);
            demo.ui.key_input(UiKey::SelectAll);demo.ui.key_input(UiKey::Delete);finish(&mut demo);assert_eq!(demo.state.wizard_checks.len(),original.len());assert_eq!(demo.state.wizard_checks.iter().find(|check|check.name.as_deref()==Some("positive")).unwrap().expression,"");
            demo.ui.key_input(UiKey::Escape);finish(&mut demo);assert_eq!(demo.state.wizard_checks,original);
            begin(&mut demo,1);let expression="length(a)>1\nAND a != 'rejected'";demo.ui.paste_into_focused_text(expression);finish(&mut demo);
            assert_eq!(demo.state.wizard_cols[1].check_expression,expression);
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.ov_field_editor));assert_eq!(demo.ui.text_input_value(demo.ids.ov_field_editor),Some(expression));
                let editor=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.ov_field_editor)).unwrap().bounds;let table=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.ov_fields_table)).unwrap().bounds;assert!(editor.x>=table.x && editor.x+editor.width<=table.x+table.width,"{theme_name} {width}: {editor:?} outside {table:?}");
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FIELD_CHECK_QA_OUT") {
                    demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let delta=soft.frame_pixels().unwrap().0.iter().zip(gpu.frame_pixels().unwrap().0).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name} {width}: {delta}");eprintln!("Fields CHECK {theme_name} {width}: Soft/Metal delta {delta}");} else {eprintln!("Fields CHECK GPU comparison not_run: adapter unavailable");}
                }
            }
            demo.ui.key_input(UiKey::Undo);finish(&mut demo);assert_eq!(demo.state.wizard_checks,original);demo.ui.key_input(UiKey::Redo);finish(&mut demo);demo.ui.key_input(UiKey::Enter);finish(&mut demo);
            begin(&mut demo,2);let before=demo.state.wizard_checks.clone();demo.ui.paste_into_focused_text("length(b)>0");finish(&mut demo);demo.ui.key_input(UiKey::Escape);finish(&mut demo);assert_eq!(demo.state.wizard_checks,before);
            begin(&mut demo,2);demo.ui.paste_into_focused_text("length(b)>0");finish(&mut demo);demo.ui.key_input(UiKey::Enter);finish(&mut demo);
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();assert!(conn.execute("UPDATE analytics.records SET a='x' WHERE id=7",[]).is_err());assert!(conn.execute("UPDATE analytics.records SET a='rejected' WHERE id=7",[]).is_err());assert!(conn.execute("UPDATE analytics.records SET b='' WHERE id=7",[]).is_err());
            assert_eq!(conn.query_row("SELECT sum(id)+sum(g) FROM analytics.visible",[],|row|row.get::<_,i64>(0)).unwrap(),29);
            demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();begin(&mut demo,1);demo.ui.key_input(UiKey::SelectAll);demo.ui.key_input(UiKey::Delete);finish(&mut demo);demo.ui.key_input(UiKey::Enter);finish(&mut demo);
            assert!(!demo.state.wizard_checks.iter().any(|check|check.name.as_deref()==Some("positive")));assert_eq!(demo.state.wizard_cols[1].check_expression,"a <> 'blocked'");
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();assert!(conn.execute("UPDATE analytics.records SET a='blocked'",[]).is_err());conn.execute("UPDATE analytics.records SET a='x' WHERE id=7",[]).unwrap();
            assert_eq!(conn.query_row("SELECT value FROM analytics.audit",[],|row|row.get::<_,String>(0)).unwrap(),"x");
            conn.execute("INSERT INTO analytics.records(a,b) VALUES('third','south')",[]).unwrap();assert_eq!(conn.last_insert_rowid(),12);assert_eq!(conn.query_row("SELECT untouched FROM main.records",[],|row|row.get::<_,i64>(0)).unwrap(),42);
        }
    }

    #[test]
    fn csv_preview_options_refresh_focus_scroll_and_existing_table_confirmation() {
        for (theme_name, theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height) in [(1400,1000),(800,600)] {
                let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("source.csv");
                std::fs::write(&path,"id§amount§note\n1§2.5§「 padded§quoted 「\n2§3§plain\n").unwrap();
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap(); demo.state.csv_path=path.display().to_string(); demo.state.csv_table="imported".into(); demo.state.csv_delim_idx=4; demo.state.csv_custom_delimiter="§".into(); demo.state.csv_quote_idx=3; demo.state.csv_custom_quote="「".into(); demo.state.overlay=Some(OverlayKind::ImportCsv); demo.do_rebuild();
                let finish=|demo:&mut Demo|{demo.snapshot_inputs();if demo.rebuild{demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
                let bounds=|demo:&Demo,id|demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
                let capture=|demo:&mut Demo,phase| {
                    if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_CSV_PREVIEW_QA_OUT") {
                        let (w,h)=(demo.win_w,demo.win_h);demo.scene.mark_full_dirty(w as f32,h as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(w,h,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-{width}-{phase}.ppm"),soft.frame_pixels().unwrap().0,w,h).unwrap();
                        if let Ok(mut gpu)=GpuRenderer::try_new(w,h,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-{width}-{phase}-metal.ppm"),b,w,h).unwrap();let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{width}/{phase}: {delta}");eprintln!("CSV preview {theme_name}/{width}/{phase}: Soft/Metal delta {delta}");}else{eprintln!("CSV preview GPU comparison not_run: adapter unavailable");}
                    }
                };
                assert_eq!(demo.state.csv_preview.as_ref().unwrap().as_ref().unwrap().rows[0][2]," padded§quoted ");
                demo.ui.set_checked(demo.ids.ov_csv_trim,true);finish(&mut demo);
                assert_eq!(demo.state.csv_preview.as_ref().unwrap().as_ref().unwrap().rows[0][2],"padded§quoted");
                let preview=bounds(&demo,demo.ids.ov_csv_preview);let import=bounds(&demo,demo.ids.ov_btn_ok);assert!(preview.x>=320.0 && preview.x+preview.width<=width as f32 && preview.y+preview.height<import.y);assert!(import.y+import.height<=height as f32);demo.ui.set_table_scroll_x(demo.ids.ov_csv_preview,10000.0);assert_eq!(demo.ui.table_scroll_offset(demo.ids.ov_csv_preview).unwrap().0,0.0);capture(&mut demo,"preview");
                demo.ui.set_focus(Some(demo.ids.ov_csv_custom_delimiter));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("xx");finish(&mut demo);assert!(demo.state.csv_preview.as_ref().unwrap().is_err());assert_eq!(demo.ui.focused(),Some(demo.ids.ov_csv_custom_delimiter));
                demo.ui.key_input(UiKey::Undo);finish(&mut demo);assert!(demo.state.csv_preview.as_ref().unwrap().is_ok());assert_eq!(demo.ui.text_input_value(demo.ids.ov_csv_custom_delimiter),Some("§"));
                demo.ui.set_scroll_offset(demo.ids.ov_csv_scroll,0.0,10000.0);demo.ui.sync(&mut demo.scene);let advanced=bounds(&demo,demo.ids.ov_csv_advanced);assert_eq!(demo.ui.hit_widget(&demo.scene,advanced.x+8.0,advanced.y+8.0),Some(demo.ids.ov_csv_advanced));demo.ui.pointer_pressed(&mut demo.scene,advanced.x+8.0,advanced.y+8.0);demo.ui.pointer_released();demo.handle_button(demo.ids.ov_csv_advanced);demo.do_rebuild();assert!(demo.state.csv_advanced);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_csv_advanced));
                demo.ui.set_checked(demo.ids.ov_csv_types,true);finish(&mut demo);assert!(!demo.state.csv_import_settings.detect_types);
                demo.ui.set_scroll_offset(demo.ids.ov_csv_scroll,0.0,10000.0);finish(&mut demo);let old_scroll=demo.state.csv_scroll;demo.do_rebuild();assert_eq!(demo.ui.scroll_offset(demo.ids.ov_csv_scroll),Some(old_scroll));capture(&mut demo,"advanced");
                std::fs::write(&path,"id§amount§note\n3§4§refreshed\n").unwrap();demo.handle_button(demo.ids.ov_csv_refresh);demo.do_rebuild();assert_eq!(demo.state.csv_preview.as_ref().unwrap().as_ref().unwrap().rows[0][2],"refreshed");
                demo.handle_button(demo.ids.ov_btn_ok);demo.settle_csv_test();assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());assert!(dbmod::table_columns(demo.state.conn.as_ref().unwrap(),"imported").unwrap().iter().all(|column|column.type_name.is_empty()));
                demo.state.csv_paste="x§y§z\n4§5§append\n".into();demo.state.overlay=Some(OverlayKind::PasteCsv);demo.state.csv_preview=None;demo.do_rebuild();demo.handle_button(demo.ids.ov_btn_ok);demo.settle_csv_test();demo.do_rebuild();assert!(demo.state.csv_work.as_ref().unwrap().pending.is_some());capture(&mut demo,"confirm");
                assert!(demo.ui.key_input(UiKey::Escape));demo.handle_csv_confirmation();demo.settle_csv_test();demo.do_rebuild();assert!(demo.state.overlay.is_some());assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"imported").unwrap(),1);
                demo.handle_button(demo.ids.ov_btn_ok);demo.settle_csv_test();demo.do_rebuild();assert!(demo.ui.key_input(UiKey::BackTab));assert!(demo.ui.key_input(UiKey::Enter));demo.handle_csv_confirmation();demo.settle_csv_test();assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"imported").unwrap(),2);
            }
        }
    }

    #[test]
    fn csv_encoding_controls_keep_sessions_repair_errors_and_import_file_and_clipboard_text() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height) in [(1400,1000),(800,600)] {
                let dir=tempfile::tempdir().unwrap();let path=dir.path().join("customers.csv");std::fs::write(&path,b"value,note\r\ncaf\xe9,\x80\r\n").unwrap();
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();demo.state.csv_path=path.display().to_string();demo.state.csv_table="imported".into();demo.state.overlay=Some(OverlayKind::ImportCsv);demo.do_rebuild();
                let finish=|demo:&mut Demo|{demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
                let capture=|demo:&mut Demo,phase| {
                    if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_CSV_ENCODING_QA_OUT") {
                        let (width,height)=(demo.win_w,demo.win_h);demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-{width}-{phase}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                        if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-{width}-{phase}-metal.ppm"),b,width,height).unwrap();let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{width}/{phase}: {delta}");eprintln!("CSV encoding {theme_name}/{width}/{phase}: Soft/Metal delta {delta}");}else{eprintln!("CSV encoding GPU comparison not_run: adapter unavailable");}
                    }
                };
                demo.ui.set_focus(Some(demo.ids.ov_csv_encoding));assert!(demo.ui.key_input(UiKey::Space));
                for _ in 0..2 {assert!(demo.ui.key_input(UiKey::Down));finish(&mut demo);assert!(demo.ui.has_open_popup());assert_eq!(demo.ui.focused(),Some(demo.ids.ov_csv_encoding));}
                assert_eq!(demo.state.csv_options().encoding,"ISO-8859-1");capture(&mut demo,"encoding");
                assert!(demo.ui.key_input(UiKey::Down));finish(&mut demo);assert_eq!(demo.state.csv_encoding_idx,3);assert!(demo.ids.ov_csv_custom_encoding.0!=0);demo.ui.key_input(UiKey::Enter);finish(&mut demo);
                demo.ui.set_focus(Some(demo.ids.ov_csv_custom_encoding));demo.ui.key_input(UiKey::SelectAll);assert!(demo.ui.paste_into_focused_text("windows-1252"));finish(&mut demo);assert_eq!(demo.state.csv_options().encoding,"windows-1252");capture(&mut demo,"custom");
                demo.win_w=if width==800 {1400} else {800};demo.win_h=if height==600 {1000} else {600};demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.ov_csv_custom_encoding));assert!(demo.ui.has_open_popup());assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_csv_custom_encoding),Some("windows-1252"));
                demo.ui.close_popup();demo.ui.set_autocomplete_value(demo.ids.ov_csv_custom_encoding,"bad-codec");finish(&mut demo);demo.handle_button(demo.ids.ov_btn_ok);demo.settle_csv_test();assert!(demo.state.error_msg.as_deref().unwrap().contains("Unsupported CSV encoding"));assert_eq!(demo.state.overlay,Some(OverlayKind::ImportCsv));assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM sqlite_schema WHERE name='imported'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
                demo.clear_error();demo.do_rebuild();demo.ui.set_autocomplete_value(demo.ids.ov_csv_custom_encoding,"windows-1252");finish(&mut demo);demo.handle_button(demo.ids.ov_btn_ok);demo.settle_csv_test();assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT value||note FROM imported",[],|r|r.get::<_,String>(0)).unwrap(),"café€");
                demo.state.overlay=Some(OverlayKind::PasteCsv);demo.state.csv_table="clipboard".into();demo.state.csv_custom_encoding="bad-codec".into();demo.state.csv_paste="value\n日本\n".into();demo.do_rebuild();assert_eq!(demo.ids.ov_csv_encoding.0,0);assert_eq!(demo.ids.ov_csv_custom_encoding.0,0);demo.handle_button(demo.ids.ov_btn_ok);demo.settle_csv_test();assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT value FROM clipboard",[],|r|r.get::<_,String>(0)).unwrap(),"日本");
            }
        }
    }

    #[test]
    fn constraint_and_reference_select_popups_survive_resize_and_keep_escape_local() {
        for theme in [Theme::dark(),Theme::light()] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE p1(id INTEGER PRIMARY KEY);CREATE TABLE p2(id INTEGER PRIMARY KEY);INSERT INTO p1 VALUES(1);INSERT INTO p2 VALUES(1);CREATE TABLE records(id INTEGER PRIMARY KEY,p INTEGER REFERENCES p1(id));INSERT INTO records VALUES(7,1);").unwrap();demo.state.refresh_schema().unwrap();
            let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            let finish=|demo:&mut Demo| {demo.handle_table_cell_edit();if demo.rebuild {demo.do_rebuild();}demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1));demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(9));demo.ui.set_focus(Some(demo.ids.ov_fields_table));demo.snapshot_inputs();demo.do_rebuild();demo.ui.set_focus(Some(demo.ids.ov_fields_table));assert!(demo.ui.key_input(UiKey::Edit));finish(&mut demo);
            assert_eq!(demo.ui.focused(),Some(demo.ids.reference_editor.parent));assert!(demo.ui.key_input(UiKey::Space));demo.snapshot_inputs();demo.win_w=800;demo.win_h=600;demo.do_rebuild();
            assert!(demo.ui.has_open_popup());assert_eq!(demo.ui.select_value(demo.ids.reference_editor.parent),Some("p1"));assert_eq!(demo.ui.focused(),Some(demo.ids.reference_editor.parent));
            assert!(demo.ui.key_input(UiKey::Down));demo.snapshot_inputs();demo.win_w=1400;demo.win_h=1000;demo.do_rebuild();assert!(demo.ui.has_open_popup());assert_eq!(demo.ui.select_value(demo.ids.reference_editor.parent),Some("p2"));
            assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(!demo.ui.has_open_popup());assert!(demo.state.wizard_field_edit.is_some());demo.do_rebuild();assert!(!demo.ui.has_open_popup());
            assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_none());assert_eq!(table_sql::foreign_key_reference(&demo.state.wizard_cols[1].foreign_key_clause).unwrap().table,"p1");
            demo.state.wizard_table_page=1;demo.state.wizard_key_sel=0;demo.do_rebuild();demo.ui.set_table_selected_row(demo.ids.ov_keys_table,Some(0));demo.ui.set_table_selected_col(demo.ids.ov_keys_table,Some(1));demo.ui.set_focus(Some(demo.ids.ov_keys_table));demo.snapshot_inputs();assert!(demo.ui.key_input(UiKey::Edit));finish(&mut demo);
            assert!(demo.ui.is_select(demo.ids.ov_constraint_editor));assert!(demo.ui.has_open_popup());demo.snapshot_inputs();demo.win_w=800;demo.win_h=600;demo.do_rebuild();assert!(demo.ui.has_open_popup());assert_eq!(demo.ui.select_value(demo.ids.ov_constraint_editor),Some("PRIMARY KEY"));
            assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(demo.state.wizard_constraint_edit.is_some());assert!(!demo.ui.has_open_popup());demo.do_rebuild();assert!(!demo.ui.has_open_popup());assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(demo.state.wizard_constraint_edit.is_none());
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT id+p FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),8);assert!(conn.execute("UPDATE records SET p=2",[]).is_err());
        }
    }

    #[test]
    fn field_dropdowns_open_on_first_click_keep_custom_types_and_save_collation() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height) in [(1400,1000),(800,600)] {
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
                demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,name TEXT DEFAULT 'kept' COLLATE NOCASE); INSERT INTO records VALUES(7,'alpha   ');").unwrap();
                demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
                let finish=|demo:&mut Demo| {demo.handle_table_cell_edit();if demo.rebuild {demo.do_rebuild();}demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
                let click=|demo:&mut Demo,column| {
                    demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(column));demo.ui.reveal_table_selection(demo.ids.ov_fields_table);demo.ui.sync(&mut demo.scene);
                    let table=demo.ids.ov_fields_table;let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(table)).unwrap().bounds;
                    let style=next_gui_ui::TableStyle::from_theme(&demo.theme);let widths=demo.ui.table_column_widths(table).unwrap();let (sx,sy)=demo.ui.table_scroll_offset(table).unwrap();
                    let x=(bounds.x+style.padding+widths[..column].iter().sum::<f32>()-sx+widths[column]*0.5).clamp(bounds.x+widths[0]+5.0,bounds.x+bounds.width-5.0);
                    let y=bounds.y+style.header_height+style.row_height*1.5-sy;
                    assert_eq!(demo.ui.hit_widget(&demo.scene,x,y),Some(table));assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));demo.ui.pointer_released();
                    demo.snapshot_inputs();finish(demo);assert_eq!(demo.state.wizard_col_sel,1);assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,column);assert!(demo.ui.has_open_popup());
                };
                let capture=|demo:&mut Demo,phase| {
                    let (width,height)=(demo.win_w,demo.win_h);
                    if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FIELD_DROPDOWN_QA_OUT") {
                        demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);
                        write_ppm(&format!("{prefix}-{theme_name}-{width}-{phase}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                        if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-{width}-{phase}-metal.ppm"),b,width,height).unwrap();let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{width}/{phase}: {delta}");eprintln!("Field dropdown {theme_name}/{width}/{phase}: Soft/Metal delta {delta}");}
                        else {eprintln!("Field dropdown GPU comparison not_run: adapter unavailable");}
                    }
                };
                capture(&mut demo,"closed");click(&mut demo,1);assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_field_editor),Some("TEXT"));capture(&mut demo,"type");
                assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_some());
                demo.ui.paste_into_focused_text("VARCHAR(48)");demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[1].type_name,"VARCHAR(48)");
                assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert_eq!(demo.state.wizard_cols[1].type_name,"TEXT");assert!(demo.state.wizard_field_edit.is_none());
                click(&mut demo,1);demo.ui.paste_into_focused_text("VARCHAR(48)");demo.snapshot_inputs();assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);
                click(&mut demo,8);assert!(demo.ui.is_select(demo.ids.ov_field_editor));assert!(!demo.ui.paste_into_focused_text("invalid_collation"));capture(&mut demo,"collation");
                assert!(demo.ui.key_input(UiKey::Down));demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[1].collation,"RTRIM");
                demo.win_w=if width==800 {1400} else {800};demo.win_h=if height==600 {1000} else {600};demo.do_rebuild();assert_eq!(demo.ui.select_value(demo.ids.ov_field_editor),Some("RTRIM"));assert!(demo.ui.has_open_popup());capture(&mut demo,"resized-collation");
                demo.ui.set_focus(Some(demo.ids.ov_field_editor));assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_some());assert!(!demo.ui.has_open_popup());demo.do_rebuild();assert!(!demo.ui.has_open_popup());assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_none());
                demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());
                let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT id FROM records WHERE name='alpha'",[],|row|row.get::<_,i64>(0)).unwrap(),7);
                let defs=dbmod::table_column_defs_in(conn,"main","records").unwrap();assert_eq!(defs[1].type_name.replace(' ',""),"VARCHAR(48)");assert_eq!(defs[1].collation,"RTRIM");assert_eq!(defs[1].default_value,"'kept'");
                demo.state.open_modify_table_wizard().unwrap();demo.state.wizard_table_options.strict=true;demo.state.wizard_cols[1].type_name="TEXT".into();demo.state.wizard_col_sel=0;demo.do_rebuild();click(&mut demo,1);
                assert!(demo.ui.is_select(demo.ids.ov_field_editor));assert!(!demo.ui.paste_into_focused_text("VARCHAR(48)"));
                let mut choices=Vec::new();for _ in 0..6 {demo.ui.key_input(UiKey::Down);choices.push(demo.ui.select_value(demo.ids.ov_field_editor).unwrap().to_owned());}
                choices.sort();assert_eq!(choices,vec!["ANY","BLOB","INT","INTEGER","REAL","TEXT"]);demo.snapshot_inputs();
                assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert_eq!(demo.state.wizard_cols[1].type_name,"TEXT");
            }
        }
    }

    #[test]
    fn field_text_cells_edit_cancel_tab_restore_history_and_save_real_constraints() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,name TEXT DEFAULT 'old' CHECK(length(name)>0) COLLATE NOCASE,n INTEGER); INSERT INTO records VALUES(7,'alpha',3); CREATE VIEW visible AS SELECT id,name,n FROM records;").unwrap();demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            let finish=|demo:&mut Demo| {demo.handle_field_check_change();demo.handle_table_cell_edit();if demo.rebuild {demo.do_rebuild();}demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}demo.ui.sync(&mut demo.scene);};
            let begin=|demo:&mut Demo,row,column,key| {
                demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(row));demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(column));demo.ui.set_focus(Some(demo.ids.ov_fields_table));demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();}
                assert!(demo.ui.key_input(key));finish(demo);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_field_editor));assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,column);
            };
            assert_eq!(map_key(KeyCode::Function(2),Modifiers::default()),Some(UiKey::Edit));begin(&mut demo,1,0,UiKey::Edit);
            demo.ui.paste_into_focused_text("customer_name");demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[1].name,"customer_name");assert!(demo.state.wizard_cols[1].check_expression.contains("customer_name"));
            demo.do_rebuild();assert_eq!(demo.ui.focused(),Some(demo.ids.ov_field_editor));assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[1].name,"name");assert!(demo.ui.key_input(UiKey::Redo));demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[1].name,"customer_name");
            assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert_eq!(demo.state.wizard_cols[1].name,"name");assert!(!demo.state.wizard_cols[1].check_expression.contains("customer_name"));assert!(demo.state.wizard_field_edit.is_none());assert_eq!(demo.state.overlay,Some(OverlayKind::EditTable));
            begin(&mut demo,1,1,UiKey::Enter);demo.ui.paste_into_focused_text("VARCHAR(48)");demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[1].type_name,"VARCHAR(48)");assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_none());
            begin(&mut demo,1,6,UiKey::Char('4'));assert_eq!(demo.ui.text_input_value(demo.ids.ov_field_editor),Some("4"));assert_eq!(demo.state.wizard_cols[1].default_value,"4");
            assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,7);assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,8);assert!(demo.ui.is_select(demo.ids.ov_field_editor));assert!(demo.ui.key_input(UiKey::Down));demo.snapshot_inputs();assert!(demo.ui.has_open_popup());
            assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_some());assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert!(demo.state.wizard_field_edit.is_none());assert_eq!(demo.state.wizard_cols[1].collation,"RTRIM");
            begin(&mut demo,1,8,UiKey::Edit);assert!(demo.ui.key_input(UiKey::BackTab));finish(&mut demo);assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,7);assert!(demo.ui.key_input(UiKey::BackTab));finish(&mut demo);assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,6);assert_eq!(demo.ui.text_input_value(demo.ids.ov_field_editor),Some("4"));
            for _ in 0..3 {assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);}assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,9);
            for _ in 0..3 {assert!(demo.ui.key_input(UiKey::Tab));finish(&mut demo);}
            assert_eq!(demo.state.wizard_col_sel,2);assert_eq!(demo.state.wizard_field_edit.as_ref().unwrap().column,0);
            demo.ui.paste_into_focused_text("qty");demo.snapshot_inputs();assert_eq!(demo.state.wizard_cols[2].name,"qty");assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);assert_eq!(demo.state.wizard_cols[1].default_value,"4");
            // A second click on the already selected Name cell starts the standard input.
            demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(0));demo.ui.set_table_scroll_offset(demo.ids.ov_fields_table,0.0,0.0);demo.ui.sync(&mut demo.scene);
            let table=demo.ids.ov_fields_table;let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(table)).unwrap().bounds;let style=next_gui_ui::TableStyle::from_theme(&demo.theme);
            let (x,y)=(bounds.x+40.0,bounds.y+style.header_height+style.row_height*2.5);assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));demo.ui.pointer_released();demo.snapshot_inputs();finish(&mut demo);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_field_editor));demo.ui.paste_into_focused_text("amount");demo.snapshot_inputs();
            assert!(demo.ui.key_input(UiKey::Escape));finish(&mut demo);assert_eq!(demo.state.wizard_cols[2].name,"qty");
            begin(&mut demo,1,6,UiKey::Edit);demo.ui.paste_into_focused_text("5");demo.snapshot_inputs();demo.ui.set_table_scroll_offset(demo.ids.ov_fields_table,0.0,0.0);demo.ui.sync(&mut demo.scene);
            let table=demo.ids.ov_fields_table;let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(table)).unwrap().bounds;let widths=demo.ui.table_column_widths(table).unwrap();
            let (x,y)=(bounds.x+style.padding+widths[..2].iter().sum::<f32>()+widths[2]*0.5,bounds.y+style.header_height+style.row_height*2.5);
            assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));demo.ui.pointer_released();demo.snapshot_inputs();finish(&mut demo);assert!(demo.state.wizard_field_edit.is_none());assert_eq!(demo.state.wizard_cols[1].default_value,"5");assert!(demo.state.wizard_cols[2].notnull);
            for (width,height) in [(1400,1000),(800,600)] {
                demo.win_w=width;demo.win_h=height;demo.do_rebuild();begin(&mut demo,1,8,UiKey::Edit);demo.ui.sync(&mut demo.scene);
                let editor=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.ov_field_editor)).unwrap();let grid=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.ov_fields_table)).unwrap();
                let clip=editor.clip.unwrap();assert!(clip.width>20.0 && clip.height>10.0);assert!(clip.x>=grid.bounds.x && clip.x+clip.width<=grid.bounds.x+grid.bounds.width+0.001);assert!(clip.y>=grid.bounds.y+style.header_height);
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FIELD_EDITOR_QA_OUT") {
                    demo.scene.mark_full_dirty(width as f32,height as f32);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(width,height,clear);soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{width}.ppm"),soft.frame_pixels().unwrap().0,width,height).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(width,height,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-{width}-metal.ppm"),b,width,height).unwrap();let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{width}: {delta}");eprintln!("Field editor {theme_name}/{width}: Soft/Metal delta {delta}");}
                    else {eprintln!("Field editor GPU comparison not_run: adapter unavailable");}
                }
                assert!(demo.ui.key_input(UiKey::Enter));finish(&mut demo);
            }
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);let conn=demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.query_row("SELECT rowid+id+qty FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),17);assert_eq!(conn.query_row("SELECT qty FROM visible",[],|row|row.get::<_,i64>(0)).unwrap(),3);
            assert_eq!(conn.query_row("SELECT count(*) FROM records WHERE name='alpha   '",[],|row|row.get::<_,i64>(0)).unwrap(),1);assert!(conn.execute("INSERT INTO records(name) VALUES('')",[]).is_err());conn.execute("INSERT INTO records(qty) VALUES(9)",[]).unwrap();assert_eq!(conn.query_row("SELECT name FROM records WHERE qty=9",[],|row|row.get::<_,String>(0)).unwrap(),"5");
            assert_eq!(dbmod::table_column_defs_in(conn,"main","records").unwrap()[1].type_name.replace(' ',""),"VARCHAR(48)");demo.state.open_create_table_wizard();assert!(demo.state.wizard_field_edit.is_none());
        }
    }

    #[test]
    fn field_check_cells_preserve_previous_row_validate_keys_and_save_qualified_data() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(untouched); INSERT INTO records VALUES(42); ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE \"aux.\"\"db\".records(id INTEGER PRIMARY KEY,code TEXT,qty TEXT,total AS(length(code))); INSERT INTO \"aux.\"\"db\".records VALUES(7,'first','12'),(11,'second','24'); CREATE VIEW \"aux.\"\"db\".visible AS SELECT code,total FROM records;").unwrap();
            demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.schema=="aux.\"db" && object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            let finish=|demo:&mut Demo| {
                demo.handle_field_check_change();
                if demo.rebuild {demo.do_rebuild();} else {demo.snapshot_inputs();if demo.rebuild {demo.do_rebuild();} else {demo.ui.sync(&mut demo.scene);}}
            };
            let click=|demo:&mut Demo,row:usize,col:usize| {
                let table=demo.ids.ov_fields_table;let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(table)).unwrap().bounds;
                let style=next_gui_ui::TableStyle::from_theme(&demo.theme);let widths=demo.ui.table_column_widths(table).unwrap();
                let x=bounds.x+style.padding+widths[..col].iter().sum::<f32>()+widths[col]*0.5;
                let y=bounds.y+style.header_height+(row as f32+0.5)*style.row_height;
                assert_eq!(demo.ui.hit_widget(&demo.scene,x,y),Some(table));assert!(demo.ui.pointer_pressed(&mut demo.scene,x,y));demo.ui.pointer_released();
                demo.snapshot_inputs();finish(demo);
            };
            demo.ui.set_text_input(demo.ids.ov_default,"100");click(&mut demo,1,2);
            assert_eq!(demo.state.wizard_cols[0].default_value,"100");assert!(demo.state.wizard_cols[1].notnull);assert_eq!(demo.state.wizard_col_sel,1);
            assert_eq!(demo.ui.is_checked(demo.ids.ov_nn),Some(true));assert_eq!(demo.ui.table_cell_checked(demo.ids.ov_fields_table,1,2),Some(true));
            demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(5));demo.ui.set_focus(Some(demo.ids.ov_fields_table));assert!(demo.ui.key_input(UiKey::Space));finish(&mut demo);
            assert!(demo.state.wizard_cols[1].unique);assert_eq!(demo.ui.table_selected_text(demo.ids.ov_fields_table).as_deref(),Some("Yes"));
            click(&mut demo,3,3);assert!(demo.state.error_msg.as_deref().unwrap().contains("Generated"));assert!(!demo.state.wizard_cols[3].pk);
            assert_eq!(demo.ui.is_checked(demo.ids.ov_pk),Some(false));assert_eq!(demo.ui.table_cell_checked(demo.ids.ov_fields_table,3,3),Some(false));assert!(demo.state.wizard_cols[0].pk);
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT sum(id) FROM \"aux.\"\"db\".records",[],|row|row.get::<_,i64>(0)).unwrap(),18);
            demo.clear_error();demo.do_rebuild();
            if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FIELD_CHECK_QA_OUT") {
                demo.scene.mark_full_dirty(1400.0,1000.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(1400,1000,clear);soft.render(&commands,&dirty);
                write_ppm(&format!("{prefix}-{theme_name}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-metal.ppm"),b,1400,1000).unwrap();let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}: {delta}");eprintln!("Fields checks {theme_name}: Soft/Metal delta {delta}");}
                else {eprintln!("Fields checks GPU comparison not_run: adapter unavailable");}
            }
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.overlay.is_none());
            let conn=demo.state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT sum(rowid)+sum(id) FROM \"aux.\"\"db\".records",[],|row|row.get::<_,i64>(0)).unwrap(),36);
            assert!(conn.execute("INSERT INTO \"aux.\"\"db\".records(code) VALUES('first')",[]).is_err());assert!(conn.execute("UPDATE \"aux.\"\"db\".records SET code=NULL",[]).is_err());
            assert_eq!(conn.query_row("SELECT sum(total) FROM \"aux.\"\"db\".visible",[],|row|row.get::<_,i64>(0)).unwrap(),11);assert_eq!(conn.query_row("SELECT untouched FROM main.records",[],|row|row.get::<_,i64>(0)).unwrap(),42);
            demo.state.open_create_table_wizard();demo.state.wizard_cols[0].type_name="TEXT".into();demo.do_rebuild();click(&mut demo,0,4);
            assert_eq!(demo.state.wizard_cols[0].type_name,"INTEGER");assert!(demo.state.wizard_cols[0].pk && demo.state.wizard_cols[0].auto_increment);
            assert_eq!(demo.ui.table_cell_checked(demo.ids.ov_fields_table,0,3),Some(true));assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type),Some("INTEGER"));
            click(&mut demo,0,3);assert!(!demo.state.wizard_cols[0].pk && !demo.state.wizard_cols[0].auto_increment);assert_eq!(demo.ui.table_cell_checked(demo.ids.ov_fields_table,0,4),Some(false));
            click(&mut demo,0,4);demo.state.wizard_table_options.without_rowid=true;demo.do_rebuild();demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some());assert_eq!(demo.state.overlay,Some(OverlayKind::EditTable));assert!(demo.state.wizard_cols[0].auto_increment);
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM main.sqlite_schema WHERE name='new_table'",[],|row|row.get::<_,i64>(0)).unwrap(),0);
            demo.clear_error();demo.state.wizard_table_options.without_rowid=false;demo.do_rebuild();demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();let schema=demo.state.wizard_schema.clone();let table=dbmod::qualified_name(&schema,"new_table");
            conn.execute(&format!("INSERT INTO {table}(name) VALUES('created')"),[]).unwrap();assert_eq!(conn.query_row(&format!("SELECT id FROM {table}"),[],|row|row.get::<_,i64>(0)).unwrap(),1);
            assert!(dbmod::table_column_defs_in(conn,&schema,"new_table").unwrap()[0].auto_increment);
        }
    }

    #[test]
    fn field_grid_keeps_live_properties_selection_widths_scroll_and_endpoint_move_sessions() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);demo.state.new_memory_db().unwrap();
            let columns=(0..28).map(|i|format!("f{i:02} TEXT DEFAULT 'kept'" )).collect::<Vec<_>>().join(",");
            demo.state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE records(id INTEGER PRIMARY KEY,{columns}); INSERT INTO records(id) VALUES(7),(11); CREATE INDEX by_field ON records(f02); CREATE VIEW visible AS SELECT id,f02 FROM records; CREATE TABLE audit(value); CREATE TRIGGER record_insert AFTER INSERT ON records BEGIN INSERT INTO audit VALUES(new.f02); END;")).unwrap();
            demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();demo.do_rebuild();
            assert_eq!(demo.ui.table_column_widths(demo.ids.ov_fields_table).unwrap().len(),10);assert!(demo.ui.accessible_name(demo.ids.ov_fields_table).unwrap().contains("Foreign Key"));
            assert_eq!(demo.ui.button_enabled(demo.ids.ov_field_top),Some(false));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(3));demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(1));demo.ui.set_focus(Some(demo.ids.ov_fields_table));demo.snapshot_inputs();demo.do_rebuild();
            assert_eq!(demo.state.wizard_col_sel,3);assert_eq!(demo.ui.text_input_value(demo.ids.ov_col_name),Some("f02"));assert_eq!(demo.ui.focused(),Some(demo.ids.ov_fields_table));
            demo.ui.set_autocomplete_value(demo.ids.ov_col_type,"VARCHAR(24)");demo.ui.set_checked(demo.ids.ov_nn,true);demo.ui.set_text_input(demo.ids.ov_default,"'changed'");
            demo.ui.set_focus(Some(demo.ids.ov_check));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("length(f02)>0");demo.snapshot_inputs();
            let cell=|demo:&mut Demo,column| {demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(column));demo.ui.table_selected_text(demo.ids.ov_fields_table).unwrap()};
            assert_eq!(cell(&mut demo,1),"VARCHAR(24)");assert_eq!(cell(&mut demo,2),"Yes");assert_eq!(cell(&mut demo,6),"'changed'");assert_eq!(cell(&mut demo,7),"length(f02)>0");
            demo.ui.set_table_column_width(demo.ids.ov_fields_table,0,210.0);demo.ui.set_table_column_width(demo.ids.ov_fields_table,9,340.0);demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(9));demo.ui.set_table_scroll_offset(demo.ids.ov_fields_table,10000.0,10000.0);demo.ui.sync(&mut demo.scene);
            let offset=demo.ui.table_scroll_offset(demo.ids.ov_fields_table).unwrap();assert!(offset.0>0.0 && offset.1>0.0);demo.snapshot_inputs();demo.do_rebuild();
            assert_eq!(demo.ui.table_scroll_offset(demo.ids.ov_fields_table),Some(offset));assert_eq!(demo.ui.table_column_widths(demo.ids.ov_fields_table).unwrap()[0],210.0);assert_eq!(demo.ui.table_selected_col(demo.ids.ov_fields_table),Some(9));
            demo.ui.set_focus(Some(demo.ids.ov_fields_table));demo.ui.key_input(UiKey::Down);demo.snapshot_inputs();demo.do_rebuild();assert_eq!(demo.state.wizard_col_sel,4);assert_eq!(demo.ui.focused(),Some(demo.ids.ov_fields_table));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(3));demo.snapshot_inputs();demo.do_rebuild();
            demo.handle_button(demo.ids.ov_field_bottom);demo.do_rebuild();assert_eq!(demo.state.wizard_cols.last().unwrap().name,"f02");assert_eq!(demo.state.wizard_col_sources.last().unwrap().as_deref(),Some("f02"));assert_eq!(demo.ui.table_selected_row(demo.ids.ov_fields_table),Some(28));
            assert_eq!(demo.ui.button_enabled(demo.ids.ov_field_bottom),Some(false));demo.ui.set_focus(Some(demo.ids.ov_check));demo.ui.key_input(UiKey::Undo);assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(""));demo.ui.key_input(UiKey::Redo);demo.snapshot_inputs();
            demo.handle_button(demo.ids.ov_field_top);demo.do_rebuild();assert_eq!(demo.state.wizard_cols[0].name,"f02");assert_eq!(demo.state.wizard_constraint_columns[0],"f02");assert_eq!(demo.ui.button_enabled(demo.ids.ov_field_top),Some(false));
            for phase in ["overview","references"] {
                demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,180.0);demo.ui.set_table_scroll_offset(demo.ids.ov_fields_table,if phase=="references" {10000.0} else {0.0},0.0);demo.ui.sync(&mut demo.scene);
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FIELD_GRID_QA_OUT") {
                    demo.scene.mark_full_dirty(1400.0,1000.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);let mut soft=SoftRenderer::new(1400,1000,clear);soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{phase}: {delta}");eprintln!("Fields grid {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");}
                    else {eprintln!("Fields grid GPU comparison not_run: adapter unavailable");}
                }
            }
            demo.win_w=800;demo.win_h=600;demo.do_rebuild();demo.ui.set_table_selected_col(demo.ids.ov_fields_table,Some(9));demo.ui.reveal_table_selection(demo.ids.ov_fields_table);demo.ui.sync(&mut demo.scene);
            assert!(demo.ui.table_scroll_offset(demo.ids.ov_fields_table).unwrap().0>0.0);let bounds=|demo:&Demo,id|demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
            let grid=bounds(&demo,demo.ids.ov_fields_table);let viewport=bounds(&demo,demo.ids.ov_scroll);let offset=demo.ui.scroll_offset(demo.ids.ov_scroll).unwrap();demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,offset.1+grid.y-viewport.y);demo.ui.sync(&mut demo.scene);
            let grid=bounds(&demo,demo.ids.ov_fields_table);let viewport=bounds(&demo,demo.ids.ov_scroll);assert!(grid.y+0.001>=viewport.y && grid.y+grid.height<=viewport.y+viewport.height+0.001,"{grid:?}/{viewport:?}");
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);let conn=demo.state.conn.as_ref().unwrap();
            assert_eq!(dbmod::table_column_defs_in(conn,"main","records").unwrap()[0].name,"f02");assert_eq!(conn.query_row("SELECT sum(rowid)+sum(id) FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),36);assert!(conn.execute("UPDATE records SET f02=''",[]).is_err());
            assert_eq!(conn.query_row("SELECT count(*) FROM visible",[],|row|row.get::<_,i64>(0)).unwrap(),2);conn.execute("INSERT INTO records(id) VALUES(13)",[]).unwrap();assert_eq!(conn.query_row("SELECT value FROM audit",[],|row|row.get::<_,String>(0)).unwrap(),"changed");
            demo.state.open_create_table_wizard();assert!(demo.state.wizard_field_widths.is_empty());assert_eq!(demo.state.wizard_field_scroll,(0.0,0.0));assert_eq!(demo.state.wizard_field_column,None);
        }
    }

    #[test]
    fn field_endpoint_moves_preserve_qualified_identities_and_reject_inconsistent_bindings() {
        for schema in ["main","temp","aux.\"db"] {
            let mut state=AppState::new_with_demo();state.new_memory_db().unwrap();let table=dbmod::qualified_name(schema,"records");
            state.conn.as_ref().unwrap().execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {table}(id INTEGER PRIMARY KEY,a INTEGER CHECK(a>0),b TEXT DEFAULT 'kept'); INSERT INTO {table} VALUES(7,2,'original');")).unwrap();state.refresh_schema().unwrap();let row=state.schema.iter().position(|object|object.schema==schema && object.name=="records").unwrap();state.select_schema_node(10_000+row as u64);state.open_modify_table_wizard().unwrap();
            state.wizard_col_sel=1;state.move_wizard_column(2).unwrap();assert_eq!(state.wizard_cols.iter().map(|column|column.name.as_str()).collect::<Vec<_>>(),vec!["id","b","a"]);assert_eq!(state.wizard_col_sources[2].as_deref(),Some("a"));assert_eq!(state.wizard_constraint_columns[2],"a");
            let before=dbmod::create_table_ddl_in(schema,"records",&state.wizard_cols);assert!(state.move_wizard_column(3).is_err());assert_eq!(dbmod::create_table_ddl_in(schema,"records",&state.wizard_cols),before);state.wizard_constraint_columns.pop();assert!(state.move_wizard_column(0).is_err());assert_eq!(dbmod::create_table_ddl_in(schema,"records",&state.wizard_cols),before);state.wizard_constraint_columns.push("a".into());state.move_wizard_column(0).unwrap();
            let preview=state.table_wizard_sql().unwrap();state.apply_table_wizard().unwrap();let conn=state.conn.as_ref().unwrap();let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",dbmod::quote_ident(schema)),[],|row|row.get(0)).unwrap();assert_eq!(table_sql::inline_checks(&preview).unwrap(),table_sql::inline_checks(&saved).unwrap());
            assert_eq!(conn.query_row(&format!("SELECT rowid,id,a,b FROM {table}"),[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?))).unwrap(),(7,7,2,"original".into()));assert!(conn.execute(&format!("UPDATE {table} SET a=0"),[]).is_err());
        }
    }

    #[test]
    fn inline_check_page_edits_all_named_checks_and_syncs_fields_and_column_removal() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,v INTEGER CONSTRAINT positive CHECK(v>0) NOT NULL CONSTRAINT ceiling CHECK(v<10),note TEXT CONSTRAINT note_check CHECK(length(note)>0),g INTEGER AS(v*2) STORED,CONSTRAINT table_check CHECK(g<40)); INSERT INTO records(id,v,note) VALUES(7,2,'kept'),(11,4,'pending');").unwrap();
            demo.state.refresh_schema().unwrap();let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();
            demo.state.select_schema_node(10_000+row as u64);demo.state.open_modify_table_wizard().unwrap();
            assert_eq!(demo.state.wizard_checks.len(),4);demo.state.wizard_col_sel=1;demo.state.wizard_check_sel=1;demo.state.wizard_table_page=3;demo.do_rebuild();
            assert_eq!(demo.ui.text_input_value(demo.ids.ov_check_name),Some("positive"));assert_eq!(demo.ui.accessible_name(demo.ids.ov_check_expr).as_deref(),Some("CHECK expression"));
            assert_eq!(demo.ui.button_enabled(demo.ids.ov_check_up),Some(false));demo.handle_button(demo.ids.ov_check_up);assert_eq!(demo.state.wizard_check_sel,1);
            let original=demo.ui.text_area_value(demo.ids.ov_check_expr).unwrap().to_string();
            let expression="records.v > 1\nAND length('v')=1";
            demo.ui.set_text_input(demo.ids.ov_check_name,"positive.\"约束");demo.ui.set_focus(Some(demo.ids.ov_check_expr));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text(expression);demo.snapshot_inputs();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(expression));
            demo.ui.set_table_selected_row(demo.ids.ov_checks_table,Some(2));demo.snapshot_inputs();demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_check_expr));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("v < 20");demo.snapshot_inputs();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(expression));
            demo.ui.set_table_selected_row(demo.ids.ov_checks_table,Some(1));demo.snapshot_inputs();demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_check_expr));demo.ui.key_input(UiKey::Undo);assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo);demo.snapshot_inputs();
            demo.ui.set_scroll_offset(demo.ids.ov_checks_scroll,0.0,10000.0);demo.ui.select_text_area_range(demo.ids.ov_check_expr,0..0);demo.ui.ime_preedit("中文",Some((3,6)));demo.ui.sync(&mut demo.scene);
            let candidate=demo.ui.ime_cursor_area(&demo.scene).unwrap();demo.snapshot_inputs();demo.do_rebuild();
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(candidate));assert!(demo.ui.ime_commit("-- 中文\n"));demo.ui.key_input(UiKey::Undo);demo.snapshot_inputs();
            demo.handle_button(demo.ids.ov_check_down);demo.do_rebuild();assert!(demo.ui.text_area_value(demo.ids.ov_check).unwrap().contains("< 20"));
            assert_eq!(demo.ui.button_enabled(demo.ids.ov_check_down),Some(false));demo.handle_button(demo.ids.ov_check_up);demo.do_rebuild();
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,0);demo.snapshot_inputs();demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_check,"v > 1 AND v != 7");demo.snapshot_inputs();
            assert_eq!(demo.state.wizard_checks[1].expression,"v > 1 AND v != 7");assert_eq!(demo.state.wizard_checks[1].name.as_deref(),Some("positive.\"约束"));
            demo.ui.set_text_input(demo.ids.ov_col_name,"amount");demo.ui.set_text_input(demo.ids.ov_name,"orders");demo.snapshot_inputs();
            assert!(demo.state.wizard_checks[1].expression.contains("\"amount\""));assert_eq!(demo.state.wizard_checks[2].column.as_deref(),Some("amount"));
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,3);demo.snapshot_inputs();demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_check_expr,"amount +");demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_some());demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check_expr),Some("amount +"));assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),2);
            demo.ui.set_text_area_value(demo.ids.ov_check_expr,"amount > 1 AND amount != 7");demo.snapshot_inputs();demo.clear_error();
            demo.handle_button(demo.ids.ov_check_add_column);demo.do_rebuild();assert_eq!(demo.state.wizard_checks.last().unwrap().column.as_deref(),Some("amount"));
            demo.ui.set_text_input(demo.ids.ov_check_name,"not_thirteen");demo.ui.set_text_area_value(demo.ids.ov_check_expr,"amount != 13");demo.snapshot_inputs();
            for phase in ["draft","saved"] {
                if phase=="saved" {demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);demo.do_rebuild();}
                else {demo.ui.set_scroll_offset(demo.ids.ov_checks_scroll,0.0,10000.0);demo.ui.sync(&mut demo.scene);}
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_INLINE_CHECK_QA_OUT") {
                    demo.scene.mark_full_dirty(1400.0,1000.0);let dirty=demo.scene.take_dirty();let commands=encode_scene(&demo.scene,&dirty);let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear);soft.render(&commands,&dirty);write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty);let a=soft.frame_pixels().unwrap().0;let b=gpu.frame_pixels().unwrap().0;write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|shift|(((a>>shift)&255)as i32-((b>>shift)&255)as i32).abs())).max().unwrap();assert!(delta<=2,"{theme_name}/{phase}: {delta}");eprintln!("Inline CHECK {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else {eprintln!("Inline CHECK GPU comparison not_run: adapter unavailable");}
                }
            }
            let conn=demo.state.conn.as_ref().unwrap();
            for update in ["amount=1","amount=7","amount=13","amount=20","note=''","amount=NULL"] {assert!(conn.execute(&format!("UPDATE orders SET {update}"),[]).is_err(),"{update}");}
            conn.execute("UPDATE orders SET amount=10 WHERE id=7",[]).unwrap();
            demo.state.open_modify_table_wizard().unwrap();assert_eq!(demo.state.wizard_checks.len(),5);demo.state.wizard_col_sel=1;demo.state.wizard_table_page=0;demo.do_rebuild();
            let remaining=demo.state.wizard_checks.iter().find(|check|check.name.as_deref()==Some("not_thirteen")).unwrap().expression.clone();
            demo.ui.set_text_area_value(demo.ids.ov_check,"");demo.snapshot_inputs();assert_eq!(demo.state.wizard_check_sel,0);assert!(demo.ui.text_area_value(demo.ids.ov_check).unwrap().contains("< 20"));
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,3);demo.ui.set_table_selected_row(demo.ids.ov_checks_table,Some(1));demo.snapshot_inputs();demo.do_rebuild();
            demo.handle_button(demo.ids.ov_check_del);demo.do_rebuild();assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(remaining.as_str()));
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,0);demo.snapshot_inputs();demo.do_rebuild();demo.ui.set_text_area_value(demo.ids.ov_check,"");demo.snapshot_inputs();assert_eq!(demo.state.wizard_checks.len(),2);
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(2));demo.snapshot_inputs();demo.do_rebuild();demo.ui.set_text_input(demo.ids.ov_col_name,"id");demo.snapshot_inputs();assert!(!demo.state.table_wizard_names_valid());
            remove_selected_field_or_index(&mut demo);demo.do_rebuild();assert_eq!(demo.state.wizard_checks.len(),1);assert!(demo.state.wizard_checks[0].column.is_none());
            demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();conn.execute("UPDATE orders SET amount=0 WHERE id=7",[]).unwrap();assert!(conn.execute("UPDATE orders SET amount=20",[]).is_err());
            demo.state.open_create_table_wizard();demo.state.wizard_col_sel=1;demo.state.wizard_table_page=3;demo.do_rebuild();demo.handle_button(demo.ids.ov_check_add_column);demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_check_expr,"length(name)>0");demo.handle_button(demo.ids.ov_btn_ok);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(1,'')",[]).is_err());demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(1,'valid')",[]).unwrap();
        }
    }

    #[test]
    fn direct_inline_check_previews_and_saves_resolve_swapped_owners_and_expressions() {
        for schema in ["main","temp","aux.\"db"] {
            let mut state=AppState::new_with_demo();state.new_memory_db().unwrap();let table=dbmod::qualified_name(schema,"records");
            state.conn.as_ref().unwrap().execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {table}(a INTEGER CONSTRAINT ordered CHECK(records.a < b) CHECK(a>0),b INTEGER CHECK(b<99)); INSERT INTO {table}(rowid,a,b) VALUES(7,2,6);")).unwrap();
            state.refresh_schema().unwrap();let row=state.schema.iter().position(|object|object.schema==schema && object.name=="records").unwrap();state.select_schema_node(10_000+row as u64);state.open_modify_table_wizard().unwrap();
            state.wizard_checks[0].expression="records.a + 1 < b AND 'a' != 'b'".into();state.store_wizard_checks();
            state.wizard_cols[0].name="b".into();state.wizard_cols[1].name="a".into();state.wizard_table_name="swapped.\"名".into();
            let preview=state.table_wizard_sql().unwrap();state.apply_table_wizard().unwrap();let conn=state.conn.as_ref().unwrap();
            let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name=?1",dbmod::quote_ident(schema)),["swapped.\"名"],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::inline_checks(&preview).unwrap(),table_sql::inline_checks(&saved).unwrap());let checks=table_sql::inline_checks(&saved).unwrap();
            assert_eq!(checks[0].column.as_deref(),Some("b"));assert_eq!(checks[2].column.as_deref(),Some("a"));assert!(checks[0].expression.contains("'a'"));
            let target=dbmod::qualified_name(schema,"swapped.\"名");assert_eq!(conn.query_row(&format!("SELECT rowid,b,a FROM {target}"),[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,i64>(2)?))).unwrap(),(7,2,6));
            assert!(conn.execute(&format!("UPDATE {target} SET b=6"),[]).is_err());assert!(conn.execute(&format!("UPDATE {target} SET a=99"),[]).is_err());
        }
    }

    #[test]
    fn inline_foreign_key_page_edits_every_reference_and_syncs_named_fields() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer); demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1),(2); CREATE TABLE other_parent(id INTEGER PRIMARY KEY); INSERT INTO other_parent VALUES(1),(2); CREATE TABLE records(id INTEGER PRIMARY KEY,p INTEGER CONSTRAINT first_parent REFERENCES parent(id) CHECK(p>0) CONSTRAINT deadline DEFERRABLE INITIALLY DEFERRED NOT NULL CONSTRAINT second_parent REFERENCES other_parent(id),q INTEGER REFERENCES records(id) DEFERRABLE INITIALLY DEFERRED,FOREIGN KEY(q) REFERENCES records(id)); INSERT INTO records VALUES(7,1,NULL),(11,2,7);").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="records").unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_col_sel=1; demo.state.wizard_table_page=2; demo.do_rebuild();
            assert_eq!(demo.state.wizard_foreign_keys.len(),4); assert!(demo.state.wizard_foreign_keys[..3].iter().all(|key|key.inline));
            assert!(demo.ui.text_area_value(demo.ids.ov_fk_clause).unwrap().contains("CONSTRAINT deadline DEFERRABLE"));
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),demo.ui.text_area_value(demo.ids.ov_foreign_key));
            let first=demo.ui.text_area_value(demo.ids.ov_fk_clause).unwrap().to_string();
            demo.ui.set_text_input(demo.ids.ov_fk_name,"first.\"父键"); demo.ui.set_focus(Some(demo.ids.ov_fk_clause)); demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text("REFERENCES parent(id) CONSTRAINT new_deadline DEFERRABLE INITIALLY DEFERRED"); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_fk_clause)); demo.ui.key_input(UiKey::Undo); assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),Some(first.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fks_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_fk_clause)); demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text("REFERENCES other_parent(id) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED"); demo.snapshot_inputs();
            assert!(demo.ui.text_area_value(demo.ids.ov_foreign_key).unwrap().contains("new_deadline"));
            demo.ui.set_table_selected_row(demo.ids.ov_fks_table,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_fk_clause)); demo.ui.key_input(UiKey::Undo); assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_clause),Some(first.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
            demo.ui.set_scroll_offset(demo.ids.ov_fks_scroll,0.0,10000.0); demo.ui.select_text_area_range(demo.ids.ov_fk_clause,0..0); demo.ui.ime_preedit("中文",Some((3,6))); demo.ui.sync(&mut demo.scene);
            let candidate=demo.ui.ime_cursor_area(&demo.scene).unwrap(); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(candidate)); assert!(demo.ui.ime_commit("-- 中文\n")); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fks_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.button_enabled(demo.ids.ov_fk_down),Some(false)); demo.handle_button(demo.ids.ov_fk_down); assert_eq!(demo.state.wizard_foreign_key_sel,1);
            demo.handle_button(demo.ids.ov_fk_up); demo.do_rebuild(); assert!(demo.ui.text_area_value(demo.ids.ov_foreign_key).unwrap().contains("other_parent"));
            demo.handle_button(demo.ids.ov_fk_down); demo.do_rebuild(); assert!(demo.ui.text_area_value(demo.ids.ov_foreign_key).unwrap().contains("new_deadline"));
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,0); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,"REFERENCES parent(id) CONSTRAINT field_deadline DEFERRABLE INITIALLY DEFERRED"); demo.snapshot_inputs();
            assert!(demo.state.wizard_foreign_keys[0].clause.contains("field_deadline")); assert_eq!(demo.state.wizard_foreign_keys[0].name.as_deref(),Some("first.\"父键"));
            assert!(demo.state.wizard_foreign_keys[1].clause.contains("CASCADE"));
            demo.ui.set_text_input(demo.ids.ov_col_name,"parent_value"); demo.ui.set_text_input(demo.ids.ov_name,"orders"); demo.snapshot_inputs();
            assert!(demo.state.wizard_foreign_keys[0].columns.contains("parent_value")); assert!(demo.state.wizard_foreign_keys[2].clause.contains("\"orders\""));
            demo.state.wizard_foreign_key_sel=0; demo.state.wizard_table_page=2; demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_fk_columns,"parent_value,q"); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_some()); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_fk_columns),Some("parent_value,q"));
            demo.ui.set_text_area_value(demo.ids.ov_fk_columns,"\"parent_value\""); demo.snapshot_inputs();
            for phase in ["draft","saved"] {
                if phase=="saved" { demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg); demo.do_rebuild(); }
                else { demo.ui.set_scroll_offset(demo.ids.ov_fks_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene); }
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_INLINE_FK_QA_OUT") {
                    demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty); write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0; write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("Inline FK {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Inline FK GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn=demo.state.conn.as_ref().unwrap(); let sql:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='orders'",[],|row|row.get(0)).unwrap();
            let keys=table_sql::inline_foreign_keys(&sql).unwrap(); assert_eq!(keys.len(),3); assert!(keys[0].clause.contains("field_deadline")); assert!(keys[1].clause.contains("CASCADE"));
            conn.execute("INSERT INTO orders(id,parent_value) VALUES(13,999)",[]).unwrap(); assert!(conn.execute_batch("COMMIT").is_err()); conn.execute_batch("INSERT INTO parent VALUES(999); INSERT INTO other_parent VALUES(999);").unwrap();
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_col_sel=1; demo.state.wizard_foreign_key_sel=3; demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,""); demo.snapshot_inputs();
            assert_eq!(demo.state.wizard_foreign_keys.len(),3); assert_eq!(demo.state.wizard_foreign_key_sel,2); assert!(!demo.state.wizard_foreign_keys[2].inline);
            assert!(demo.ui.text_area_value(demo.ids.ov_foreign_key).unwrap().contains("other_parent"));
            demo.state.wizard_foreign_key_sel=0; demo.state.wizard_table_page=2; demo.do_rebuild();
            demo.handle_button(demo.ids.ov_fk_del); demo.do_rebuild(); assert_eq!(demo.state.wizard_foreign_keys.len(),2);
            demo.state.wizard_table_page=0; demo.do_rebuild(); assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(""));
            remove_selected_field_or_index(&mut demo); demo.do_rebuild(); assert_eq!(demo.state.wizard_foreign_keys.len(),2);
            demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            demo.state.open_create_table_wizard(); demo.do_rebuild(); demo.ui.set_text_area_value(demo.ids.ov_foreign_key,"REFERENCES parent(id)"); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(1234,'invalid')",[]).is_err());
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE composite_parent(a INTEGER,b INTEGER,PRIMARY KEY(a,b)); INSERT INTO composite_parent VALUES(1,2); CREATE TABLE converted(x INTEGER CONSTRAINT own REFERENCES parent(id),y INTEGER); INSERT INTO converted VALUES(1,2);").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="converted").unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_page=2; demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.ov_fk_columns,"x,y"); demo.ui.set_text_area_value(demo.ids.ov_fk_clause,"REFERENCES composite_parent(a,b) ON DELETE CASCADE"); demo.snapshot_inputs();
            assert!(!demo.state.wizard_foreign_keys[0].inline); demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap(); let sql:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='converted'",[],|row|row.get(0)).unwrap();
            assert!(table_sql::inline_foreign_keys(&sql).unwrap().is_empty()); assert_eq!(table_sql::table_foreign_keys(&sql).unwrap().len(),1);
            assert!(conn.execute("INSERT INTO converted VALUES(999,2)",[]).is_err()); conn.execute("DELETE FROM composite_parent WHERE a=1 AND b=2",[]).unwrap(); assert_eq!(dbmod::count_rows(conn,"converted").unwrap(),0);
            conn.execute_batch("CREATE TABLE ambiguous_scope(a INTEGER REFERENCES parent(id),b INTEGER REFERENCES other_parent(id)); INSERT INTO ambiguous_scope(rowid,a,b) VALUES(7,1,1);").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="ambiguous_scope").unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_col_sel=1; demo.do_rebuild();
            demo.ui.set_text_input(demo.ids.ov_col_name,"a"); demo.snapshot_inputs(); assert!(!demo.state.table_wizard_names_valid());
            remove_selected_field_or_index(&mut demo); demo.do_rebuild(); assert_eq!(demo.state.wizard_foreign_keys.len(),1);
            assert!(demo.state.wizard_foreign_keys[0].clause.contains("parent (id)")); assert!(!demo.state.wizard_foreign_keys[0].clause.contains("other_parent"));
            demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT rowid,a FROM ambiguous_scope",[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))).unwrap(),(7,1));
        }
    }

    #[test]
    fn direct_inline_foreign_key_previews_and_saves_resolve_owner_and_self_key_renames() {
        for schema in ["main","temp","aux.\"db"] {
            let mut state=state::AppState::new_with_demo(); state.new_memory_db().unwrap();
            let table=dbmod::qualified_name(schema,"records"); let parent=dbmod::qualified_name(schema,"parent");
            state.conn.as_ref().unwrap().execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {parent}(id INTEGER PRIMARY KEY); INSERT INTO {parent} VALUES(1); CREATE TABLE {table}(id INTEGER PRIMARY KEY,a INTEGER CONSTRAINT own REFERENCES records(id) CONSTRAINT deadline DEFERRABLE INITIALLY DEFERRED,b INTEGER REFERENCES parent(id)); INSERT INTO {table} VALUES(7,7,1);")).unwrap();
            state.refresh_schema().unwrap(); let row=state.schema.iter().position(|object|object.schema==schema && object.name=="records").unwrap();
            state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap(); state.store_wizard_foreign_keys();
            state.wizard_cols[0].name="key".into(); state.wizard_cols[1].name="self_ref".into(); state.wizard_cols[2].name="external_ref".into(); state.wizard_table_name="renamed.名".into();
            let preview=state.table_wizard_sql().unwrap(); state.apply_table_wizard().unwrap();
            let conn=state.conn.as_ref().unwrap(); let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='renamed.名'",dbmod::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::inline_foreign_keys(&preview).unwrap(),table_sql::inline_foreign_keys(&saved).unwrap());
            let keys=table_sql::inline_foreign_keys(&saved).unwrap(); assert!(keys[0].clause.contains("\"renamed.名\" (\"key\")") && keys[0].clause.contains("CONSTRAINT deadline DEFERRABLE"));
            assert_eq!(keys[1].clause,"REFERENCES parent (id)");
            let target=dbmod::qualified_name(schema,"renamed.名");
            assert_eq!(conn.query_row(&format!("SELECT rowid,\"key\",self_ref,external_ref FROM {target}"),[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,i64>(2)?,row.get::<_,i64>(3)?))).unwrap(),(7,7,7,1));
            assert!(conn.execute(&format!("INSERT INTO {target}(\"key\",external_ref) VALUES(8,999)"),[]).is_err());
            conn.execute(&format!("INSERT INTO {target}(\"key\",self_ref,external_ref) VALUES(8,999,1)"),[]).unwrap(); assert!(conn.execute_batch("COMMIT").is_err());
            conn.execute(&format!("INSERT INTO {target}(\"key\") VALUES(999)"),[]).unwrap();
        }
    }

    #[test]
    fn foreign_key_column_editor_keeps_sessions_and_saves_real_reference_semantics() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("COMMIT; PRAGMA foreign_keys=ON; BEGIN; CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE other_parent(id INTEGER PRIMARY KEY); INSERT INTO other_parent VALUES(1); CREATE TABLE records(value INTEGER CONSTRAINT named_fk REFERENCES parent(id),note TEXT); INSERT INTO records VALUES(1,'kept');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row=demo.state.schema.iter().position(|o|o.name=="records" && o.kind==ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64);
            demo.state.open_modify_table_wizard().unwrap(); demo.do_rebuild();
            let area=demo.ids.ov_foreign_key;
            assert_eq!(demo.ui.accessible_name(area).as_deref(),Some("Column foreign key clause"));
            let original=demo.ui.text_area_value(area).unwrap().to_string();
            demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,10000.0);
            demo.ui.sync(&mut demo.scene);
            let clause="REFERENCES other_parent(id)\nON UPDATE CASCADE ON DELETE CASCADE\nDEFERRABLE INITIALLY DEFERRED";
            demo.ui.set_focus(Some(area)); demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text(clause); demo.snapshot_inputs();
            demo.ui.select_text_area_range(area,0..0);
            demo.ui.ime_preedit("中文",Some((3,6))); demo.ui.sync(&mut demo.scene);
            let cursor=demo.ui.ime_cursor_area(&demo.scene).expect("visible FK editor has an IME candidate position");
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_foreign_key));
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(cursor));
            assert!(demo.ui.ime_commit("-- 中文\n"));
            demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(clause));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(""));
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,"REFERENCES parent(id)"); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_foreign_key)); demo.ui.key_input(UiKey::Undo);
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(clause));
            demo.handle_button(demo.ids.ov_btn_down); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(clause));
            demo.handle_button(demo.ids.ov_btn_up); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_btn_add); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(""));
            remove_selected_field_or_index(&mut demo); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some("REFERENCES parent(id)"));
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,""); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(0)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(clause));
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,"REFERENCES absent(id)");
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some());
            assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),1);
            demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some("REFERENCES absent(id)"));
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,clause); demo.snapshot_inputs();
            for phase in ["draft","saved"] {
                if phase=="saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                    demo.do_rebuild();
                } else { demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene); }
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_FOREIGN_KEY_QA_OUT") {
                    for area in [demo.ids.ov_check,demo.ids.ov_foreign_key] { sql_syntax::apply(&mut demo.ui,area,&demo.theme); }
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty();
                    let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("Foreign key {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Foreign key GPU comparison not_run: adapter unavailable"); }
                }
            }
            demo.state.open_modify_table_wizard().unwrap(); demo.do_rebuild();
            assert!(demo.ui.text_area_value(demo.ids.ov_foreign_key).unwrap().contains("CASCADE"));
            demo.state.conn.as_ref().unwrap().execute("DELETE FROM other_parent WHERE id=1",[]).unwrap();
            assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),0);
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,""); demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(dbmod::table_column_defs_in(demo.state.conn.as_ref().unwrap(),"main","records").unwrap()[0].foreign_key_clause.is_empty());
            demo.state.open_create_table_wizard(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_foreign_key),Some(""));
            demo.ui.set_text_area_value(demo.ids.ov_foreign_key,"REFERENCES parent(id)");
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(99,'invalid')",[]).is_err());
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(1,'valid')",[]).unwrap();
        }
    }

    #[test]
    fn index_constraint_page_keeps_sessions_syncs_fields_and_saves_real_keys() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo=Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.theme=ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,a TEXT CONSTRAINT single UNIQUE ON CONFLICT FAIL,b INTEGER,CONSTRAINT positive CHECK(b>0),CONSTRAINT pair UNIQUE(b,a)); INSERT INTO records VALUES(7,'west',1),(11,'east',2);").unwrap();
            demo.state.refresh_schema().unwrap();
            let row=demo.state.schema.iter().position(|object|object.schema=="main" && object.name=="records" && object.kind==ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap();
            demo.state.wizard_table_page=1; demo.state.wizard_key_sel=2; demo.do_rebuild();
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_key_columns).as_deref(),Some("Index constraint ordered columns SQL"));
            assert_eq!(demo.ui.tabs_selected(demo.ids.ov_table_tabs),Some(1));
            let original=demo.ui.text_area_value(demo.ids.ov_key_columns).unwrap().to_string();
            demo.ui.set_focus(Some(demo.ids.ov_key_columns)); demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text("b DESC, a COLLATE NOCASE ASC"); demo.ui.set_text_input(demo.ids.ov_key_name,"region.\"unique");
            demo.ui.set_select_selected(demo.ids.ov_key_conflict,Some(2)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_key_columns));
            demo.ui.key_input(UiKey::Undo); assert_eq!(demo.ui.text_area_value(demo.ids.ov_key_columns),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_keys_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_key_columns)); demo.ui.select_text_area_range(demo.ids.ov_key_columns,0..0); demo.ui.paste_into_focused_text("-- kept\n");
            demo.snapshot_inputs(); demo.ui.set_table_selected_row(demo.ids.ov_keys_table,Some(2)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_key_columns),Some("b DESC, a COLLATE NOCASE ASC"));
            demo.ui.set_table_selected_row(demo.ids.ov_keys_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_key_columns)); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_key_columns),Some("a"));
            demo.ui.set_table_selected_row(demo.ids.ov_keys_table,Some(2)); demo.snapshot_inputs(); demo.do_rebuild();
            let area=demo.ids.ov_key_columns; demo.ui.set_focus(Some(area)); demo.ui.set_scroll_offset(demo.ids.ov_keys_scroll,0.0,10000.0);
            demo.ui.select_text_area_range(area,0..0); demo.ui.ime_preedit("中文",Some((3,6))); demo.ui.sync(&mut demo.scene);
            let candidate=demo.ui.ime_cursor_area(&demo.scene).expect("key editor is visible"); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_key_columns)); assert_eq!(demo.ui.ime_cursor_area(&demo.scene),Some(candidate));
            assert!(demo.ui.ime_commit("-- 中文\n")); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
            demo.handle_button(demo.ids.ov_key_add_unique); demo.do_rebuild(); assert_eq!(demo.state.wizard_index_constraints.len(),4);
            demo.handle_button(demo.ids.ov_key_del); demo.do_rebuild(); assert_eq!(demo.state.wizard_index_constraints.len(),3);
            demo.handle_button(demo.ids.ov_key_up); demo.do_rebuild(); assert_eq!(demo.state.wizard_key_sel,1);
            assert_eq!(demo.state.wizard_index_constraints[1].name.as_deref(),Some("region.\"unique"));
            demo.handle_button(demo.ids.ov_key_down); demo.do_rebuild(); assert_eq!(demo.state.wizard_key_sel,2);
            demo.handle_button(demo.ids.ov_key_add_pk); assert_eq!(demo.state.wizard_index_constraints.len(),3);
            demo.ui.set_text_area_value(demo.ids.ov_key_columns,"b+1"); demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some()); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_key_columns),Some("b+1"));
            assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),2);
            demo.ui.set_text_area_value(demo.ids.ov_key_columns,"b DESC, a COLLATE NOCASE ASC"); demo.snapshot_inputs();
            demo.ui.set_tabs_selected(demo.ids.ov_table_tabs,0); demo.ui.set_focus(Some(demo.ids.ov_table_tabs)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_text_input(demo.ids.ov_col_name,"label"); demo.ui.set_text_input(demo.ids.ov_name,"orders"); demo.snapshot_inputs();
            assert!(demo.state.wizard_index_constraints[2].columns.contains("\"label\" COLLATE NOCASE ASC"));
            demo.ui.set_checked(demo.ids.ov_unique,false); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.wizard_index_constraints.len(),2); assert!(!demo.state.wizard_cols[1].unique);
            demo.ui.set_checked(demo.ids.ov_unique,true); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.wizard_index_constraints.len(),3);
            demo.state.wizard_key_sel=1; demo.state.wizard_table_page=1; demo.do_rebuild();
            let expected_keys=demo.state.wizard_index_constraints.clone();
            for phase in ["draft","saved"] {
                if phase=="saved" { demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg); demo.do_rebuild(); }
                else { demo.ui.set_scroll_offset(demo.ids.ov_keys_scroll,0.0,10000.0); demo.ui.sync(&mut demo.scene); }
                if let Ok(prefix)=std::env::var("NEXT_GUI_DB_BROWSER_TABLE_KEYS_QA_OUT") {
                    demo.scene.mark_full_dirty(1400.0,1000.0); let dirty=demo.scene.take_dirty(); let commands=encode_scene(&demo.scene,&dirty); let clear=clear_color(&demo.theme);
                    let mut soft=SoftRenderer::new(1400,1000,clear); soft.render(&commands,&dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"),soft.frame_pixels().unwrap().0,1400,1000).unwrap();
                    if let Ok(mut gpu)=GpuRenderer::try_new(1400,1000,clear) {
                        gpu.render(&commands,&dirty); let a=soft.frame_pixels().unwrap().0; let b=gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"),b,1400,1000).unwrap();
                        let delta=a.iter().zip(b).flat_map(|(a,b)|[0,8,16].map(|s|(((a>>s)&255)as i32-((b>>s)&255)as i32).abs())).max().unwrap();
                        assert!(delta<=2,"{theme_name}/{phase}: {delta}"); eprintln!("Table keys {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Table keys GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn=demo.state.conn.as_ref().unwrap();
            let definition:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='orders'",[],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::table_index_constraints(&definition).unwrap(),expected_keys);
            assert!(conn.execute("INSERT INTO orders(label,b) VALUES('west',99)",[]).is_err());
            conn.execute("INSERT INTO orders(label,b) VALUES('north',3)",[]).unwrap();
            assert_eq!(conn.query_row("SELECT id FROM orders WHERE label='north'",[],|row|row.get::<_,i64>(0)).unwrap(),12);
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_page=1; demo.state.wizard_key_sel=1;
            demo.win_w=800; demo.win_h=600; demo.do_rebuild();
            demo.ui.set_scroll_offset(demo.ids.ov_keys_scroll,0.0,10000.0); demo.ui.set_focus(Some(demo.ids.ov_table_tabs)); demo.ui.sync(&mut demo.scene);
            let remembered=demo.ui.scroll_offset(demo.ids.ov_keys_scroll).unwrap(); assert!(remembered.1>0.0);
            demo.snapshot_inputs(); demo.do_rebuild(); assert_eq!(demo.ui.scroll_offset(demo.ids.ov_keys_scroll),Some(remembered));
            let bounds=|id| demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
            let field=bounds(demo.ids.ov_key_columns); let viewport=bounds(demo.ids.ov_keys_scroll); let preview=bounds(demo.ids.ov_table_preview); let save=bounds(demo.ids.ov_btn_ok);
            assert!(field.y>=viewport.y && field.y+field.height<=viewport.y+viewport.height+1.0 && viewport.y+viewport.height<=preview.y && preview.y+preview.height<save.y);
            demo.state.open_create_table_wizard(); demo.state.wizard_table_page=1; demo.do_rebuild();
            assert_eq!(demo.state.wizard_index_constraints.len(),1); demo.handle_button(demo.ids.ov_key_del); demo.do_rebuild();
            assert!(!demo.state.wizard_cols.iter().any(|column|column.pk)); demo.handle_button(demo.ids.ov_key_add_pk); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_key_add_unique); demo.do_rebuild(); demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(demo.state.conn.as_ref().unwrap().execute("INSERT INTO new_table VALUES(1,'x'),(1,'y')",[]).is_err());
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE pinned(a TEXT UNIQUE,id INTEGER PRIMARY KEY DESC); INSERT INTO pinned(rowid,id,a) VALUES(7,11,'kept');").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="pinned").unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_page=1; demo.do_rebuild();
            assert!(table_sql::index_constraint_requires_inline(&demo.state.wizard_index_constraints[0]));
            assert_eq!(demo.ui.button_enabled(demo.ids.ov_key_down),Some(false));
            let original=demo.state.wizard_index_constraints.clone(); demo.handle_button(demo.ids.ov_key_down); assert_eq!(demo.state.wizard_index_constraints,original);
            demo.state.wizard_col_sel=1; demo.state.wizard_table_page=0; demo.do_rebuild(); demo.ui.set_checked(demo.ids.ov_ai,true); demo.snapshot_inputs(); demo.do_rebuild();
            demo.handle_button(demo.ids.ov_btn_ok); assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap(); conn.execute("INSERT INTO pinned(a) VALUES('new')",[]).unwrap();
            assert_eq!(conn.query_row("SELECT id FROM pinned WHERE a='new'",[],|row|row.get::<_,i64>(0)).unwrap(),12);
            conn.execute_batch("CREATE TABLE generated_key(g TEXT AS(id+1),id INTEGER); INSERT INTO generated_key(id) VALUES(7);").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="generated_key").unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.do_rebuild();
            demo.ui.set_text_input(demo.ids.ov_col_name,"computed"); demo.ui.set_checked(demo.ids.ov_ai,true); demo.snapshot_inputs();
            assert!(demo.state.error_msg.as_deref().is_some_and(|error|error.contains("Generated")));
            assert_eq!(demo.state.wizard_cols[0].name,"g"); assert_eq!(demo.state.wizard_cols[0].type_name,"TEXT");
            assert_eq!(demo.state.wizard_constraint_columns[0],"g"); assert!(demo.state.wizard_index_constraints.is_empty());
            assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type),Some("TEXT"));
            demo.snapshot_inputs(); assert_eq!(demo.state.wizard_cols[0].type_name,"TEXT"); assert!(!demo.state.wizard_cols[0].pk);
            demo.handle_button(demo.ids.ov_btn_cancel);
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE ambiguous(a TEXT,b INTEGER);").unwrap();
            demo.state.refresh_schema().unwrap(); let row=demo.state.schema.iter().position(|object|object.name=="ambiguous").unwrap();
            demo.state.select_schema_node(10_000+row as u64); demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_col_sel=1; demo.do_rebuild();
            demo.ui.set_text_input(demo.ids.ov_col_name,"a"); demo.ui.set_checked(demo.ids.ov_pk,true); demo.snapshot_inputs();
            assert!(demo.state.error_msg.as_deref().is_some_and(|error|error.contains("distinct column")));
            assert_eq!(demo.state.wizard_cols[1].name,"b"); assert!(!demo.state.wizard_cols[1].pk); assert!(demo.state.wizard_index_constraints.is_empty());
            demo.ui.set_text_input(demo.ids.ov_col_name,"c"); demo.ui.set_checked(demo.ids.ov_pk,true); demo.snapshot_inputs();
            assert_eq!(demo.state.wizard_constraint_columns[1],"c"); assert_eq!(demo.state.wizard_index_constraints[0].columns,"\"c\"");
        }
    }

    #[test]
    fn edited_index_constraint_previews_and_saves_follow_direct_column_swaps() {
        for schema in ["main","temp","aux.\"db"] {
            for edited in [false,true] {
            let mut state=state::AppState::new_with_demo(); state.new_memory_db().unwrap();
            let table=dbmod::qualified_name(schema,"records");
            state.conn.as_ref().unwrap().execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {table}(a TEXT,b INTEGER,CONSTRAINT identity PRIMARY KEY('b' DESC,'a' COLLATE NOCASE ASC) ON CONFLICT FAIL,CONSTRAINT pair UNIQUE('a','b')); INSERT INTO {table}(rowid,a,b) VALUES(7,'kept',2);")).unwrap();
            state.refresh_schema().unwrap(); let row=state.schema.iter().position(|object|object.schema==schema && object.name=="records").unwrap();
            state.select_schema_node(10_000+row as u64); state.open_modify_table_wizard().unwrap();
            if edited {
                state.wizard_index_constraints[0].name=Some("named.\"主键".into()); state.wizard_index_constraints[1].columns="a COLLATE BINARY DESC, b ASC".into();
                state.wizard_table_options.index_constraints=Some(state.wizard_index_constraints.clone());
            }
            state.wizard_cols[0].name="b".into(); state.wizard_cols[1].name="a".into(); state.wizard_table_name="swapped.名".into();
            let preview=state.table_wizard_sql().unwrap(); state.apply_table_wizard().unwrap();
            let conn=state.conn.as_ref().unwrap(); let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='swapped.名'",dbmod::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::table_index_constraints(&preview).unwrap(),table_sql::table_index_constraints(&saved).unwrap());
            let keys=table_sql::table_index_constraints(&saved).unwrap(); assert_eq!(keys[0].columns,"\"a\" DESC, \"b\" COLLATE NOCASE ASC");
            assert_eq!(keys[1].columns,if edited { "\"b\" COLLATE BINARY DESC, \"a\" ASC" } else { "\"b\", \"a\"" });
            let target=dbmod::qualified_name(schema,"swapped.名");
            assert_eq!(conn.query_row(&format!("SELECT rowid,b,a FROM {target}"),[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?))).unwrap(),(7,"kept".into(),2));
            assert!(conn.execute(&format!("INSERT INTO {target}(b,a) VALUES('KEPT',2)"),[]).is_err());
            }
        }
    }

    #[test]
    fn primary_key_conflict_control_saves_reopens_and_tracks_key_removal() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo = Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(code TEXT CONSTRAINT named_key PRIMARY KEY ON CONFLICT FAIL,note TEXT); INSERT INTO records VALUES('first','kept'),('second','key conflict policy');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema=="main" && o.name=="records" && o.kind==ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64);
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_pk_conflict).as_deref(),Some("Primary key conflict action"));
            assert_eq!(demo.ui.select_selected(demo.ids.ov_pk_conflict),Some(3));
            demo.ui.set_select_selected(demo.ids.ov_pk_conflict,Some(5));
            demo.ui.set_focus(Some(demo.ids.ov_pk_conflict));
            demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_pk_conflict));
            assert_eq!(demo.ui.select_selected(demo.ids.ov_pk_conflict),Some(5));
            demo.ui.set_text_input(demo.ids.ov_default,"0); DELETE FROM records; --");
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some());
            assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),2);
            demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_pk_conflict),Some(5));
            demo.ui.set_text_input(demo.ids.ov_default,""); demo.snapshot_inputs(); demo.clear_error();
            for phase in ["draft","saved"] {
                if phase=="saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_PK_CONFLICT_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("PK conflict {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("PK conflict GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn=demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.execute("INSERT INTO records VALUES('first','replacement')",[]).unwrap(),1);
            assert_eq!(conn.query_row("SELECT note FROM records WHERE code='first'",[],|r|r.get::<_,String>(0)).unwrap(),"replacement");
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_pk_conflict),Some(5));
            demo.ui.set_select_selected(demo.ids.ov_pk_conflict,Some(0));
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert!(demo.state.conn.as_ref().unwrap().execute("INSERT INTO records VALUES('first','must fail')",[]).is_err());
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            demo.ui.set_select_selected(demo.ids.ov_pk_conflict,Some(3)); demo.snapshot_inputs();
            demo.ui.set_checked(demo.ids.ov_pk,false); demo.snapshot_inputs();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_pk_conflict),Some(0));
            assert_eq!(demo.state.wizard_table_options.primary_key_conflict,None);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO records VALUES('first','key removed')",[]).unwrap();
            demo.state.open_create_table_wizard(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            assert_eq!(demo.ui.select_selected(demo.ids.ov_pk_conflict),Some(0));
            demo.ui.set_checked(demo.ids.ov_pk,false); demo.snapshot_inputs();
            demo.ui.set_select_selected(demo.ids.ov_pk_conflict,Some(4));
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.as_deref().is_some_and(|error|error.contains("primary-key column")));
            demo.do_rebuild();
            demo.ui.set_checked(demo.ids.ov_pk,true);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn=demo.state.conn.as_ref().unwrap();
            conn.execute("INSERT INTO new_table VALUES(1,'first')",[]).unwrap();
            assert_eq!(conn.execute("INSERT INTO new_table VALUES(1,'ignored')",[]).unwrap(),0);
        }
    }

    #[test]
    fn table_options_controls_validate_create_and_modify_and_reopen_actual_flags() {
        for (theme_name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            let mut demo = Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme,demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(code TEXT PRIMARY KEY,amount INTEGER,note TEXT); INSERT INTO records(rowid,code,amount,note) VALUES(7,'first',10,'kept'),(9,'second','not an integer','pending');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000+row as u64);
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_strict),Some(false));
            assert_eq!(demo.ui.is_checked(demo.ids.ov_without_rowid),Some(false));
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_strict).as_deref(),Some("Table STRICT option"));
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_without_rowid).as_deref(),Some("Table WITHOUT ROWID option"));
            demo.ui.set_checked(demo.ids.ov_strict,true);
            demo.ui.set_checked(demo.ids.ov_without_rowid,true);
            demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_strict),Some(true));
            assert_eq!(demo.ui.is_checked(demo.ids.ov_without_rowid),Some(true));
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some());
            let error = demo.state.error_msg.as_deref().unwrap();
            assert!(error.contains("\"main\".\"records\".amount") && !error.contains("__next_gui_rebuild"), "{error}");
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM records WHERE rowid IN(7,9)",[],|r|r.get::<_,i64>(0)).unwrap(),2);
            assert_eq!(dbmod::table_options_in(demo.state.conn.as_ref().unwrap(),"main","records").unwrap(),table_sql::TableOptions::default());
            demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_strict),Some(true));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1)); demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_autocomplete_value(demo.ids.ov_col_type,"ANY");
            demo.snapshot_inputs(); demo.do_rebuild(); demo.clear_error();
            for phase in ["draft","saved"] {
                if phase == "saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_OPTIONS_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Options {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Options GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(dbmod::table_options_in(conn,"main","records").unwrap(),table_sql::TableOptions { strict:true, without_rowid:true, primary_key_conflict:None, checks:None, inline_checks:None, foreign_keys:None, inline_foreign_keys:None, index_constraints:None });
            assert!(conn.prepare("SELECT rowid FROM records").is_err());
            assert_eq!(conn.query_row("SELECT amount FROM records WHERE code='second'",[],|r|r.get::<_,String>(0)).unwrap(),"not an integer");
            demo.state.open_modify_table_wizard().unwrap(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_strict),Some(true));
            assert_eq!(demo.ui.is_checked(demo.ids.ov_without_rowid),Some(true));
            demo.ui.set_checked(demo.ids.ov_strict,false);
            demo.ui.set_checked(demo.ids.ov_without_rowid,false);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT count(rowid) FROM records",[],|r|r.get::<_,i64>(0)).unwrap(),2);
            demo.state.open_create_table_wizard(); demo.state.wizard_table_advanced=true; demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_strict),Some(false));
            assert_eq!(demo.ui.is_checked(demo.ids.ov_without_rowid),Some(false));
            demo.ui.set_text_input(demo.ids.ov_name,"created");
            demo.ui.set_checked(demo.ids.ov_strict,true);
            demo.ui.set_checked(demo.ids.ov_without_rowid,true);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(dbmod::table_options_in(conn,"main","created").unwrap(),table_sql::TableOptions { strict:true, without_rowid:true, primary_key_conflict:None, checks:None, inline_checks:None, foreign_keys:None, inline_foreign_keys:None, index_constraints:None });
            assert!(conn.execute("INSERT INTO created VALUES('bad','text')",[]).is_err());
            conn.execute("INSERT INTO created VALUES(1,'valid')",[]).unwrap();
        }
    }

    #[test]
    fn sql_and_index_editors_keep_active_composition_through_form_rebuilds() {
        for index in [false,true] {
            let mut demo = Demo::new(1400,1000,PresentBackend::Softbuffer);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(value TEXT)").unwrap();
            demo.state.refresh_schema().unwrap();
            if index { demo.state.open_create_index_wizard(); }
            else { demo.state.tab = MainTab::ExecuteSql; }
            demo.do_rebuild();
            let area = if index { demo.ids.ov_idx_cols } else { demo.ids.sql_area };
            demo.ui.set_text_area_value(area,"value");
            demo.ui.set_focus(Some(area));
            demo.ui.select_text_area_range(area,0..0);
            demo.ui.ime_preedit("中文",Some((3,6)));
            demo.ui.sync(&mut demo.scene);
            let cursor = demo.ui.ime_cursor_area(&demo.scene);
            demo.snapshot_inputs(); demo.do_rebuild();
            let area = if index { demo.ids.ov_idx_cols } else { demo.ids.sql_area };
            assert_eq!(demo.ui.focused(),Some(area));
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene),cursor);
            assert!(demo.ui.ime_commit("中文"));
            assert_eq!(demo.ui.text_area_value(area),Some("中文value"));
            assert!(demo.ui.key_input(UiKey::Undo));
            assert_eq!(demo.ui.text_area_value(area),Some("value"));
        }
    }

    #[test]
    fn table_wizard_check_editor_keeps_column_sessions_and_saves_validated_constraints() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(value TEXT CONSTRAINT valid_value CHECK(length(value)>0),note TEXT); INSERT INTO records VALUES('Zulu','last'),('alpha','first');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000 + row as u64);
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.accessible_name(demo.ids.ov_check).as_deref(), Some("Column CHECK expression"));
            let original = demo.ui.text_area_value(demo.ids.ov_check).unwrap().to_string();
            let expression = "length(value) >= 3\nAND value != 'blocked'";
            demo.ui.set_focus(Some(demo.ids.ov_check));
            demo.ui.key_input(UiKey::SelectAll);
            assert!(demo.ui.paste_into_focused_text(expression));
            demo.snapshot_inputs();
            demo.ui.select_text_area_range(demo.ids.ov_check,0..0);
            demo.ui.ime_preedit("中文",Some((3,6)));
            demo.ui.sync(&mut demo.scene);
            let cursor = demo.ui.ime_cursor_area(&demo.scene);
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(),Some(demo.ids.ov_check));
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene),cursor);
            assert!(demo.ui.ime_commit("-- 中文\n"));
            assert!(demo.ui.text_area_value(demo.ids.ov_check).unwrap().starts_with("-- 中文"));
            demo.ui.key_input(UiKey::Undo);
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(expression));
            demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(1));
            demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(""));
            demo.ui.set_focus(Some(demo.ids.ov_check));
            demo.ui.paste_into_focused_text("note IS NULL OR length(note)>0");
            demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(0));
            demo.snapshot_inputs(); demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.ov_check));
            assert!(demo.ui.key_input(UiKey::Undo));
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(original.as_str()));
            demo.ui.key_input(UiKey::Redo);
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(expression));
            demo.handle_button(demo.ids.ov_btn_down); demo.do_rebuild();
            assert_eq!(demo.state.wizard_col_sel,1);
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(expression));
            demo.handle_button(demo.ids.ov_btn_up); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(expression));
            demo.handle_button(demo.ids.ov_btn_add); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(""));
            demo.ui.set_text_area_value(demo.ids.ov_check,"col IS NULL");
            remove_selected_field_or_index(&mut demo); demo.do_rebuild();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table,Some(0));
            demo.snapshot_inputs(); demo.do_rebuild();
            for invalid in ["length(value)>20", "1)); DELETE FROM records; --"] {
                demo.ui.set_text_area_value(demo.ids.ov_check,invalid);
                demo.handle_button(demo.ids.ov_btn_ok);
                assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some());
                assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),2);
                demo.do_rebuild();
                assert_eq!(demo.ui.text_area_value(demo.ids.ov_check),Some(invalid));
            }
            demo.ui.set_text_area_value(demo.ids.ov_check,expression);
            demo.snapshot_inputs(); demo.clear_error();
            demo.ui.set_scroll_offset(demo.ids.ov_scroll,0.0,10000.0);
            demo.snapshot_inputs(); demo.do_rebuild();
            for phase in ["draft", "saved"] {
                if phase == "saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_CHECK_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Check {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Check GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            assert!(conn.execute("INSERT INTO records VALUES('X','valid')",[]).is_err());
            assert!(conn.execute("INSERT INTO records VALUES('blocked','valid')",[]).is_err());
            assert!(conn.execute("INSERT INTO records VALUES('valid','')",[]).is_err());
            conn.execute("INSERT INTO records VALUES('valid','valid')",[]).unwrap();
            demo.state.open_modify_table_wizard().unwrap(); demo.do_rebuild();
            assert!(demo.ui.text_area_value(demo.ids.ov_check).unwrap().contains("blocked"));
            demo.ui.set_text_area_value(demo.ids.ov_check,"");
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO records VALUES('X','valid')",[]).unwrap();
        }
    }

    #[test]
    fn table_wizard_collation_controls_keep_sessions_and_save_comparison_rules() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(value TEXT UNIQUE,note TEXT); INSERT INTO records VALUES('Zulu','last with NOCASE'),('alpha','first with NOCASE');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000 + row as u64);
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert!(demo.state.column_collation_names().contains(&"NOCASE".into()));
            demo.ui.set_focus(Some(demo.ids.ov_collation));
            demo.ui.key_input(UiKey::SelectAll);
            assert!(demo.ui.paste_into_focused_text("NOCASE"));
            demo.snapshot_inputs();
            demo.ui.key_input(UiKey::Home);
            demo.ui.ime_preedit("中文", Some((3,3)));
            demo.ui.sync(&mut demo.scene);
            let cursor = demo.ui.ime_cursor_area(&demo.scene).unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.ov_collation));
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene), Some(cursor));
            assert!(demo.ui.ime_commit("custom"));
            demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text("NOCASE");
            demo.snapshot_inputs();
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_collation), Some(""));
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(0));
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_collation), Some("NOCASE"));
            for phase in ["draft", "saved"] {
                if phase == "saved" {
                    demo.ui.set_autocomplete_value(demo.ids.ov_collation, "unknown collation");
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_some() && demo.state.overlay.is_some());
                    assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "records").unwrap(), 2);
                    demo.do_rebuild();
                    assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_collation), Some("unknown collation"));
                    demo.ui.set_autocomplete_value(demo.ids.ov_collation, "NOCASE");
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_COLLATION_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Collation {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Collation GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.query_row("SELECT value FROM records ORDER BY value LIMIT 1", [], |r| r.get::<_,String>(0)).unwrap(), "alpha");
            assert!(conn.execute("INSERT INTO records(value) VALUES('ALPHA')", []).is_err());
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_collation), Some("NOCASE"));
            demo.ui.set_autocomplete_value(demo.ids.ov_collation, "");
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO records(value) VALUES('ALPHA')", []).unwrap();
        }
    }

    #[test]
    fn table_wizard_autoincrement_controls_match_type_and_key_rules_and_save() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id TEXT,code TEXT PRIMARY KEY,note TEXT); INSERT INTO records(rowid,id,code,note) VALUES(7,NULL,'first','assigned ID'),(9,'2','second','kept ID');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000 + row as u64);
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_ai), Some(false));
            demo.ui.set_checked(demo.ids.ov_ai, true);
            demo.snapshot_inputs();
            assert_eq!(demo.ui.autocomplete_value(demo.ids.ov_col_type), Some("INTEGER"));
            assert_eq!(demo.ui.is_checked(demo.ids.ov_pk), Some(true));
            assert!(demo.state.wizard_cols[0].auto_increment && demo.state.wizard_cols[0].pk);
            assert!(!demo.state.wizard_cols[1].pk);
            demo.ui.set_checked(demo.ids.ov_pk, false);
            demo.snapshot_inputs();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_ai), Some(false));
            demo.ui.set_checked(demo.ids.ov_ai, true);
            demo.snapshot_inputs();
            demo.do_rebuild();
            for phase in ["draft", "saved"] {
                if phase == "saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                    assert!(demo.state.overlay.is_none());
                    demo.state.tab = MainTab::Browse;
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_AI_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Autoincrement {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Autoincrement GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.query_row("SELECT id FROM records WHERE code='first'", [], |r| r.get::<_,i64>(0)).unwrap(), 3);
            assert_eq!(conn.query_row("SELECT id FROM records WHERE code='second'", [], |r| r.get::<_,i64>(0)).unwrap(), 2);
            conn.execute_batch("INSERT INTO records DEFAULT VALUES; DELETE FROM records WHERE id=4;").unwrap();
            conn.execute("INSERT INTO records DEFAULT VALUES", []).unwrap();
            assert_eq!(conn.query_row("SELECT max(id) FROM records", [], |r| r.get::<_,i64>(0)).unwrap(), 5);
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_ai), Some(true));
            demo.ui.set_checked(demo.ids.ov_ai, false);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            let columns = dbmod::table_column_defs_in(demo.state.conn.as_ref().unwrap(), "main", "records").unwrap();
            assert!(columns[0].pk && !columns[0].auto_increment);
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            demo.ui.set_checked(demo.ids.ov_ai, true);
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
            demo.snapshot_inputs();
            demo.do_rebuild();
            demo.ui.set_checked(demo.ids.ov_pk, true);
            demo.snapshot_inputs();
            assert!(!demo.state.wizard_cols[0].auto_increment);
            assert!(demo.state.wizard_cols[0].pk && demo.state.wizard_cols[1].pk);
        }
    }

    #[test]
    fn table_wizard_primary_key_controls_save_and_remove_composite_keys() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(label TEXT,revision INTEGER,note TEXT); INSERT INTO records(rowid,label,revision,note) VALUES(7,'first',1,'kept'),(9,'second',2,'kept');").unwrap();
            demo.state.refresh_schema().unwrap();
            let row = demo.state.schema.iter().position(|o| o.schema == "main" && o.name == "records" && o.kind == ObjectKind::Table).unwrap();
            demo.state.select_schema_node(10_000 + row as u64);
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            demo.ui.set_checked(demo.ids.ov_pk, true);
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert!(demo.state.wizard_cols[0].pk);
            demo.ui.set_checked(demo.ids.ov_pk, true);
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert!(demo.state.wizard_cols[1].pk);
            for phase in ["draft", "saved"] {
                if phase == "saved" {
                    demo.handle_button(demo.ids.ov_btn_ok);
                    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                    assert!(demo.state.overlay.is_none());
                    demo.do_rebuild();
                }
                if let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_PRIMARY_QA_OUT") {
                    demo.ui.sync(&mut demo.scene);
                    demo.scene.mark_full_dirty(1400.0, 1000.0);
                    let dirty = demo.scene.take_dirty();
                    let commands = encode_scene(&demo.scene, &dirty);
                    let clear = clear_color(&demo.theme);
                    let mut soft = SoftRenderer::new(1400, 1000, clear);
                    soft.render(&commands, &dirty);
                    write_ppm(&format!("{prefix}-{theme_name}-{phase}.ppm"), soft.frame_pixels().unwrap().0, 1400, 1000).unwrap();
                    if let Ok(mut gpu) = GpuRenderer::try_new(1400, 1000, clear) {
                        gpu.render(&commands, &dirty);
                        let a = soft.frame_pixels().unwrap().0;
                        let b = gpu.frame_pixels().unwrap().0;
                        write_ppm(&format!("{prefix}-{theme_name}-{phase}-metal.ppm"), b, 1400, 1000).unwrap();
                        let delta = a.iter().zip(b).flat_map(|(a,b)| [0,8,16].map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())).max().unwrap();
                        assert!(delta <= 2, "{theme_name}/{phase}: full-frame channel delta {delta}");
                        eprintln!("Primary key {theme_name}/{phase}: full-frame Soft/Metal delta {delta}");
                    } else { eprintln!("Primary key GPU comparison not_run: adapter unavailable"); }
                }
            }
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(conn.query_row("SELECT count(*) FROM records WHERE rowid IN(7,9)", [], |r| r.get::<_,i64>(0)).unwrap(), 2);
            assert!(conn.execute("INSERT OR ABORT INTO records VALUES('first',1,'duplicate')", []).is_err());
            demo.state.open_modify_table_wizard().unwrap();
            demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_pk), Some(true));
            demo.ui.set_checked(demo.ids.ov_pk, false);
            demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.is_checked(demo.ids.ov_pk), Some(true));
            demo.ui.set_checked(demo.ids.ov_pk, false);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            assert!(!dbmod::table_columns(demo.state.conn.as_ref().unwrap(), "records").unwrap().iter().any(|column| column.pk));
            demo.state.conn.as_ref().unwrap().execute("INSERT INTO records VALUES('first',1,'duplicate')", []).unwrap();
        }
    }

    #[test]
    fn table_wizard_selection_keeps_each_columns_definition() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.open_create_table_wizard();
        demo.do_rebuild();
        demo.ui.set_text_input(demo.ids.ov_col_name, "record_id");
        demo.ui.set_table_selected_row(demo.ids.ov_fields_table, Some(1));
        demo.snapshot_inputs();
        assert_eq!(demo.state.wizard_cols[0].name, "record_id");
        assert_eq!(demo.state.wizard_cols[1].name, "name");
        assert_eq!(demo.state.wizard_cols[1].type_name, "TEXT");
        assert!(!demo.state.wizard_cols[1].pk);
        demo.do_rebuild();
        assert_eq!(demo.ui.table_selected_row(demo.ids.ov_fields_table), Some(1));
        assert_eq!(demo.ui.text_input_value(demo.ids.ov_col_name), Some("name"));
    }

    #[test]
    fn sql_editor_word_shortcuts_move_to_identifier_boundaries() {
        let mut demo = Demo::new(1200, 800, PresentBackend::Softbuffer);
        demo.state.tab = MainTab::ExecuteSql;
        let sql = "SELECT 中文_name, other_name";
        demo.state.sql_tabs[0] = sql.into();
        demo.do_rebuild();
        let area = demo.ids.sql_area;
        demo.ui.set_focus(Some(area));
        let end = sql.chars().count();
        let start = sql[..sql.find("other_name").unwrap()].chars().count();
        demo.ui.select_text_area_range(area, end..end);
        let word = if cfg!(target_os = "macos") { Modifiers { alt: true, ..Modifiers::default() } }
            else { Modifiers { ctrl: true, ..Modifiers::default() } };
        demo.ui.key_input(map_key(KeyCode::Left, word).unwrap());
        assert_eq!(demo.ui.text_area_caret(area), Some(start));
        demo.ui.key_input(map_key(KeyCode::Right, Modifiers { shift: true, ..word }).unwrap());
        assert_eq!(demo.ui.text_area_selected_text(area).as_deref(), Some("other_name"));
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_WORD_QA_OUT") {
            demo.ui.sync(&mut demo.scene);
            demo.scene.mark_full_dirty(1200.0, 800.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(1200, 800, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 1200, 800).unwrap();
        }
        demo.input_key_with_sql_completions(map_key(KeyCode::Backspace, word).unwrap());
        assert_eq!(demo.ui.text_area_value(area), Some("SELECT 中文_name, "));
        demo.ui.key_input(UiKey::Undo);
        assert_eq!(demo.ui.text_area_value(area), Some(sql));
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some(sql));
        assert_eq!(demo.ui.text_area_selected_text(demo.ids.sql_area).as_deref(), Some("other_name"));
        assert!(demo.state.sql_result.is_none());
    }

    #[test]
    fn command_shortcuts_do_not_insert_literal_letters() {
        let command = Modifiers {
            meta: true,
            ..Modifiers::default()
        };
        assert_eq!(map_key(KeyCode::Char('z'), command), Some(UiKey::Undo));
        assert_eq!(map_key(KeyCode::Char('s'), command), None);
        assert_eq!(map_key(KeyCode::Char('a'), command), Some(UiKey::SelectAll));
        assert_eq!(
            map_key(KeyCode::Space, Modifiers::default()),
            Some(UiKey::Space)
        );
    }

    #[test]
    fn browse_rebuild_preserves_resized_columns_and_viewport() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(
            "CREATE TABLE t(a, b, c, d, e, f); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100) INSERT INTO t SELECT x,x,x,x,x,x FROM n; COMMIT; BEGIN;"
        ).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.refresh_browse().unwrap();
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        for column in 0..6 {
            demo.ui
                .set_table_column_width(demo.ids.browse_table, column, 240.0);
        }
        demo.ui
            .set_table_scroll_offset(demo.ids.browse_table, 150.0, 200.0);
        demo.snapshot_inputs();
        demo.state.browse_mut().sel_row = Some(10);
        demo.do_rebuild();
        assert_eq!(
            demo.ui.table_column_widths(demo.ids.browse_table),
            Some(vec![240.0; 6])
        );
        assert_eq!(
            demo.ui.table_scroll_offset(demo.ids.browse_table),
            Some((150.0, 200.0))
        );
        let table_bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.browse_table)).unwrap().bounds;
        let field = demo.ids.browse_col_filters[0];
        let input_node = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(field)).unwrap();
        assert_eq!(input_node.bounds.width, 236.0);
        assert!((input_node.bounds.x + 150.0 - table_bounds.x - next_gui_ui::TableStyle::from_theme(&demo.theme).padding - 2.0).abs() < 0.01);
        demo.ui.set_table_frozen_cols(demo.ids.browse_table, 1);
        demo.ui.sync(&mut demo.scene);
        let input_node = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(field)).unwrap();
        assert!((input_node.bounds.x - table_bounds.x - next_gui_ui::TableStyle::from_theme(&demo.theme).padding - 2.0).abs() < 0.01);
        if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
            demo.scene.mark_full_dirty(1400.0, 860.0);
            let dirty = demo.scene.take_dirty();
            let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
            renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
            write_ppm(&path, renderer.frame_pixels().unwrap().0, 1400, 860).unwrap();
        }

        // Sorting uses the SQL identifier, not the decorated heading label.
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE arrows(\"score ▲\"); INSERT INTO arrows VALUES(2),(1);")
            .unwrap();
        demo.state.browse_mut().table = Some("arrows".into());
        demo.state.browse_mut().result = None;
        demo.state.browse_mut().column_widths.clear();
        demo.state.browse_mut().scroll = (0.0, 0.0);
        demo.state.refresh_browse().unwrap();
        demo.do_rebuild();
        let bounds = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.browse_table))
            .unwrap()
            .bounds;
        demo.ui
            .pointer_pressed(&mut demo.scene, bounds.x + 30.0, bounds.y + 10.0);
        demo.poll_header_sort(false);
        assert_eq!(demo.state.browse().order_col.as_deref(), Some("score ▲"));
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][0], "1");
        demo.do_rebuild();
        let bounds = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.browse_table))
            .unwrap()
            .bounds;
        demo.ui
            .pointer_pressed(&mut demo.scene, bounds.x + 30.0, bounds.y + 10.0);
        demo.poll_header_sort(false);
        assert!(demo.state.browse().order_desc);
        demo.do_rebuild();
        let bounds = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.browse_table))
            .unwrap()
            .bounds;
        demo.ui
            .pointer_pressed(&mut demo.scene, bounds.x + 30.0, bounds.y + 10.0);
        demo.poll_header_sort(false);
        assert!(!demo.state.browse().order_desc);
        demo.do_rebuild();
        demo.handle_button(demo.ids.btn_browse_clear_sort);
        assert!(demo.state.browse().ordering().is_empty());
    }

    #[test]
    fn rollback_conflict_in_record_dialog_restores_unwritten_edit_transaction() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(code TEXT PRIMARY KEY ON CONFLICT ROLLBACK,note TEXT); INSERT INTO records VALUES('first','committed');").unwrap();
        demo.state.write_changes().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("INSERT INTO records VALUES('pending','unwritten'); CREATE TABLE pending_schema(v);").unwrap();
        demo.state.mark_dirty();
        demo.state.refresh_schema().unwrap();
        demo.state.browse_mut().table = Some("records".into());
        demo.state.refresh_browse().unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.open_insert_row().unwrap(); demo.do_rebuild();
        for (i,value) in [(0,"first"),(1,"retained input")] {
            demo.ui.set_select_selected(demo.ids.insert_modes[i],Some(2));
            demo.ui.set_text_input(demo.ids.insert_values[i],value);
        }
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.as_deref().is_some_and(|e|e.contains("UNIQUE constraint failed")));
        assert!(!demo.state.conn.as_ref().unwrap().is_autocommit(), "failed ROLLBACK conflict left later GUI edits in autocommit");
        assert!(!demo.state.dirty);
        assert!(!demo.state.schema.iter().any(|o|o.name=="pending_schema"));
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows.len(),1);
        assert_eq!(demo.state.overlay,Some(OverlayKind::InsertRow));
        demo.do_rebuild();
        assert_eq!(demo.ui.text_input_value(demo.ids.insert_values[1]),Some("retained input"));
        demo.ui.set_text_input(demo.ids.insert_values[0],"corrected");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
        assert!(demo.state.dirty);
        assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),2);
        demo.state.revert_changes().unwrap();
        assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"records").unwrap(),1,"corrected record was committed before Write Changes");
    }

    #[test]
    fn record_dialog_buttons_fit_and_drop_preference_is_applied() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE t(v TEXT NOT NULL); COMMIT; BEGIN;")
            .unwrap();
        demo.state.browse_mut().table = Some("t".into());
        demo.state.tab = MainTab::Browse;
        demo.state.open_insert_row().unwrap();
        demo.do_rebuild();
        for button in [demo.ids.ov_btn_ok, demo.ids.ov_btn_cancel] {
            let bounds = demo
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|n| demo.ui.widget_from_node(n.id) == Some(button))
                .unwrap()
                .bounds;
            assert!(
                bounds.y + bounds.height <= 820.0,
                "dialog action is behind the status bar"
            );
            assert_eq!(
                demo.ui
                    .hit_widget(&demo.scene, bounds.x + 5.0, bounds.y + 5.0),
                Some(button)
            );
        }
        demo.ui
            .set_select_selected(demo.ids.insert_modes[0], Some(2));
        demo.ui
            .set_text_input(demo.ids.insert_values[0], "required value");
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.overlay.is_none());
        assert_eq!(
            dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "t").unwrap(),
            1
        );
        demo.state.prefs.confirm_drops = false;
        demo.state.selected = Some(crate::state::SelectedObject {
            schema: "main".into(),
            kind: ObjectKind::Table,
            name: "t".into(),
        });
        demo.handle_menu(mid::EDIT_DELETE);
        assert!(!demo.state.confirm_open);
        assert!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "t").is_err());
        demo.state.revert_changes().unwrap();
        assert_eq!(
            dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "t").unwrap(),
            0
        );
    }

    #[test]
    fn tab_switch_pixels_match_fresh_page_without_stale_descendants() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        for tab in [1, 2, 0, 1] {
            demo.ui.set_tabs_selected(demo.ids.tabs, tab);
            demo.snapshot_inputs();
            demo.poll_selection_changes();
            demo.do_rebuild();
            assert_eq!(demo.state.tab, MainTab::from_index(tab));
            let mut fresh = build_ui(demo.font.clone(), demo.theme, 1400, 860, &demo.state);
            let render = |scene: &mut Scene| {
                scene.mark_full_dirty(1400.0, 860.0);
                let dirty = scene.take_dirty();
                let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
                renderer.render(&encode_scene(scene, &dirty), &dirty);
                renderer.frame_pixels().unwrap().0.to_vec()
            };
            assert!(
                render(&mut demo.scene) == render(&mut fresh.scene),
                "stale pixels on tab {tab}"
            );
        }
    }

    #[test]
    fn sql_buttons_rebuild_and_close_keep_user_state() {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES('saved'); COMMIT; BEGIN;")
            .unwrap();
        demo.state.tab = MainTab::ExecuteSql;
        demo.do_rebuild();
        demo.ui
            .set_text_area_value(demo.ids.sql_area, "SELECT '中文';\nSELECT 42;");
        demo.handle_button(demo.ids.btn_sql_new_tab);
        demo.do_rebuild();
        assert_eq!(demo.state.sql_tabs[0], "SELECT '中文';\nSELECT 42;");
        assert_eq!(
            demo.ui.text_area_value(demo.ids.sql_area),
            Some("")
        );
        demo.ui.set_text_area_value(
            demo.ids.sql_area,
            "INSERT INTO t VALUES('pending'); SELECT count(*) FROM t;",
        );
        demo.handle_button(demo.ids.btn_sql_run);
        assert!(!demo.can_close());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !demo.state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        demo.do_rebuild();
        assert_eq!(demo.state.sql_result.as_ref().unwrap().rows[0][0], "2");
        assert!(!demo.can_close());
        demo.handle_menu(mid::FILE_EXIT);
        demo.do_rebuild();
        assert!(demo.ui.modal_open(demo.ids.modal).unwrap());
        assert!(!demo.exit_requested);
        demo.state.revert_changes().unwrap();
        assert!(!demo.can_close(), "SQL drafts survive a database rollback");
        demo.state.checkpoint_project();
        assert!(demo.can_close());
        assert_eq!(
            dbmod::count_rows(demo.state.conn.as_ref().unwrap(), "t").unwrap(),
            1
        );
        demo.state.tab = MainTab::Browse;
        demo.state.confirm_open = false;
        demo.do_rebuild();
        assert_eq!(
            demo.ui.tabs_selected(demo.ids.tabs),
            Some(MainTab::Browse.index())
        );
        demo.state.browse_mut().table = Some("t".into());
        demo.state.refresh_browse().unwrap();
        demo.state.toggle_browse_view();
        demo.do_rebuild();
        demo.handle_button(demo.ids.form_fields[0]);
        demo.do_rebuild();
        assert_eq!(demo.state.edit_col, "v");
        assert_eq!(demo.state.edit_value, "saved");
        demo.ui.set_text_area_value(demo.ids.edit_value, "form edit");
        demo.handle_button(demo.ids.btn_edit_cell);
        assert_eq!(
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT v FROM t", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "form edit"
        );
        demo.state.revert_changes().unwrap();
    }
}

#[cfg(test)]
mod browse_sort_tests;

#[cfg(test)]
mod browse_navigation_tests;

#[cfg(test)]
mod sql_result_tests;
