//! DB Browser for SQLite–style example (`e28_db_browser`) — DB4S chrome parity.
//!
//! Soft↔GPU: Space when nothing focused. Theme: toolbar / View menu.
//! `NEXT_GUI_DB_BROWSER_QA=1` dumps a PPM after first paint then exits.

mod db;
mod file_dlg;
mod prefs;
mod project;
mod state;
mod ui_build;

use std::path::PathBuf;

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
use crate::state::{AppState, ConfirmKind, DockKind, MainTab, OverlayKind};
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
    last_wizard_list: Option<usize>,
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
    btn_browse_new_tab: WidgetId,
    browse_select: WidgetId,
    browse_filter: WidgetId,
    btn_browse_go: WidgetId,
    btn_page_prev: WidgetId,
    btn_page_next: WidgetId,
    btn_add_row: WidgetId,
    btn_del_row: WidgetId,
    browse_table: WidgetId,
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
    btn_sql_save: WidgetId,
    btn_sql_find: WidgetId,
    btn_sql_stop: WidgetId,
    btn_sql_csv: WidgetId,
    btn_sql_json: WidgetId,
    sql_table: WidgetId,
    dock_tabs: WidgetId,
    edit_col: WidgetId,
    edit_value: WidgetId,
    btn_edit_cell: WidgetId,
    chart_select: WidgetId,
    chart_col: WidgetId,
    btn_chart: WidgetId,
    modal: WidgetId,
    ov_name: WidgetId,
    insert_values: Vec<WidgetId>,
    insert_modes: Vec<WidgetId>,
    ov_list: WidgetId,
    ov_col_name: WidgetId,
    ov_col_type: WidgetId,
    ov_pk: WidgetId,
    ov_nn: WidgetId,
    ov_unique: WidgetId,
    ov_default: WidgetId,
    ov_btn_add: WidgetId,
    ov_btn_del: WidgetId,
    ov_btn_up: WidgetId,
    ov_btn_down: WidgetId,
    ov_btn_ok: WidgetId,
    ov_btn_cancel: WidgetId,
    ov_idx_table: WidgetId,
    ov_idx_cols: WidgetId,
    ov_idx_unique: WidgetId,
    ov_csv_table: WidgetId,
    ov_csv_header: WidgetId,
    ov_csv_delim: WidgetId,
    ov_csv_quote: WidgetId,
    ov_csv_paste: WidgetId,
    ov_attach_path: WidgetId,
    ov_attach_schema: WidgetId,
    ov_find: WidgetId,
    ov_replace: WidgetId,
    ov_btn_find: WidgetId,
    ov_btn_repl: WidgetId,
    ov_btn_repl_all: WidgetId,
    ov_cipher_pass: WidgetId,
    ov_cipher_page: WidgetId,
    ov_cipher_kdf: WidgetId,
    ov_prefs_font: WidgetId,
    ov_prefs_theme: WidgetId,
    ov_prefs_confirm: WidgetId,
    ov_prefs_delim: WidgetId,
    ov_frozen: WidgetId,
    ov_export_struct: WidgetId,
    ov_export_data: WidgetId,
    ov_export_drop: WidgetId,
    ov_export_fk: WidgetId,
    ov_cond_col: WidgetId,
    ov_cond_op: WidgetId,
    ov_cond_val: WidgetId,
    ov_cond_color: WidgetId,
    btn_sql_line: WidgetId,
    btn_sql_comment: WidgetId,
    btn_sql_view: WidgetId,
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
    edit_image: WidgetId,
    remote_list: WidgetId,
    remote_draft: WidgetId,
    btn_remote_add: WidgetId,
    btn_remote_open: WidgetId,
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
            btn_browse_new_tab: b.btn_browse_new_tab,
            browse_select: b.browse_select,
            browse_filter: b.browse_filter,
            btn_browse_go: b.btn_browse_go,
            btn_page_prev: b.btn_page_prev,
            btn_page_next: b.btn_page_next,
            btn_add_row: b.btn_add_row,
            btn_del_row: b.btn_del_row,
            browse_table: b.browse_table,
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
            btn_sql_save: b.btn_sql_save,
            btn_sql_find: b.btn_sql_find,
            btn_sql_stop: b.btn_sql_stop,
            btn_sql_csv: b.btn_sql_csv,
            btn_sql_json: b.btn_sql_json,
            sql_table: b.sql_table,
            dock_tabs: b.dock_tabs,
            edit_col: b.edit_col,
            edit_value: b.edit_value,
            btn_edit_cell: b.btn_edit_cell,
            chart_select: b.chart_select,
            chart_col: b.chart_col,
            btn_chart: b.btn_chart,
            modal: b.modal,
            ov_name: b.ov_name,
            insert_values: b.insert_values.clone(),
            insert_modes: b.insert_modes.clone(),
            ov_list: b.ov_list,
            ov_col_name: b.ov_col_name,
            ov_col_type: b.ov_col_type,
            ov_pk: b.ov_pk,
            ov_nn: b.ov_nn,
            ov_unique: b.ov_unique,
            ov_default: b.ov_default,
            ov_btn_add: b.ov_btn_add,
            ov_btn_del: b.ov_btn_del,
            ov_btn_up: b.ov_btn_up,
            ov_btn_down: b.ov_btn_down,
            ov_btn_ok: b.ov_btn_ok,
            ov_btn_cancel: b.ov_btn_cancel,
            ov_idx_table: b.ov_idx_table,
            ov_idx_cols: b.ov_idx_cols,
            ov_idx_unique: b.ov_idx_unique,
            ov_csv_table: b.ov_csv_table,
            ov_csv_header: b.ov_csv_header,
            ov_csv_delim: b.ov_csv_delim,
            ov_csv_quote: b.ov_csv_quote,
            ov_csv_paste: b.ov_csv_paste,
            ov_attach_path: b.ov_attach_path,
            ov_attach_schema: b.ov_attach_schema,
            ov_find: b.ov_find,
            ov_replace: b.ov_replace,
            ov_btn_find: b.ov_btn_find,
            ov_btn_repl: b.ov_btn_repl,
            ov_btn_repl_all: b.ov_btn_repl_all,
            ov_cipher_pass: b.ov_cipher_pass,
            ov_cipher_page: b.ov_cipher_page,
            ov_cipher_kdf: b.ov_cipher_kdf,
            ov_prefs_font: b.ov_prefs_font,
            ov_prefs_theme: b.ov_prefs_theme,
            ov_prefs_confirm: b.ov_prefs_confirm,
            ov_prefs_delim: b.ov_prefs_delim,
            ov_frozen: b.ov_frozen,
            ov_export_struct: b.ov_export_struct,
            ov_export_data: b.ov_export_data,
            ov_export_drop: b.ov_export_drop,
            ov_export_fk: b.ov_export_fk,
            ov_cond_col: b.ov_cond_col,
            ov_cond_op: b.ov_cond_op,
            ov_cond_val: b.ov_cond_val,
            ov_cond_color: b.ov_cond_color,
            btn_sql_line: b.btn_sql_line,
            btn_sql_comment: b.btn_sql_comment,
            btn_sql_view: b.btn_sql_view,
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
            edit_image: b.edit_image,
            remote_list: b.remote_list,
            remote_draft: b.remote_draft,
            btn_remote_add: b.btn_remote_add,
            btn_remote_open: b.btn_remote_open,
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
                "wizard" | "table" => {
                    state.open_create_table_wizard();
                    MainTab::Structure
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
                "exportsql" | "export" => {
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
            last_wizard_list: None,
            exit_requested: false,
        }
    }

    fn snapshot_inputs(&mut self) {
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
        if self.ids.sql_area.0 != 0 {
            if let Some(v) = self.ui.text_area_value(self.ids.sql_area) {
                *self.state.sql_editor_mut() = v.to_string();
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
                let docks = self.state.visible_docks();
                if let Some(d) = docks.get(i) {
                    self.state.dock_focus = *d;
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
                    let val = match key.as_str() {
                        "journal_mode" => ["delete", "truncate", "persist", "memory", "wal", "off"]
                            .get(si)
                            .copied()
                            .unwrap_or("delete"),
                        "synchronous" => ["OFF", "NORMAL", "FULL", "EXTRA"]
                            .get(si)
                            .copied()
                            .unwrap_or("FULL"),
                        _ => continue,
                    };
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
                if let Some(i) = self.ui.list_selected(self.ids.ov_list) {
                    if Some(i) != self.last_wizard_list {
                        self.flush_wizard_col();
                        self.state.wizard_col_sel = i;
                        self.last_wizard_list = Some(i);
                        self.rebuild = true;
                    }
                }
                self.flush_wizard_col();
            }
            OverlayKind::EditIndex => {
                if self.ids.ov_idx_cols.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_idx_cols) {
                        self.state.wizard_index_cols = v.to_string();
                    }
                }
                if self.ids.ov_idx_unique.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_idx_unique) {
                        self.state.wizard_index_unique = on;
                    }
                }
                if self.ids.ov_idx_table.0 != 0 {
                    if let Some(i) = self.ui.select_selected(self.ids.ov_idx_table) {
                        let names = self.state.table_names();
                        if let Some(n) = names.get(i) {
                            self.state.wizard_index_table = n.clone();
                        }
                    }
                }
            }
            OverlayKind::ImportCsv | OverlayKind::PasteCsv => {
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
                if kind == OverlayKind::PasteCsv && self.ids.ov_csv_paste.0 != 0 {
                    if let Some(v) = self.ui.text_area_value(self.ids.ov_csv_paste) {
                        self.state.csv_paste = v.to_string();
                    }
                }
            }
            OverlayKind::FindReplace => {
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
                if self.ids.ov_prefs_delim.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_prefs_delim) {
                        self.state.prefs_csv_delim = v.to_string();
                    }
                }
                if self.ids.ov_frozen.0 != 0 {
                    if let Some(v) = self.ui.text_input_value(self.ids.ov_frozen) {
                        if let Ok(n) = v.trim().parse::<usize>() {
                            self.state.frozen_cols = n;
                        }
                    }
                }
            }
            OverlayKind::ExportSql => {
                if self.ids.ov_export_struct.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_export_struct) {
                        self.state.export_structure = on;
                    }
                }
                if self.ids.ov_export_data.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_export_data) {
                        self.state.export_data = on;
                    }
                }
                if self.ids.ov_export_drop.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_export_drop) {
                        self.state.export_drop = on;
                    }
                }
                if self.ids.ov_export_fk.0 != 0 {
                    if let Some(on) = self.ui.is_checked(self.ids.ov_export_fk) {
                        self.state.export_keep_fk = on;
                    }
                }
            }
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

    fn flush_wizard_col(&mut self) {
        let sel = self.state.wizard_col_sel;
        if sel >= self.state.wizard_cols.len() {
            return;
        }
        if self.ids.ov_col_name.0 != 0 {
            if let Some(v) = self.ui.text_input_value(self.ids.ov_col_name) {
                self.state.wizard_cols[sel].name = v.to_string();
            }
        }
        if self.ids.ov_col_type.0 != 0 {
            if let Some(i) = self.ui.select_selected(self.ids.ov_col_type) {
                let types = AppState::column_type_names();
                if let Some(ty) = types.get(i) {
                    self.state.wizard_cols[sel].type_name = ty.clone();
                }
            }
        }
        if self.ids.ov_pk.0 != 0 {
            if let Some(on) = self.ui.is_checked(self.ids.ov_pk) {
                self.state.wizard_cols[sel].pk = on;
            }
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
    }

    fn do_rebuild(&mut self) {
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
        self.last_wizard_list = self.ui.list_selected(self.ids.ov_list);
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
        self.state.open_create_table_wizard();
        self.rebuild = true;
    }

    fn action_create_index(&mut self) {
        self.state.open_create_index_wizard();
        self.rebuild = true;
    }

    fn action_import_csv(&mut self) {
        if let Some(path) = file_dlg::pick_open_csv() {
            self.state.csv_path = path.display().to_string();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("imported")
                .to_string();
            self.state.csv_table = stem;
            self.state.csv_has_header = true;
            self.state.csv_delim_idx = match self.state.prefs.csv_delimiter {
                ';' => 1,
                '\t' => 2,
                '|' => 3,
                _ => 0,
            };
            self.state.csv_quote_idx = 0;
            self.state.overlay = Some(OverlayKind::ImportCsv);
        } else {
            self.state.status_msg = "Import CSV cancelled".into();
        }
        self.rebuild = true;
    }

    fn action_export_json(&mut self) {
        self.snapshot_inputs();
        if let Some(path) = file_dlg::pick_save_json() {
            match self.state.export_browse(&path, true) {
                Ok(()) => {
                    self.state.status_msg = format!("Exported JSON {}", path.display());
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
        }
        self.rebuild = true;
    }

    fn action_export_csv(&mut self) {
        self.snapshot_inputs();
        let Some(path) = file_dlg::pick_save_csv() else {
            return;
        };
        match self.state.export_browse(&path, false) {
            Ok(()) => {
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

    fn action_export_sql(&mut self) {
        if !self.require_conn() {
            return;
        }
        self.state.overlay = Some(OverlayKind::ExportSql);
        self.rebuild = true;
    }

    fn action_export_sql_do(&mut self) {
        let Some(path) = file_dlg::pick_save_sql() else {
            return;
        };
        match self.state.export_sql_with_options(&path) {
            Ok(()) => {
                self.state.io_path = path.display().to_string();
                self.state.log_sql(format!("-- dump {}", path.display()));
                self.state.overlay = None;
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
        match self.state.open_project(&path) {
            Ok(()) => self.clear_error(),
            Err(e) => self.set_error(e),
        }
        self.rebuild = true;
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

    fn handle_menu(&mut self, item: u64) {
        if self.state.sql_busy {
            self.set_error("Stop or finish SQL first");
            self.rebuild = true;
            return;
        }
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
            mid::FILE_ATTACH => {
                let Some(path) = file_dlg::pick_open_db() else {
                    return;
                };
                self.state.attach_path = path.display().to_string();
                self.state.overlay = Some(OverlayKind::Attach);
                self.rebuild = true;
            }
            mid::FILE_IMPORT_CLIP => {
                self.state.csv_paste.clear();
                self.state.csv_table = "clipboard".into();
                self.state.csv_has_header = true;
                self.state.csv_delim_idx = match self.state.prefs.csv_delimiter {
                    ';' => 1,
                    '\t' => 2,
                    '|' => 3,
                    _ => 0,
                };
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
            mid::FILE_PROJECT_CLOSE => {
                self.state.close_project();
                self.rebuild = true;
            }
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
                self.snapshot_inputs();
                if self.state.project_path.is_none() {
                    let Some(path) = file_dlg::pick_save_project() else {
                        return;
                    };
                    if let Err(e) = self.state.save_project(&path) {
                        self.set_error(e);
                        self.rebuild = true;
                        return;
                    }
                }
                match self.state.save_all() {
                    Ok(()) => self.clear_error(),
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            mid::FILE_PRINT_SCHEMA => {
                if !self.require_conn() {
                    return;
                }
                let Some(path) = file_dlg::pick_save_sql() else {
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
                    match self.state.open_path(&path) {
                        Ok(()) => self.clear_error(),
                        Err(e) => self.set_error(e),
                    }
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
        if id == self.ids.ov_btn_cancel {
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
                            | Some(OverlayKind::ImportCsv)
                            | Some(OverlayKind::PasteCsv)
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
        if id == self.ids.ov_btn_add {
            self.flush_wizard_col();
            self.state
                .wizard_cols
                .push(dbmod::ColDef::new("col", "TEXT"));
            self.state.wizard_col_sel = self.state.wizard_cols.len() - 1;
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_del {
            self.flush_wizard_col();
            if !self.state.wizard_cols.is_empty() {
                let i = self
                    .state
                    .wizard_col_sel
                    .min(self.state.wizard_cols.len() - 1);
                self.state.wizard_cols.remove(i);
                if self.state.wizard_col_sel >= self.state.wizard_cols.len() {
                    self.state.wizard_col_sel = self.state.wizard_cols.len().saturating_sub(1);
                }
            }
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_up {
            self.flush_wizard_col();
            let i = self.state.wizard_col_sel;
            if i > 0 {
                self.state.wizard_cols.swap(i, i - 1);
                self.state.wizard_col_sel = i - 1;
            }
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_down {
            self.flush_wizard_col();
            let i = self.state.wizard_col_sel;
            if i + 1 < self.state.wizard_cols.len() {
                self.state.wizard_cols.swap(i, i + 1);
                self.state.wizard_col_sel = i + 1;
            }
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_find {
            self.snapshot_inputs();
            self.state.status_msg = if self.state.find_next_in_sql() {
                format!("Found at {}", self.state.find_pos)
            } else {
                "Not found".into()
            };
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_repl {
            self.snapshot_inputs();
            self.state.status_msg = if self.state.replace_one_in_sql() {
                "Replaced one".into()
            } else {
                "Not found".into()
            };
            self.rebuild = true;
            return true;
        }
        if id == self.ids.ov_btn_repl_all {
            self.snapshot_inputs();
            let n = self.state.replace_all_in_sql();
            self.state.status_msg = format!("Replaced {n}");
            self.rebuild = true;
            return true;
        }
        false
    }

    fn handle_button(&mut self, id: WidgetId) {
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
            self.state.dock_focus = DockKind::EditCell;
            self.rebuild = true;
            return;
        }
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
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            self.snapshot_inputs();
            let sql = self.state.ddl_draft.clone();
            match self.state.run_sql_text_sync(&sql) {
                Ok(_) => {
                    self.state.log_sql(sql);
                    self.state.status_msg = "DDL executed".into();
                    self.clear_error();
                }
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_page_prev || id == self.ids.btn_page_next {
            match self.state.browse_page(id == self.ids.btn_page_next) {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_go {
            self.snapshot_inputs();
            self.apply_browse_select();
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
            if let Err(e) = self.state.guard_writable() {
                self.set_error(e);
                self.rebuild = true;
                return;
            }
            self.snapshot_inputs();
            let Some(table) = self.state.browse().table.clone() else {
                self.set_error("No browse table");
                self.rebuild = true;
                return;
            };
            let Some(row) = self
                .state
                .browse()
                .sel_row
                .or_else(|| self.ui.table_selected_row(self.ids.browse_table))
            else {
                self.set_error("Select a browse row first");
                self.rebuild = true;
                return;
            };
            let Some(&rowid) = self.state.browse().rowids.get(row) else {
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
            let val = match self.state.edited_cell_value() {
                Ok(value) => value,
                Err(e) => {
                    self.set_error(e);
                    self.rebuild = true;
                    return;
                }
            };
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
            self.snapshot_inputs();
            match self.state.save_pragmas() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_browse_new_tab {
            self.state.add_browse_tab();
            self.rebuild = true;
            return;
        }
        for (i, bid) in self.ids.browse_tab_btns.iter().enumerate() {
            if id == *bid {
                self.snapshot_inputs();
                self.state.browse_tab_idx = i;
                let _ = self.state.refresh_browse();
                self.rebuild = true;
                return;
            }
        }
        if id == self.ids.btn_sql_csv || id == self.ids.btn_sql_json {
            let json = id == self.ids.btn_sql_json;
            let path = if json {
                file_dlg::pick_save_json()
            } else {
                file_dlg::pick_save_csv()
            };
            if let Some(path) = path {
                let result = self
                    .state
                    .conn
                    .as_ref()
                    .ok_or("No database open".to_string())
                    .and_then(|conn| {
                        let result = self.state.sql_result.as_ref().ok_or("Execute SQL first")?;
                        dbmod::export_query_result(conn, result, &path, json)
                    });
                match result {
                    Ok(()) => {
                        self.clear_error();
                        self.state.status_msg =
                            format!("SQL results exported to {}", path.display());
                    }
                    Err(e) => self.set_error(e),
                }
                self.rebuild = true;
            }
            return;
        }
        if id == self.ids.btn_sql_stop {
            self.state.request_sql_stop();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_find {
            self.state.overlay = Some(OverlayKind::FindReplace);
            self.rebuild = true;
            return;
        }
        for (i, bid) in self.ids.sql_tab_btns.iter().enumerate() {
            if id == *bid {
                self.snapshot_inputs();
                self.state.sql_tab_idx = i;
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
            self.snapshot_inputs();
            self.state.toggle_sql_line_comment();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_sql_view {
            self.snapshot_inputs();
            match self.state.save_results_as_view("e28_saved_view") {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_freeze {
            self.state.frozen_cols = if self.state.frozen_cols == 0 { 1 } else { 0 };
            self.state.status_msg = format!("Frozen columns = {}", self.state.frozen_cols);
            self.rebuild = true;
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
            self.state.set_edit_mode(crate::state::EditCellMode::Text);
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_hex {
            self.state.set_edit_mode(crate::state::EditCellMode::Hex);
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_json {
            self.state.set_edit_mode(crate::state::EditCellMode::Json);
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_edit_image {
            self.state.set_edit_mode(crate::state::EditCellMode::Image);
            self.state.decode_edit_image();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_form_view {
            self.state.toggle_browse_view();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_form_prev {
            self.state.form_prev_record();
            self.state.sync_edit_cell_from_selection();
            self.rebuild = true;
            return;
        }
        if id == self.ids.btn_form_next {
            self.state.form_next_record();
            self.state.sync_edit_cell_from_selection();
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
        if id == self.ids.btn_remote_open {
            if let Some(i) = self.ui.list_selected(self.ids.remote_list) {
                self.state.remote_sel = i;
            }
            match self.state.remote_open_selected() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
            self.rebuild = true;
            return;
        }
        if id == self.ids.toolbar_project {
            self.action_project_open();
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
            self.snapshot_inputs();
            match self.state.start_sql_job() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            }
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
            let Some(path) = file_dlg::pick_save_sql() else {
                return;
            };
            if let Some(conn) = self.state.conn.as_ref() {
                if let Err(e) = dbmod::guard_output_path(conn, &path) {
                    self.set_error(e);
                    self.rebuild = true;
                    return;
                }
            }
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
        if self.ids.browse_select.0 == 0 {
            return;
        }
        let opts = self.state.table_names();
        if let Some(i) = self.ui.select_selected(self.ids.browse_select) {
            if let Some(name) = opts.get(i) {
                let tab = self.state.browse_mut();
                if tab.table.as_ref() != Some(name) {
                    tab.col_filters.clear();
                    tab.result = None;
                    tab.offset = 0;
                }
                tab.table = Some(name.clone());
                tab.title = name.clone();
                tab.order_col = None;
                tab.sel_row = None;
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
                dirty = true;
            }
        }
        if self.ids.dock_tabs.0 != 0 {
            if let Some(i) = self.ui.tabs_selected(self.ids.dock_tabs) {
                let docks = self.state.visible_docks();
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
                self.apply_browse_select();
                let _ = self.state.refresh_browse();
                dirty = true;
            }
        }
        if let Some(row) = self.ui.table_selected_row(self.ids.browse_table) {
            let col = self.ui.table_selected_col(self.ids.browse_table);
            let col_changed = col.is_some() && col != self.state.browse().sel_col;
            if self.last_browse_row != Some(row) || col_changed {
                self.last_browse_row = Some(row);
                self.state.browse_mut().sel_row = Some(row);
                if let Some(c) = col {
                    self.state.browse_mut().sel_col = Some(c);
                }
                self.state.sync_edit_cell_from_selection();
                if matches!(self.state.edit_mode, crate::state::EditCellMode::Image) {
                    self.state.decode_edit_image();
                }
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
            let Some(res) = self.state.browse().result.as_ref() else {
                return;
            };
            let Some(col_name) = res.columns.get(col_idx).cloned() else {
                return;
            };
            let col_name = col_name
                .trim_end_matches(" ▲")
                .trim_end_matches(" ▼")
                .to_string();
            {
                let tab = self.state.browse_mut();
                tab.offset = 0;
                if tab.order_col.as_deref() == Some(col_name.as_str()) {
                    tab.order_desc = !tab.order_desc;
                } else {
                    tab.order_col = Some(col_name);
                    tab.order_desc = false;
                }
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
            self.apply_confirmed_action(kind);
        }
    }

    fn apply_confirmed_action(&mut self, kind: ConfirmKind) {
        match kind {
            ConfirmKind::CloseDb => {
                self.state.close_db();
            }
            ConfirmKind::Revert => match self.state.revert_changes() {
                Ok(()) => self.clear_error(),
                Err(e) => self.set_error(e),
            },
            ConfirmKind::ExitDirty => match self.state.revert_changes() {
                Ok(()) => self.exit_requested = true,
                Err(e) => self.set_error(e),
            },
            ConfirmKind::DropObject => {
                if let Some(sel) = self.state.selected.clone() {
                    if let Err(e) = self.state.guard_writable() {
                        self.set_error(e);
                    } else if let Some(conn) = self.state.conn.as_ref() {
                        match dbmod::drop_object(conn, sel.kind, &sel.name) {
                            Ok(()) => {
                                self.state
                                    .log_sql(format!("DROP {:?} {}", sel.kind, sel.name));
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
            }
            ConfirmKind::None => {}
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

    fn after_ui(&mut self, ctx: &mut WindowCtx<'_>) {
        while let Some(id) = self.ui.take_button_activate() {
            self.snapshot_inputs();
            self.handle_button(id);
        }
        while let Some((_owner, item)) = self.ui.take_menu_action() {
            self.snapshot_inputs();
            self.handle_menu(item);
        }
        self.handle_modal();
        if !self.rebuild {
            self.snapshot_inputs();
            self.poll_header_sort();
            self.poll_selection_changes();
        }
        if self.state.poll_sql_job() {
            self.rebuild = true;
        }
        if self.rebuild {
            self.do_rebuild();
        } else {
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
        !self.state.dirty && !self.state.sql_busy
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
            Event::CloseRequested { .. } => {
                if self.state.sql_busy {
                    self.state.request_sql_stop();
                } else {
                    self.handle_menu(mid::FILE_EXIT);
                }
                self.after_ui(ctx);
            }
            Event::Resized { width, height, .. } => {
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
                if modifiers.commandish() {
                    match key {
                        KeyCode::Char('c' | 'C') => {
                            if let Some(text) = self.ui.focused_text_for_copy() {
                                if let Err(e) = ctx.clipboard_set_text(&text) {
                                    self.set_error(e.to_string());
                                }
                            }
                            return;
                        }
                        KeyCode::Char('v' | 'V') => {
                            match ctx.clipboard_get_text() {
                                Ok(Some(text)) => {
                                    self.ui.paste_into_focused_text(&text);
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
                    if self.ui.key_input(ui_key) {
                        self.after_ui(ctx);
                        return;
                    }
                    if matches!(ui_key, UiKey::Escape) {
                        if self.state.overlay.is_some() {
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
        if self.state.sql_busy {
            if self.state.poll_sql_job() {
                self.snapshot_inputs();
                self.rebuild = true;
                self.do_rebuild();
            } else {
                ctx.request_redraw();
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

#[cfg(test)]
mod interaction_tests {
    use super::*;

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
            Some("-- new tab\n")
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
        demo.ui.set_text_input(demo.ids.edit_value, "form edit");
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
