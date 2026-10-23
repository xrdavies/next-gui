//! DB Browser for SQLite–style example (`e28_db_browser`).
//!
//! Soft↔GPU: Space when nothing focused. Theme: toolbar button.
//! `NEXT_GUI_DB_BROWSER_QA=1` dumps `/tmp/next-gui-db-browser.ppm` after first paint then exits.
//!
//! ```bash
//! cargo run -p e28_db_browser
//! ```

mod db;
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
use next_gui_ui::{
    ModalAction, PropertyValue, Theme, ThemeMode, Ui, UiKey, WidgetId,
};

use crate::db::{self as dbmod};
use crate::state::{AppState, ConfirmKind, MainTab};
use crate::ui_build::{build_ui, clear_color, BuiltUi};

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

    fn detail(&self) -> String {
        match self {
            Backend::Soft(_) => "CPU soft".into(),
            Backend::Gpu(g) => {
                let path = match g.present_path() {
                    GpuPresentPath::ReadbackSoftbuffer => "readback→softbuffer",
                    GpuPresentPath::NativeSurface => "native surface",
                };
                format!("wgpu {} ({})", g.backend_name(), path)
            }
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
        std::fs::create_dir_all(parent)?;
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
}

/// Widget ids kept after rebuild (mirror of BuiltUi without ui/scene).
#[allow(dead_code)]
struct BuiltUiIds {
    btn_new: WidgetId,
    btn_demo: WidgetId,
    btn_open: WidgetId,
    btn_close: WidgetId,
    btn_vacuum: WidgetId,
    btn_refresh: WidgetId,
    btn_theme: WidgetId,
    path_input: WidgetId,
    tabs: WidgetId,
    tree: WidgetId,
    ddl_area: WidgetId,
    btn_run_ddl: WidgetId,
    btn_drop: WidgetId,
    new_table_name: WidgetId,
    new_table_cols: WidgetId,
    btn_create_table: WidgetId,
    browse_select: WidgetId,
    browse_filter: WidgetId,
    btn_browse_go: WidgetId,
    btn_add_row: WidgetId,
    btn_del_row: WidgetId,
    btn_edit_cell: WidgetId,
    edit_col: WidgetId,
    edit_value: WidgetId,
    browse_table: WidgetId,
    pragma_grid: WidgetId,
    pragma_value: WidgetId,
    btn_pragma_apply: WidgetId,
    sql_area: WidgetId,
    btn_run_sql: WidgetId,
    sql_table: WidgetId,
    chart_select: WidgetId,
    chart_col: WidgetId,
    btn_chart: WidgetId,
    io_path: WidgetId,
    btn_import_csv: WidgetId,
    btn_export_csv: WidgetId,
    btn_export_sql: WidgetId,
    btn_import_sql: WidgetId,
    status: WidgetId,
    modal: WidgetId,
}

impl From<&BuiltUi> for BuiltUiIds {
    fn from(b: &BuiltUi) -> Self {
        Self {
            btn_new: b.btn_new,
            btn_demo: b.btn_demo,
            btn_open: b.btn_open,
            btn_close: b.btn_close,
            btn_vacuum: b.btn_vacuum,
            btn_refresh: b.btn_refresh,
            btn_theme: b.btn_theme,
            path_input: b.path_input,
            tabs: b.tabs,
            tree: b.tree,
            ddl_area: b.ddl_area,
            btn_run_ddl: b.btn_run_ddl,
            btn_drop: b.btn_drop,
            new_table_name: b.new_table_name,
            new_table_cols: b.new_table_cols,
            btn_create_table: b.btn_create_table,
            browse_select: b.browse_select,
            browse_filter: b.browse_filter,
            btn_browse_go: b.btn_browse_go,
            btn_add_row: b.btn_add_row,
            btn_del_row: b.btn_del_row,
            btn_edit_cell: b.btn_edit_cell,
            edit_col: b.edit_col,
            edit_value: b.edit_value,
            browse_table: b.browse_table,
            pragma_grid: b.pragma_grid,
            pragma_value: b.pragma_value,
            btn_pragma_apply: b.btn_pragma_apply,
            sql_area: b.sql_area,
            btn_run_sql: b.btn_run_sql,
            sql_table: b.sql_table,
            chart_select: b.chart_select,
            chart_col: b.chart_col,
            btn_chart: b.btn_chart,
            io_path: b.io_path,
            btn_import_csv: b.btn_import_csv,
            btn_export_csv: b.btn_export_csv,
            btn_export_sql: b.btn_export_sql,
            btn_import_sql: b.btn_import_sql,
            status: b.status,
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
        let state = AppState::new_with_demo();
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
        }
    }

    fn snapshot_inputs(&mut self) {
        if let Some(v) = self.ui.text_input_value(self.ids.path_input) {
            self.state.path_draft = v.to_string();
        }
        if let Some(v) = self.ui.text_area_value(self.ids.ddl_area) {
            self.state.ddl_draft = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.new_table_name) {
            self.state.new_table_name = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.new_table_cols) {
            self.state.new_table_cols = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.browse_filter) {
            self.state.browse_filter = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.edit_col) {
            self.state.edit_col = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.edit_value) {
            self.state.edit_value = v.to_string();
        }
        if let Some(v) = self.ui.text_area_value(self.ids.sql_area) {
            self.state.sql_editor = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.chart_col) {
            self.state.chart_column = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.io_path) {
            self.state.io_path = v.to_string();
        }
        if let Some(v) = self.ui.text_input_value(self.ids.pragma_value) {
            self.state.pragma_value_draft = v.to_string();
        }
        if let Some(i) = self.ui.tabs_selected(self.ids.tabs) {
            self.state.tab = MainTab::from_index(i);
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
    }

    fn set_error(&mut self, e: impl Into<String>) {
        let e = e.into();
        self.state.error_msg = Some(e.clone());
        self.state.status_msg = e;
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

    fn handle_button(&mut self, id: WidgetId) {
        if id == self.ids.btn_theme {
            self.theme = match self.theme.mode {
                ThemeMode::Dark => Theme::light(),
                ThemeMode::Light => Theme::dark(),
            };
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_new {
            self.snapshot_inputs();
            let path = if self.state.path_draft.trim().is_empty() {
                dbmod::default_demo_path()
                    .with_extension("new.db")
                    .display()
                    .to_string()
            } else {
                self.state.path_draft.trim().to_string()
            };
            match self.state.create_new_db(&path) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_demo {
            match self.state.new_demo_db() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_open {
            self.snapshot_inputs();
            let path = self.state.path_draft.trim().to_string();
            match self.state.open_path(&path) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_close {
            self.state.confirm_kind = ConfirmKind::CloseDb;
            self.state.confirm_open = true;
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_vacuum {
            if !self.require_conn() {
                return;
            }
            match dbmod::vacuum(self.state.conn.as_ref().unwrap()) {
                Ok(()) => {
                    self.state.log_sql("VACUUM");
                    self.state.status_msg = "VACUUM done".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_refresh {
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
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let sql = self.state.ddl_draft.clone();
            match dbmod::execute_sql(self.state.conn.as_ref().unwrap(), &sql) {
                Ok(_) => {
                    self.state.log_sql(sql);
                    let _ = self.state.refresh_schema();
                    self.state.status_msg = "DDL ok".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_drop {
            if self.state.selected.is_none() {
                self.set_error("Select an object to drop");
                self.rebuild = true;
                return;
            }
            self.state.confirm_kind = ConfirmKind::DropObject;
            self.state.confirm_open = true;
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_create_table {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let name = self.state.new_table_name.clone();
            let cols = self.state.new_table_cols.clone();
            match dbmod::create_table_simple(self.state.conn.as_ref().unwrap(), &name, &cols) {
                Ok(()) => {
                    self.state.log_sql(format!("CREATE TABLE {name} ({cols})"));
                    let _ = self.state.refresh_schema();
                    self.state.browse_table = Some(name);
                    let _ = self.state.refresh_browse();
                    self.state.status_msg = "Table created".into();
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
            let sort = self.state.edit_col.trim();
            if !sort.is_empty() {
                if self.state.browse_order_col.as_deref() == Some(sort) {
                    self.state.browse_order_desc = !self.state.browse_order_desc;
                } else {
                    self.state.browse_order_col = Some(sort.to_string());
                    self.state.browse_order_desc = false;
                }
            } else {
                self.state.browse_order_col = None;
                self.state.browse_order_desc = false;
            }
            match self.state.refresh_browse() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_add_row {
            if !self.require_conn() {
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
                    let _ = self.state.refresh_browse();
                    self.state.status_msg = "Row inserted".into();
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
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let Some(table) = self.state.browse_table.clone() else {
                self.set_error("No browse table");
                self.rebuild = true;
                return;
            };
            let Some(row) = self.ui.table_selected_row(self.ids.browse_table) else {
                self.set_error("Select a row first");
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
                self.set_error("Enter column name");
                self.rebuild = true;
                return;
            }
            let val = self.state.edit_value.clone();
            match dbmod::update_cell(self.state.conn.as_ref().unwrap(), &table, rowid, &col, &val) {
                Ok(_) => {
                    self.state
                        .log_sql(format!("UPDATE {table} SET {col}=… WHERE rowid={rowid}"));
                    let _ = self.state.refresh_browse();
                    self.state.status_msg = "Cell updated".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_pragma_apply {
            if !self.require_conn() {
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
            // Prefer typed draft; else take grid display value.
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
                    let _ = self.state.refresh_pragmas();
                    self.state.status_msg = format!("PRAGMA {key} applied");
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_run_sql {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let sql = self.state.sql_editor.clone();
            match dbmod::execute_sql(self.state.conn.as_ref().unwrap(), &sql) {
                Ok(res) => {
                    self.state.log_sql(sql.lines().next().unwrap_or("SQL").to_string());
                    self.state.sql_result = Some(res);
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
        if id == self.ids.btn_chart {
            self.snapshot_inputs();
            self.apply_chart_select();
            match self.state.refresh_chart() {
                Ok(()) => {
                    self.state.status_msg = format!(
                        "Chart {} pts from {}.{}",
                        self.state.chart_points.len(),
                        self.state.chart_table.as_deref().unwrap_or("?"),
                        self.state.chart_column
                    );
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_import_csv {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let path = PathBuf::from(self.state.io_path.trim());
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
                            let _ = self.state.refresh_schema();
                            self.state.browse_table = Some(table);
                            let _ = self.state.refresh_browse();
                            self.state.status_msg = format!("Imported {n} rows");
                            self.clear_error();
                        }
                        Err(e) => self.set_error(e),
                    }
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_export_csv {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let path = PathBuf::from(if self.state.io_path.trim().is_empty() {
                "/tmp/e28_export.csv"
            } else {
                self.state.io_path.trim()
            });
            let Some(res) = self.state.browse_result.clone() else {
                self.set_error("Nothing to export — browse a table first");
                self.rebuild = true;
                return;
            };
            match dbmod::export_csv(&path, &res.columns, &res.rows) {
                Ok(()) => {
                    self.state.io_path = path.display().to_string();
                    self.state.status_msg = format!("Exported CSV {}", path.display());
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_export_sql {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let path = PathBuf::from(if self.state.io_path.trim().is_empty() {
                "/tmp/e28_dump.sql"
            } else {
                self.state.io_path.trim()
            });
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
            return;
        }
        if id == self.ids.btn_import_sql {
            if !self.require_conn() {
                return;
            }
            self.snapshot_inputs();
            let path = PathBuf::from(self.state.io_path.trim());
            match dbmod::import_sql_file(self.state.conn.as_ref().unwrap(), &path) {
                Ok(()) => {
                    self.state.log_sql(format!("-- import sql {}", path.display()));
                    let _ = self.state.refresh_schema();
                    let _ = self.state.refresh_browse();
                    self.state.status_msg = "SQL import ok".into();
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
        // Header click sort: if user re-selects same table after clicking header — table header
        // is a no-op in next-gui; provide sort via double-Apply with edit_col as order column.
        if dirty {
            self.rebuild = true;
        }
    }

    fn handle_modal(&mut self) {
        if let Some((mid, action)) = self.ui.take_modal_action() {
            if mid != self.ids.modal {
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
                ConfirmKind::DropObject => {
                    if let Some(sel) = self.state.selected.clone() {
                        if let Some(conn) = self.state.conn.as_ref() {
                            match dbmod::drop_object(conn, sel.kind, &sel.name) {
                                Ok(()) => {
                                    self.state.log_sql(format!("DROP {:?} {}", sel.kind, sel.name));
                                    self.state.selected = None;
                                    let _ = self.state.refresh_schema();
                                    let _ = self.state.refresh_browse();
                                    self.state.status_msg = format!("Dropped {}", sel.name);
                                    self.clear_error();
                                }
                                Err(e) => self.set_error(e),
                            }
                        }
                    }
                }
                ConfirmKind::DeleteRow => {
                    if let (Some(table), Some(row)) = (
                        self.state.browse_table.clone(),
                        self.ui.table_selected_row(self.ids.browse_table),
                    ) {
                        if let Some(&rowid) = self.state.browse_rowids.get(row) {
                            if let Some(conn) = self.state.conn.as_ref() {
                                match dbmod::delete_row_by_rowid(conn, &table, rowid) {
                                    Ok(_) => {
                                        self.state.log_sql(format!(
                                            "DELETE FROM {table} WHERE rowid={rowid}"
                                        ));
                                        let _ = self.state.refresh_browse();
                                        self.state.status_msg = "Row deleted".into();
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
        self.handle_modal();
        self.poll_selection_changes();
        if self.rebuild {
            self.do_rebuild();
        } else {
            self.ui.sync(&mut self.scene);
            self.needs_paint = true;
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
                if self.ui.pointer_pressed(&mut self.scene, x as f32, y as f32) {
                    self.after_ui(ctx);
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
            let path = "/tmp/next-gui-db-browser.ppm";
            if let Some((pixels, w, h)) = self.backend.frame_pixels() {
                match write_ppm(path, pixels, w, h) {
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
        WindowAttrs::new("next-gui DB Browser", 1180, 760).with_gpu_surface()
    } else {
        WindowAttrs::new("next-gui DB Browser", 1180, 760)
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
