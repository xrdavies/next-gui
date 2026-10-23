//! DB Browser for SQLite–style example (`e28_db_browser`) — DB4S chrome parity.
//!
//! Soft↔GPU: Space when nothing focused. Theme: toolbar / View menu.
//! `NEXT_GUI_DB_BROWSER_QA=1` dumps a PPM after first paint then exits.

mod db;
mod file_dlg;
mod state;
mod ui_build;

use std::path::PathBuf;

use next_gui_platform::{
    AppCallbacks, Event, KeyCode, Modifiers, PointerButton, PresentBackend, WindowAttrs, WindowCtx,
};
use next_gui_render::{encode_scene, Renderer};
use next_gui_render_gpu::{GpuPresentPath, GpuRenderer};
use next_gui_render_soft::SoftRenderer;
use next_gui_scene::{Color, Scene};
use next_gui_text::{load_default_cjk_font, Font};
use next_gui_ui::{ModalAction, PropertyValue, Theme, ThemeMode, Ui, UiKey, WidgetId};

use crate::db::{self as dbmod};
use crate::state::{AppState, ConfirmKind, DockKind, MainTab};
use crate::ui_build::{build_ui, clear_color, mid, BuiltUi};

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

    fn render(&mut self, cmds: &[next_gui_render::DrawCommand], dirty: &next_gui_scene::DirtyRegion) {
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
    if modifiers.commandish() {
        if let KeyCode::Char(c) = key {
            if c == 'a' || c == 'A' {
                return Some(UiKey::SelectAll);
            }
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
        KeyCode::Enter => UiKey::Enter,
        KeyCode::Space => UiKey::Space,
        KeyCode::Escape => UiKey::Escape,
        KeyCode::Char(c) => UiKey::Char(c),
        KeyCode::Other(_) => return None,
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
    last_tree_sel: Option<u64>,
    last_tab: usize,
    last_browse_sel: Option<usize>,
    last_browse_row: Option<usize>,
    exit_requested: bool,
}

#[allow(dead_code)]
struct BuiltUiIds {
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
    new_table_name: WidgetId,
    new_table_cols: WidgetId,
    new_index_name: WidgetId,
    new_index_on: WidgetId,
    browse_select: WidgetId,
    browse_filter: WidgetId,
    btn_browse_go: WidgetId,
    btn_add_row: WidgetId,
    btn_del_row: WidgetId,
    browse_table: WidgetId,
    pragma_grid: WidgetId,
    pragma_value: WidgetId,
    btn_pragma_apply: WidgetId,
    sql_area: WidgetId,
    btn_sql_run: WidgetId,
    btn_sql_new_tab: WidgetId,
    btn_sql_open: WidgetId,
    btn_sql_save: WidgetId,
    sql_table: WidgetId,
    dock_tabs: WidgetId,
    edit_col: WidgetId,
    edit_value: WidgetId,
    btn_edit_cell: WidgetId,
    chart_select: WidgetId,
    chart_col: WidgetId,
    btn_chart: WidgetId,
    modal: WidgetId,
}

impl From<&BuiltUi> for BuiltUiIds {
    fn from(b: &BuiltUi) -> Self {
        Self {
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
            new_table_name: b.new_table_name,
            new_table_cols: b.new_table_cols,
            new_index_name: b.new_index_name,
            new_index_on: b.new_index_on,
            browse_select: b.browse_select,
            browse_filter: b.browse_filter,
            btn_browse_go: b.btn_browse_go,
            btn_add_row: b.btn_add_row,
            btn_del_row: b.btn_del_row,
            browse_table: b.browse_table,
            pragma_grid: b.pragma_grid,
            pragma_value: b.pragma_value,
            btn_pragma_apply: b.btn_pragma_apply,
            sql_area: b.sql_area,
            btn_sql_run: b.btn_sql_run,
            btn_sql_new_tab: b.btn_sql_new_tab,
            btn_sql_open: b.btn_sql_open,
            btn_sql_save: b.btn_sql_save,
            sql_table: b.sql_table,
            dock_tabs: b.dock_tabs,
            edit_col: b.edit_col,
            edit_value: b.edit_value,
            btn_edit_cell: b.btn_edit_cell,
            chart_select: b.chart_select,
            chart_col: b.chart_col,
            btn_chart: b.btn_chart,
            modal: b.modal,
        }
    }
}

impl Demo {
    fn new(width: u32, height: u32, window_backend: PresentBackend) -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            eprintln!("font error: {e}");
            std::process::exit(1);
        });
        let theme = Theme::dark();
        let mut state = AppState::new_with_demo();
        if let Ok(tab) = std::env::var("NEXT_GUI_DB_BROWSER_QA_TAB") {
            state.tab = match tab.to_ascii_lowercase().as_str() {
                "browse" => MainTab::Browse,
                "pragmas" => MainTab::Pragmas,
                "sql" | "execute" => MainTab::ExecuteSql,
                // legacy aliases → docks / structure
                "charts" | "plot" => {
                    state.dock_plot = true;
                    state.dock_focus = DockKind::Plot;
                    MainTab::Browse
                }
                "log" => {
                    state.dock_log = true;
                    state.dock_focus = DockKind::Log;
                    MainTab::Structure
                }
                _ => MainTab::Structure,
            };
        }
        let built = build_ui(font.clone(), theme, width, height, &state);
        let clear = clear_color(&theme);
        let qa_exit = std::env::var_os("NEXT_GUI_DB_BROWSER_QA").is_some();
        Self {
            font,
            theme,
            state,
            ids: BuiltUiIds::from(&built),
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
            last_tree_sel: None,
            last_tab: 0,
            last_browse_sel: None,
            last_browse_row: None,
            exit_requested: false,
        }
    }

    fn snapshot_inputs(&mut self) {
        if let Some(v) = self.ui.text_area_value(self.ids.ddl_area) {
            self.state.ddl_draft = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.new_table_name) {
            self.state.new_table_name = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.new_table_cols) {
            self.state.new_table_cols = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.new_index_name) {
            self.state.new_index_name = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.new_index_on) {
            self.state.new_index_on = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.browse_filter) {
            self.state.browse_filter = v.to_string();
        }
        if self.ids.edit_col.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.edit_col) {
                self.state.edit_col = v.to_string();
            }
        }
        if self.ids.edit_value.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.edit_value) {
                self.state.edit_value = v.to_string();
            }
        }
        if let Some(v) = self.ui.text_area_value(self.ids.sql_area) {
            *self.state.sql_editor_mut() = v.to_string();
        }
        if self.ids.chart_col.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.chart_col) {
                self.state.chart_column = v.to_string();
            }
        }
        if let Some(v) = self.ui.text_input_value(self.ids.pragma_value) {
            self.state.pragma_value_draft = v.to_string();
        }
        if let Some(i) = self.ui.tabs_selected(self.ids.tabs) {
            self.state.tab = MainTab::from_index(i);
        }
        if self.ids.dock_tabs.0 != 0 {
            if let Some(i) = self.ui.tabs_selected(self.ids.dock_tabs) {
                let docks = self.state.visible_docks();
                if let Some(d) = docks.get(i) {
                    self.state.dock_focus = *d;
                }
            }
        }
    }

    fn do_rebuild(&mut self) {
        self.snapshot_inputs();
        let built = build_ui(
            self.font.clone(),
            self.theme,
            self.win_w,
            self.win_h,
            &self.state,
        );
        self.ids = BuiltUiIds::from(&built);
        self.ui = built.ui;
        self.scene = built.scene;
        self.backend.set_clear_color(clear_color(&self.theme));
        self.needs_paint = true;
        self.rebuild = false;
        self.last_tree_sel = self.ui.tree_selected(self.ids.tree);
        self.last_tab = self.state.tab.index();
        self.last_browse_sel = self.ui.select_selected(self.ids.browse_select);
        self.last_browse_row = self.state.browse_sel_row;
    }

    fn set_error(&mut self, e: impl Into<String>) {
        let e = e.into();
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

    fn stub(&mut self, msg: &str) {
        self.state.status_msg = format!("TODO (parity): {msg}");
        self.rebuild = true;
    }

    fn action_new_db(&mut self) {
        if let Some(path) = file_dlg::pick_save_db() {
            match self.state.create_new_db(&path.display().to_string()) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
        } else {
            self.state.status_msg = "New Database cancelled".into();
        }
        self.rebuild = true;
    }

    fn action_open(&mut self, readonly: bool) {
        if let Some(path) = file_dlg::pick_open_db() {
            let p = path.display().to_string();
            let res = if readonly {
                self.state.open_readonly_path(&p)
            } else {
                self.state.open_path(&p)
            };
            match res {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
        } else {
            self.state.status_msg = "Open cancelled".into();
        }
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
        if self.state.guard_writable().is_err() {
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
            }
            self.rebuild = true;
            return;
        }
        self.snapshot_inputs();
        let name = self.state.new_table_name.clone();
        let cols = self.state.new_table_cols.clone();
        match dbmod::create_table_simple(self.state.conn.as_ref().unwrap(), &name, &cols) {
            Ok(()) => {
                self.state.log_sql(format!("CREATE TABLE {name} ({cols})"));
                self.state.mark_dirty();
                let _ = self.state.refresh_schema();
                self.state.browse_table = Some(name);
                let _ = self.state.refresh_browse();
                self.state.tab = MainTab::Structure;
                self.state.status_msg = "Table created (unwritten)".into();
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_create_index(&mut self) {
        if let Err(e) = self.state.guard_writable() {
            self.set_error(e);
            self.rebuild = true;
            return;
        }
        self.snapshot_inputs();
        let name = self.state.new_index_name.clone();
        let on = self.state.new_index_on.clone();
        // on form: table(cols) or table cols
        let (table, cols) = if let Some(paren) = on.find('(') {
            let table = on[..paren].trim().to_string();
            let cols = on[paren + 1..].trim().trim_end_matches(')').to_string();
            (table, cols)
        } else {
            let mut parts = on.splitn(2, char::is_whitespace);
            (
                parts.next().unwrap_or("").to_string(),
                parts.next().unwrap_or("").to_string(),
            )
        };
        if table.is_empty() || cols.is_empty() {
            self.set_error("Index needs table(cols), e.g. products(name)");
            self.rebuild = true;
            return;
        }
        match dbmod::create_index(self.state.conn.as_ref().unwrap(), &name, &table, &cols, false) {
            Ok(()) => {
                self.state
                    .log_sql(format!("CREATE INDEX {name} ON {table}({cols})"));
                self.state.mark_dirty();
                let _ = self.state.refresh_schema();
                self.state.status_msg = "Index created (unwritten)".into();
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_import_csv(&mut self) {
        if let Err(e) = self.state.guard_writable() {
            self.set_error(e);
            self.rebuild = true;
            return;
        }
        let Some(path) = file_dlg::pick_open_csv() else {
            self.state.status_msg = "Import CSV cancelled".into();
            self.rebuild = true;
            return;
        };
        let table = self
            .state
            .browse_table
            .clone()
            .unwrap_or_else(|| "imported".into());
        match dbmod::import_csv_parse(&path) {
            Ok((cols, rows)) => {
                match dbmod::import_csv_into_table(
                    self.state.conn.as_ref().unwrap(),
                    &table,
                    &cols,
                    &rows,
                    true,
                ) {
                    Ok(n) => {
                        self.state.log_sql(format!("-- import csv → {table} ({n} rows)"));
                        self.state.mark_dirty();
                        let _ = self.state.refresh_schema();
                        self.state.browse_table = Some(table);
                        let _ = self.state.refresh_browse();
                        self.state.io_path = path.display().to_string();
                        self.state.status_msg = format!("Imported {n} rows (unwritten)");
                        self.clear_error();
                    }
                    Err(e) => self.set_error(e),
                }
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
    }

    fn action_export_csv(&mut self) {
        if !self.require_conn() {
            return;
        }
        let Some(res) = self.state.browse_result.clone() else {
            self.set_error("Nothing to export — browse a table first");
            self.rebuild = true;
            return;
        };
        let path = file_dlg::pick_save_csv().unwrap_or_else(|| PathBuf::from("/tmp/e28_export.csv"));
        match dbmod::export_csv(&path, &res.columns, &res.rows) {
            Ok(()) => {
                self.state.io_path = path.display().to_string();
                self.state.status_msg = format!("Exported CSV {}", path.display());
                self.clear_error();
            }
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
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
                self.state.log_sql(format!("-- import sql {}", path.display()));
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

    fn action_export_sql(&mut self) {
        if !self.require_conn() {
            return;
        }
        if self.state.dirty {
            let _ = self.state.write_changes();
        }
        let path = file_dlg::pick_save_sql().unwrap_or_else(|| PathBuf::from("/tmp/e28_dump.sql"));
        match dbmod::export_sql_dump(self.state.conn.as_ref().unwrap(), &path) {
            Ok(()) => {
                self.state.io_path = path.display().to_string();
                self.state.log_sql(format!("-- dump {}", path.display()));
                self.state.status_msg = format!("Dumped SQL {}", path.display());
                self.clear_error();
            }
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

    fn handle_menu(&mut self, item: u64) {
        match item {
            mid::FILE_NEW => self.action_new_db(),
            mid::FILE_NEW_MEM => {
                match self.state.new_memory_db() {
                    Ok(()) => self.clear_error(),
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            mid::FILE_OPEN => self.action_open(false),
            mid::FILE_OPEN_RO => self.action_open(true),
            mid::FILE_CLOSE => {
                self.state.confirm_kind = ConfirmKind::CloseDb;
                self.state.confirm_open = true;
                self.rebuild = true;
            }
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
            mid::FILE_EXIT => {
                if self.state.dirty {
                    self.state.confirm_kind = ConfirmKind::ExitDirty;
                    self.state.confirm_open = true;
                    self.rebuild = true;
                } else {
                    self.exit_requested = true;
                }
            }
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
                } else {
                    self.state.confirm_kind = ConfirmKind::DropObject;
                    self.state.confirm_open = true;
                    self.rebuild = true;
                }
            }
            mid::VIEW_DOCK_EDIT => {
                self.state.dock_edit = !self.state.dock_edit;
                if self.state.dock_edit {
                    self.state.dock_focus = DockKind::EditCell;
                }
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
            mid::HELP_ABOUT => {
                self.state.status_msg =
                    "next-gui DB Browser — DB4S parity goal · see PARITY.md".into();
                self.rebuild = true;
            }
            mid::HELP_WIKI => self.stub("Help Wiki link"),
            mid::FILE_ATTACH
            | mid::FILE_IMPORT_CLIP
            | mid::FILE_EXPORT_JSON
            | mid::FILE_PROJECT_OPEN
            | mid::FILE_PROJECT_SAVE
            | mid::EDIT_MODIFY
            | mid::EDIT_PREFS
            | mid::TOOLS_ENCRYPT
            | mid::TOOLS_EXT => self.stub(&format!("menu item {item}")),
            _ => {}
        }
    }

    fn handle_button(&mut self, id: WidgetId) {
        if id == self.ids.btn_theme {
            self.handle_menu(mid::VIEW_THEME);
            return;
        }
        if id == self.ids.btn_new {
            self.action_new_db();
            return;
        }
        if id == self.ids.btn_open {
            self.action_open(false);
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
            self.action_create_table();
            return;
        }
        if id == self.ids.btn_struct_new_index {
            self.action_create_index();
            return;
        }
        if id == self.ids.btn_struct_modify {
            self.stub("Modify Object wizard");
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
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            self.snapshot_inputs();
            let sql = self.state.ddl_draft.clone();
            match dbmod::execute_sql(self.state.conn.as_ref().unwrap(), &sql) {
                Ok(_) => {
                    self.state.log_sql(sql);
                    self.state.mark_dirty();
                    let _ = self.state.refresh_schema();
                    self.state.status_msg = "DDL ok (unwritten)".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_go {
            self.snapshot_inputs();
            self.apply_browse_select();
            match self.state.refresh_browse() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_add_row {
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            let Some(table) = self.state.browse_table.clone() else {
                self.set_error("No browse table");
                self.rebuild = true;
                return;
            };
            match dbmod::insert_row(self.state.conn.as_ref().unwrap(), &table, &[], &[]) {
                Ok(_) => {
                    self.state.log_sql(format!("INSERT INTO {table} DEFAULT VALUES"));
                    self.state.mark_dirty();
                    let _ = self.state.refresh_browse();
                    self.state.status_msg = "Row inserted (unwritten)".into();
                    self.clear_error();
                }
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
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            self.snapshot_inputs();
            let Some(table) = self.state.browse_table.clone() else {
                self.set_error("No browse table");
                self.rebuild = true;
                return;
            };
            let Some(row) = self
                .state
                .browse_sel_row
                .or_else(|| self.ui.table_selected_row(self.ids.browse_table))
            else {
                self.set_error("Select a browse row first");
                self.rebuild = true;
                return;
            };
            let Some(&rowid) = self.state.browse_rowids.get(row) else {
                self.set_error("No rowid for selection (views are read-only here)");
                self.rebuild = true;
                return;
            };
            let col = self.state.edit_col.trim().to_string();
            if col.is_empty() {
                self.set_error("Enter column name in Edit Cell dock");
                self.rebuild = true;
                return;
            }
            let val = self.state.edit_value.clone();
            match dbmod::update_cell(self.state.conn.as_ref().unwrap(), &table, rowid, &col, &val) {
                Ok(_) => {
                    self.state
                        .log_sql(format!("UPDATE {table} SET {col}=… WHERE rowid={rowid}"));
                    self.state.mark_dirty();
                    let _ = self.state.refresh_browse();
                    self.state.status_msg = "Cell updated (unwritten)".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_pragma_apply {
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            self.snapshot_inputs();
            let Some(idx) = self.ui.property_grid_selected(self.ids.pragma_grid) else {
                self.set_error("Select a pragma row");
                self.rebuild = true;
                return;
            };
            let key = self
                .state
                .pragmas
                .get(idx)
                .map(|(k, _)| k.clone())
                .unwrap_or_default();
            let mut value = self.state.pragma_value_draft.trim().to_string();
            if value.is_empty() {
                if let Some(pv) = self.ui.property_grid_value(self.ids.pragma_grid, idx) {
                    value = match pv {
                        PropertyValue::Bool(b) => {
                            if *b {
                                "ON".into()
                            } else {
                                "OFF".into()
                            }
                        }
                        PropertyValue::Select { options, selected } => {
                            options.get(*selected).cloned().unwrap_or_default()
                        }
                        PropertyValue::Text(s) => s.clone(),
                        PropertyValue::Number(n) => n.to_string(),
                    };
                }
            }
            match dbmod::write_pragma(self.state.conn.as_ref().unwrap(), &key, &value) {
                Ok(()) => {
                    self.state.log_sql(format!("PRAGMA {key} = {value}"));
                    self.state.mark_dirty();
                    let _ = self.state.refresh_pragmas();
                    self.state.status_msg = format!("PRAGMA {key} applied (session)");
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_run {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let sql = self.state.sql_editor().to_string();
            match dbmod::execute_sql(self.state.conn.as_ref().unwrap(), &sql) {
                Ok(res) => {
                    let mutating = sql
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("insert")
                        || sql.trim_start().to_ascii_lowercase().starts_with("update")
                        || sql.trim_start().to_ascii_lowercase().starts_with("delete")
                        || sql.trim_start().to_ascii_lowercase().starts_with("create")
                        || sql.trim_start().to_ascii_lowercase().starts_with("drop")
                        || sql.trim_start().to_ascii_lowercase().starts_with("alter");
                    if mutating {
                        self.state.mark_dirty();
                    }
                    self.state.log_sql(sql.lines().next().unwrap_or("SQL").to_string());
                    let n = res.rows.len();
                    self.state.sql_result = Some(res);
                    self.state.sql_message = format!("OK — {n} result row(s)");
                    let _ = self.state.refresh_schema();
                    self.state.status_msg = "SQL ok".into();
                    self.clear_error();
                }
                Err(e) => {
                    self.state.sql_result = None;
                    self.set_error(e);
                }
            }
            self.state.tab = MainTab::ExecuteSql;
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_new_tab {
            self.snapshot_inputs();
            self.state.sql_tabs.push("-- new tab\n".into());
            self.state.sql_tab_idx = self.state.sql_tabs.len() - 1;
            self.state.status_msg = format!("SQL tab {}", self.state.sql_tab_idx + 1);
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_open {
            if let Some(path) = file_dlg::pick_open_sql() {
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        *self.state.sql_editor_mut() = text;
                        self.state.tab = MainTab::ExecuteSql;
                        self.state.status_msg = format!("Opened SQL {}", path.display());
                        self.clear_error();
                    }
                    Err(e) => self.set_error(e.to_string()),
                }
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_save {
            self.snapshot_inputs();
            let path =
                file_dlg::pick_save_sql().unwrap_or_else(|| PathBuf::from("/tmp/e28_query.sql"));
            match std::fs::write(&path, self.state.sql_editor()) {
                Ok(()) => {
                    self.state.status_msg = format!("Saved SQL {}", path.display());
                    self.clear_error();
                }
                Err(e) => self.set_error(e.to_string()),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_chart {
            self.snapshot_inputs();
            self.apply_chart_select();
            match self.state.refresh_chart() {
                Ok(()) => {
                    self.state.dock_plot = true;
                    self.state.dock_focus = DockKind::Plot;
                    self.state.status_msg = format!(
                        "Plot {} pts from {}.{}",
                        self.state.chart_points.len(),
                        self.state.chart_table.as_deref().unwrap_or("?"),
                        self.state.chart_column
                    );
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
        }
    }

    fn apply_browse_select(&mut self) {
        let opts = self.state.table_names();
        if let Some(i) = self.ui.select_selected(self.ids.browse_select) {
            if let Some(name) = opts.get(i) {
                self.state.browse_table = Some(name.clone());
            }
        }
    }

    fn apply_chart_select(&mut self) {
        if self.ids.chart_select.0 == 0 {
            return;
        }
        let opts = self.state.table_names();
        if let Some(i) = self.ui.select_selected(self.ids.chart_select) {
            if let Some(name) = opts.get(i) {
                self.state.chart_table = Some(name.clone());
            }
        }
    }

    fn poll_selection_changes(&mut self) {
        let mut dirty = false;
        if let Some(sel) = self.ui.tree_selected(self.ids.tree) {
            if self.last_tree_sel != Some(sel) {
                self.last_tree_sel = Some(sel);
                self.state.select_schema_node(sel);
                dirty = true;
            }
        }
        if let Some(tab) = self.ui.tabs_selected(self.ids.tabs) {
            if tab != self.last_tab {
                self.last_tab = tab;
                self.state.tab = MainTab::from_index(tab);
            }
        }
        if let Some(i) = self.ui.select_selected(self.ids.browse_select) {
            if self.last_browse_sel != Some(i) {
                self.last_browse_sel = Some(i);
                self.apply_browse_select();
                let _ = self.state.refresh_browse();
                dirty = true;
            }
        }
        if let Some(row) = self.ui.table_selected_row(self.ids.browse_table) {
            if self.last_browse_row != Some(row) {
                self.last_browse_row = Some(row);
                self.state.browse_sel_row = Some(row);
                self.state.sync_edit_cell_from_selection();
                dirty = true;
            }
        }
        if dirty {
            self.rebuild = true;
        }
    }

    fn poll_header_sort(&mut self) {
        if let Some((tid, col_idx)) = self.ui.take_table_header_click() {
            if tid != self.ids.browse_table {
                return;
            }
            let Some(res) = self.state.browse_result.as_ref() else {
                return;
            };
            let Some(col_name) = res.columns.get(col_idx).cloned() else {
                return;
            };
            // Strip sort markers if present in display — columns store raw names in state
            let col_name = col_name
                .trim_end_matches(" ▲")
                .trim_end_matches(" ▼")
                .to_string();
            if self.state.browse_order_col.as_deref() == Some(col_name.as_str()) {
                self.state.browse_order_desc = !self.state.browse_order_desc;
            } else {
                self.state.browse_order_col = Some(col_name);
                self.state.browse_order_desc = false;
            }
            match self.state.refresh_browse() {
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
            if !matches!(action, ModalAction::Primary) {
                self.rebuild = true;
                return;
            }
            match kind {
                ConfirmKind::CloseDb => {
                    self.state.close_db();
                }
                ConfirmKind::Revert => match self.state.revert_changes() {
                    Ok(()) => self.clear_error(),
                    Err(e) => self.set_error(e),
                },
                ConfirmKind::ExitDirty => {
                    let _ = self.state.revert_changes();
                    self.exit_requested = true;
                }
                ConfirmKind::DropObject => {
                    if let Some(sel) = self.state.selected.clone() {
                        if let Err(e) = self.state.guard_writable() {
                            self.set_error(e);
                        } else if let Some(conn) = self.state.conn.as_ref() {
                            match dbmod::drop_object(conn, sel.kind, &sel.name) {
                                Ok(()) => {
                                    self.state.log_sql(format!("DROP {:?} {}", sel.kind, sel.name));
                                    self.state.mark_dirty();
                                    self.state.selected = None;
                                    let _ = self.state.refresh_schema();
                                    let _ = self.state.refresh_browse();
                                    self.state.status_msg = format!("Dropped {} (unwritten)", sel.name);
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
                    } else if let (Some(table), Some(row)) = (
                        self.state.browse_table.clone(),
                        self.state
                            .browse_sel_row
                            .or_else(|| self.ui.table_selected_row(self.ids.browse_table)),
                    ) {
                        if let Some(&rowid) = self.state.browse_rowids.get(row) {
                            if let Some(conn) = self.state.conn.as_ref() {
                                match dbmod::delete_row_by_rowid(conn, &table, rowid) {
                                    Ok(_) => {
                                        self.state.log_sql(format!(
                                            "DELETE FROM {table} WHERE rowid={rowid}"
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
                ConfirmKind::None => {}
            }
            self.rebuild = true;
        }
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

    fn after_ui(&mut self, ctx: &mut WindowCtx<'_>) {
        while let Some(id) = self.ui.take_button_activate() {
            self.handle_button(id);
        }
        while let Some((_owner, item)) = self.ui.take_menu_action() {
            self.handle_menu(item);
        }
        self.handle_modal();
        self.poll_header_sort();
        self.poll_selection_changes();
        if self.rebuild {
            self.do_rebuild();
        } else {
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
        }
        if self.exit_requested {
            ctx.request_exit();
        }
        ctx.request_redraw();
    }
}

impl AppCallbacks for Demo {
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
        } else if matches!(self.window_backend, PresentBackend::Softbuffer) {
            match GpuRenderer::try_new(w, h, clear_color(&self.theme)) {
                Ok(gpu) => {
                    self.backend = Backend::Gpu(gpu);
                    self.using_gpu = true;
                }
                Err(e) => eprintln!("e28_db_browser: GPU unavailable ({e}); Soft only"),
            }
        }
        self.scene.mark_full_dirty(w as f32, h as f32);
        ctx.set_title(&format!(
            "next-gui DB Browser — {}",
            self.backend.name()
        ));
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
            Event::Resized { width, height, .. } => {
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
                button: PointerButton::Left,
                pressed: true,
                x,
                y,
                ..
            } => {
                let xf = x as f32;
                let yf = y as f32;
                if self.ui.pointer_pressed(&mut self.scene, xf, yf) {
                    self.after_ui(ctx);
                    return;
                }
                if let Some(hit) = self.ui.hit_widget(&self.scene, xf, yf) {
                    if self.ui.is_button(hit) {
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
                if self.ui.is_dragging() {
                    self.ui.pointer_released();
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                    ctx.request_redraw();
                }
            }
            Event::PointerMoved { x, y, .. } => {
                if self.ui.pointer_moved(x as f32, y as f32) {
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
                if let Some(ui_key) = map_key(key, modifiers) {
                    if self.ui.key_input(ui_key) {
                        self.after_ui(ctx);
                        return;
                    }
                    if matches!(ui_key, UiKey::Escape) {
                        if self.ui.modal_open(self.ids.modal).unwrap_or(false) {
                            let _ = self.ui.set_modal_open(self.ids.modal, false);
                            self.state.confirm_open = false;
                            self.rebuild = true;
                            self.do_rebuild();
                            ctx.request_redraw();
                            return;
                        }
                        ctx.request_exit();
                        return;
                    }
                    if matches!(ui_key, UiKey::Space) && self.ui.focused().is_none() {
                        let (w, h) = ctx.size();
                        self.toggle_backend(w, h);
                        ctx.set_title(&format!(
                            "next-gui DB Browser — {}",
                            self.backend.name()
                        ));
                        ctx.request_redraw();
                    }
                }
            }
            Event::RedrawRequested { .. } => {
                self.needs_paint = true;
            }
            _ => {}
        }
    }

    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
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
    let use_native = std::env::var_os("NEXT_GUI_GPU_SURFACE").is_some()
        && GpuRenderer::adapter_available();
    let attrs = if use_native {
        WindowAttrs::new("next-gui DB Browser", 1400, 860).with_gpu_surface()
    } else {
        WindowAttrs::new("next-gui DB Browser", 1400, 860)
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
