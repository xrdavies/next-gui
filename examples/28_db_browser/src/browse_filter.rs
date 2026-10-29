//! Debounced Browse filters use the existing connection and cancellable database worker.
use crate::{
    db,
    export_jobs::Job,
    sql_log::LoggedConnection,
    state::{AppState, BrowseTabState},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(crate) fn fetch(
    conn: &LoggedConnection,
    mut candidate: BrowseTabState,
    is_view: bool,
    filter_escape: &str,
) -> Result<BrowseTabState, String> {
    let schema = candidate.schema.clone();
    let table = candidate.table.clone().ok_or("Select a table first")?;
    let mut options = candidate.options();
    let columns = options.remap_source(conn, &schema, &table, options.show_row_id)?;
    let rowid_column = db::implicit_rowid_column_in(conn, &schema, &table)?;
    candidate.set_options(options);
    candidate.rowid_column = rowid_column.clone();
    let ordering = candidate.ordering();

    let where_sql = AppState::composed_where(&candidate, &columns, filter_escape)?;

    let total_rows = db::count_browse_rows_in(conn, &schema, &table, where_sql.as_deref())?;
    let last_page = total_rows.saturating_sub(1) / db::DEFAULT_LIMIT * db::DEFAULT_LIMIT;
    let offset = candidate.offset.min(last_page);
    let (total_rows, mut result, row_keys) = {
        if is_view && candidate.unlock_view_pk.is_some() {
            let (result, keys) = db::browse_view_with_key_in(
                conn,
                &schema,
                &table,
                candidate.unlock_view_pk.as_deref().unwrap(),
                &ordering,
                where_sql.as_deref(),
                (db::DEFAULT_LIMIT, offset),
            )?;
            (total_rows, result, keys)
        } else if is_view {
            (
                total_rows,
                db::browse_table_in(
                    conn,
                    &schema,
                    &table,
                    &ordering,
                    where_sql.as_deref(),
                    db::DEFAULT_LIMIT,
                    offset,
                )?,
                Vec::new(),
            )
        } else {
            match db::browse_with_keys_in(
                conn,
                &schema,
                &table,
                &ordering,
                where_sql.as_deref(),
                db::DEFAULT_LIMIT,
                offset,
            ) {
                Ok((q, row_keys)) => (total_rows, q, row_keys),
                Err(_) => (
                    total_rows,
                    db::browse_table_in(
                        conn,
                        &schema,
                        &table,
                        &ordering,
                        where_sql.as_deref(),
                        db::DEFAULT_LIMIT,
                        offset,
                    )?,
                    Vec::new(),
                ),
            }
        }
    };

    if candidate.show_row_id {
        db::show_rowid_column(
            &mut result,
            &row_keys,
            rowid_column
                .as_deref()
                .ok_or("This source has no accessible rowid")?,
        )?;
    }

    candidate
        .col_filters
        .resize(result.columns.len(), String::new());
    candidate.offset = offset;
    candidate.title = if schema == "main" {
        table.clone()
    } else {
        db::qualified_name(&schema, &table)
    };
    candidate.row_keys = row_keys;
    candidate.rowid_column = rowid_column;
    candidate.total_rows = Some(total_rows);
    candidate.result = Some(result);
    candidate.clear_selection();
    Ok(candidate)
}

pub(crate) fn status(tab: &BrowseTabState) -> String {
    let schema = &tab.schema;
    let table = tab.table.as_ref().unwrap();
    let result = tab.result.as_ref().unwrap();
    let nrows = result.rows.len();
    let offset = tab.offset;
    let total_rows = tab.total_rows.unwrap_or(0);
    let truncated = result.truncated;
    let ordering = tab.ordering();
    let order_note = if ordering.is_empty() {
        String::new()
    } else {
        format!(
            " ORDER BY {}",
            ordering
                .iter()
                .map(|order| format!(
                    "{} {}",
                    db::quote_ident(&order.column),
                    if order.descending { "DESC" } else { "ASC" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let status = format!(
        "Browse {} — {} rows {}–{} of {}{}{}",
        if schema == "main" {
            table.clone()
        } else {
            db::qualified_name(schema, table)
        },
        nrows,
        if nrows == 0 {
            0
        } else {
            offset.saturating_add(1)
        },
        offset.saturating_add(nrows),
        total_rows,
        if truncated { " (more pages)" } else { "" },
        order_note
    );
    status
}

#[derive(Clone)]
struct Source {
    tab: usize,
    schema: String,
    table: Option<String>,
    result: Option<Arc<()>>,
}
impl Source {
    fn current(state: &AppState) -> Self {
        Self {
            tab: state.browse_tab_idx,
            schema: state.browse().schema.clone(),
            table: state.browse().table.clone(),
            result: state.browse().result.as_ref().map(|r| r.values.identity()),
        }
    }
    fn matches(&self, state: &AppState) -> bool {
        self.tab == state.browse_tab_idx
            && self.schema == state.browse().schema
            && self.table == state.browse().table
            && match (&self.result, state.browse().result.as_ref()) {
                (Some(old), Some(current)) => Arc::ptr_eq(old, &current.values.identity()),
                (None, None) => true,
                _ => false,
            }
    }
}

pub struct Work {
    job: Option<Job<BrowseTabState>>,
    cancel: Arc<AtomicBool>,
    source: Source,
}
impl Drop for Work {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            self.cancel.store(true, Ordering::Relaxed);
            let _ = job.join();
        }
    }
}
impl AppState {
    pub(crate) fn start_browse_filter(&mut self) -> Result<(), String> {
        self.guard_background_idle()?;
        if self.sql_busy {
            return Err("Wait for SQL to finish before filtering".into());
        }
        if self.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        let source = Source::current(self);
        let mut candidate = self.browse().clone();
        candidate.offset = 0;
        let is_view = self.schema.iter().any(|o| {
            o.schema == candidate.schema
                && Some(&o.name) == candidate.table.as_ref()
                && o.kind == db::ObjectKind::View
        });
        let escape = self.prefs.filter_escape.clone();
        let conn = self.conn.take().ok_or("No idle database available")?;
        let cancel = Arc::new(AtomicBool::new(false));
        let interrupted = cancel.clone();
        let job = Job::start_task(conn, move |conn| {
            let progress = interrupted.clone();
            conn.progress_handler(500, Some(move || progress.load(Ordering::Relaxed)));
            let result = if interrupted.load(Ordering::Relaxed) {
                Err("Browse filtering cancelled".into())
            } else {
                fetch(conn, candidate, is_view, &escape)
            };
            conn.progress_handler(0, None::<fn() -> bool>);
            result
        });
        self.browse_work = Some(Work {
            job: Some(job),
            cancel,
            source,
        });
        Ok(())
    }

    pub(crate) fn cancel_browse_filter(&mut self) {
        if let Some(work) = self.browse_work.as_ref() {
            work.cancel.store(true, Ordering::Relaxed);
        }
        // Restore the same connection before a navigation or database action uses it.
        if self.browse_work.is_some() {
            if let Err(error) = self.finish_browse_filter() {
                self.error_msg = Some(error);
            }
        }
    }

    fn finish_browse_filter(&mut self) -> Result<bool, String> {
        let Some(mut work) = self.browse_work.take() else {
            return Ok(false);
        };
        let (conn, result) = work.job.take().unwrap().join();
        self.conn = conn;
        if self.conn.is_none() {
            self.close_db();
        }
        self.recover_ended_transaction()?;
        if !work.cancel.load(Ordering::Relaxed) && work.source.matches(self) {
            let mut candidate = result?;
            let current = self.browse();
            candidate.column_widths = current.column_widths.clone();
            candidate.frozen_cols = current.frozen_cols;
            candidate.scroll = current.scroll;
            candidate.table_options = current.table_options.clone();
            candidate.schema_table_options = current.schema_table_options.clone();
            self.status_msg = status(&candidate);
            *self.browse_mut() = candidate;
            self.clear_browse_editor();
            return Ok(true);
        } else if self.conn.is_none() {
            result?;
        }
        Ok(false)
    }

    pub(crate) fn poll_browse_filter(&mut self) -> Result<bool, String> {
        if self
            .browse_work
            .as_ref()
            .is_some_and(|work| work.job.as_ref().is_some_and(Job::is_finished))
        {
            self.finish_browse_filter()
        } else {
            Ok(false)
        }
    }
}

use crate::{state::MainTab, BuiltUiIds, Demo};
use next_gui_ui::{TextInputState, Ui, WidgetId};
use std::time::{Duration, Instant};

fn drafts(ui: &Ui, ids: &BuiltUiIds) -> Vec<String> {
    std::iter::once(ids.browse_global_filter)
        .chain(ids.browse_col_filters.iter().copied())
        .map(|id| ui.text_input_value(id).unwrap_or_default().to_string())
        .collect()
}
fn focused(ui: &Ui, ids: &BuiltUiIds) -> Option<WidgetId> {
    ui.focused().filter(|id| {
        id.0 != 0 && (*id == ids.browse_global_filter || ids.browse_col_filters.contains(id))
    })
}

pub(crate) struct Session {
    source: Source,
    observed: Vec<String>,
    escape: String,
    pending: bool,
    deadline: Option<Instant>,
    focus: Option<WidgetId>,
}
impl Session {
    pub(crate) fn new(state: &AppState, ui: &Ui, ids: &BuiltUiIds) -> Self {
        Self {
            source: Source::current(state),
            observed: drafts(ui, ids),
            escape: state.prefs.filter_escape.clone(),
            pending: false,
            deadline: None,
            focus: focused(ui, ids),
        }
    }
    pub(crate) fn reconcile(&mut self, state: &AppState, ui: &Ui, ids: &BuiltUiIds) {
        if !self.source.matches(state) {
            *self = Self::new(state, ui, ids);
        }
    }
}

pub(crate) struct InputSession {
    source: Source,
    kind: usize,
    column: Option<String>,
    value: String,
    state: TextInputState,
}
impl InputSession {
    pub(crate) fn capture(state: &AppState, ui: &Ui, ids: &BuiltUiIds) -> Option<Self> {
        if state.tab != MainTab::Browse || state.overlay.is_some() || state.confirm_open {
            return None;
        }
        let id = ui.focused()?;
        let (kind, column) = if id.0 == 0 {
            return None;
        } else if id == ids.browse_global_filter {
            (0, None)
        } else if id == ids.browse_filter {
            (1, None)
        } else {
            let index = ids
                .browse_col_filters
                .iter()
                .position(|input| *input == id)?;
            (
                2,
                Some(state.browse().result.as_ref()?.columns.get(index)?.clone()),
            )
        };
        Some(Self {
            source: Source::current(state),
            kind,
            column,
            value: ui.text_input_value(id)?.to_string(),
            state: ui.text_input_state(id)?,
        })
    }
    pub(crate) fn restore(self, state: &AppState, ui: &mut Ui, ids: &BuiltUiIds) {
        if state.tab != MainTab::Browse
            || state.overlay.is_some()
            || state.confirm_open
            || self.source.tab != state.browse_tab_idx
            || self.source.schema != state.browse().schema
            || self.source.table != state.browse().table
        {
            return;
        }
        let id = match self.kind {
            0 => ids.browse_global_filter,
            1 => ids.browse_filter,
            _ => {
                let Some(index) = state.browse().result.as_ref().and_then(|r| {
                    r.columns
                        .iter()
                        .position(|name| Some(name) == self.column.as_ref())
                }) else {
                    return;
                };
                let Some(id) = ids.browse_col_filters.get(index) else {
                    return;
                };
                *id
            }
        };
        let Some(current) = ui.text_input_value(id) else {
            return;
        };
        let compatible = current == self.value
            || self.kind == 0
                && current.trim().split(' ').filter(|s| !s.is_empty()).eq(self
                    .value
                    .trim()
                    .split(' ')
                    .filter(|s| !s.is_empty()));
        if compatible {
            if current != self.value { ui.set_text_input(id, &self.value); }
            ui.set_focus(Some(id));
            ui.restore_text_input_state(id, &self.state);
        }
    }
}

impl Demo {
    pub(crate) fn flush_browse_filter(&mut self) {
        self.filter_session.reconcile(&self.state, &self.ui, &self.ids);
        self.filter_session.observed = drafts(&self.ui, &self.ids);
        self.filter_session.escape = self.state.prefs.filter_escape.clone();
        self.filter_session.focus = focused(&self.ui, &self.ids);
        self.filter_session.pending = true;
        self.filter_session.deadline = Some(Instant::now());
    }

    pub(crate) fn cancel_browse_filter(&mut self) {
        if self.state.browse_work.is_some() {
            self.filter_session.pending = true;
            self.filter_session.deadline = None;
            self.state.cancel_browse_filter();
        }
    }
    pub(crate) fn advance_browse_filter(&mut self, now: Instant) -> Option<Duration> {
        self.filter_session
            .reconcile(&self.state, &self.ui, &self.ids);
        let current = drafts(&self.ui, &self.ids);
        let focus = focused(&self.ui, &self.ids);
        let lost_focus = self.filter_session.focus.is_some() && self.filter_session.focus != focus;
        self.filter_session.focus = focus;
        if current != self.filter_session.observed
            || self.filter_session.escape != self.state.prefs.filter_escape
        {
            self.filter_session.escape = self.state.prefs.filter_escape.clone();
            self.filter_session.observed = current;
            self.filter_session.pending = true;
            self.filter_session.deadline =
                Some(now + Duration::from_millis(self.state.prefs.filter_delay_ms));
            if let Some(work) = self.state.browse_work.as_ref() {
                work.cancel.store(true, Ordering::Relaxed);
            }
        }
        let blocked = self.state.tab != MainTab::Browse
            || self.state.overlay.is_some()
            || self.state.confirm_open
            || self.state.sql_busy
            || self.state.csv_work.is_some()
            || self.state.export_work.is_some()
            || self.state.plot_work.is_some()
            || self.state.has_cell_draft()
            || self.ui.ime_is_composing()
            || self.ui.popup_owner().is_some()
            || self.state.browse().table.is_none();
        if blocked {
            self.filter_session.deadline = None;
            if self.state.browse_work.is_some() {
                self.filter_session.pending = true;
            }
            if let Some(work) = self.state.browse_work.as_ref() {
                work.cancel.store(true, Ordering::Relaxed);
            }
        }
        let was_running = self.state.browse_work.is_some();
        match self.state.poll_browse_filter() {
            Ok(true) => {
                self.clear_error();
                self.rebuild = true;
            }
            Err(error) => {
                self.set_error(error);
                self.rebuild = true;
            }
            Ok(false) => {}
        }
        if was_running && self.state.browse_work.is_none() {
            self.rebuild = true;
        }
        if self.state.browse_work.is_some() {
            return Some(Duration::from_millis(16));
        }
        if blocked || !self.filter_session.pending || self.state.conn.is_none() {
            return None;
        }
        let deadline = *self
            .filter_session
            .deadline
            .get_or_insert(now + Duration::from_millis(self.state.prefs.filter_delay_ms));
        if lost_focus || now >= deadline {
            self.filter_session.pending = false;
            self.filter_session.deadline = None;
            self.snapshot_inputs();
            match self.state.start_browse_filter() {
                Ok(()) => {}
                Err(error) => self.set_error(error),
            }
            self.rebuild = true;
            Some(Duration::from_millis(16))
        } else {
            Some(deadline.saturating_duration_since(now))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{state::OverlayKind, ui_build, PresentBackend};
    use next_gui_ui::{Theme, UiKey};

    pub(crate) fn fixture(width: u32, height: u32, theme: Theme) -> Demo {
        let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
        demo.theme = ui_build::browser_theme(theme, if width == 800 { 18.0 } else { 13.0 });
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE names(name TEXT, amount INTEGER); INSERT INTO names VALUES('alpha',1),('alpine',2),('beta',3),('A_B',4),('A%B',5); CREATE TABLE other(v); INSERT INTO other VALUES(42);").unwrap();
        demo.state.dirty = true;
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("main", "names").unwrap();
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        demo
    }
    pub(crate) fn settle(demo: &mut Demo, now: Instant) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while demo.state.browse_work.is_some() {
            assert!(
                Instant::now() < deadline,
                "Browse filter worker did not finish"
            );
            demo.advance_browse_filter(now);
            std::thread::sleep(Duration::from_millis(1));
        }
        if demo.rebuild {
            demo.do_rebuild();
        }
    }
    pub(crate) fn capture(demo: &mut Demo, name: &str) {
        use next_gui_render::{encode_scene, Renderer};
        if let Ok(out) = std::env::var("NEXT_GUI_FILTER_QA_OUT") {
            demo.ui.sync(&mut demo.scene);
            demo.scene
                .mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
            let dirty = demo.scene.take_dirty();
            let commands = encode_scene(&demo.scene, &dirty);
            let clear = crate::ui_build::clear_color(&demo.theme);
            let mut soft = crate::SoftRenderer::new(demo.win_w, demo.win_h, clear);
            soft.render(&commands, &dirty);
            crate::write_ppm(
                &format!("{out}/window-{name}.ppm"),
                soft.frame_pixels().unwrap().0,
                demo.win_w,
                demo.win_h,
            )
            .unwrap();
            let mut gpu = crate::GpuRenderer::try_new(demo.win_w, demo.win_h, clear)
                .expect("Metal renderer required for filter QA");
            gpu.render(&commands, &dirty);
            crate::write_ppm(
                &format!("{out}/window-{name}-metal.ppm"),
                gpu.frame_pixels().unwrap().0,
                demo.win_w,
                demo.win_h,
            )
            .unwrap();
        }
    }

    #[test]
    fn automatic_filters_debounce_preserve_undo_and_retain_failed_results() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = fixture(width, height, theme);
                let now = Instant::now();
                let input = demo.ids.browse_global_filter;
                demo.ui.set_focus(Some(input));
                demo.advance_browse_filter(now);
                for c in "alp".chars() {
                    assert!(demo.ui.key_input(UiKey::Char(c)));
                }
                assert_eq!(
                    demo.advance_browse_filter(now),
                    Some(Duration::from_millis(200))
                );
                assert!(demo.state.browse_work.is_none());
                assert_eq!(demo.state.browse().total_rows, Some(5));
                capture(&mut demo, &format!("pending-{theme_name}-{width}"));
                demo.advance_browse_filter(now + Duration::from_millis(199));
                assert!(demo.state.browse_work.is_none());
                demo.advance_browse_filter(now + Duration::from_millis(200));
                assert!(demo.state.browse_work.is_some());
                settle(&mut demo, now + Duration::from_millis(201));
                assert_eq!(demo.state.browse().total_rows, Some(2));
                assert_eq!(demo.ui.focused(), Some(demo.ids.browse_global_filter));
                demo.ui.key_input(UiKey::Char('h'));
                demo.advance_browse_filter(now + Duration::from_millis(300));
                demo.advance_browse_filter(now + Duration::from_millis(500));
                settle(&mut demo, now + Duration::from_millis(501));
                assert_eq!(demo.state.browse().total_rows, Some(1));
                assert!(demo.ui.key_input(UiKey::Undo));
                assert_eq!(
                    demo.ui.text_input_value(demo.ids.browse_global_filter),
                    Some("alp")
                );
                demo.advance_browse_filter(now + Duration::from_millis(600));
                demo.ui.set_focus(Some(demo.ids.browse_col_filters[1]));
                demo.advance_browse_filter(now + Duration::from_millis(601));
                assert!(
                    demo.state.browse_work.is_some(),
                    "focus loss flushes immediately"
                );
                settle(&mut demo, now + Duration::from_millis(602));
                assert_eq!(demo.state.browse().total_rows, Some(2));
                capture(&mut demo, &format!("applied-{theme_name}-{width}"));
                let previous = demo
                    .state
                    .browse()
                    .result
                    .as_ref()
                    .unwrap()
                    .values
                    .identity();
                demo.ui
                    .set_text_input(demo.ids.browse_filter, "missing_column = 1");
                demo.ui
                    .set_text_input(demo.ids.browse_global_filter, "beta");
                demo.advance_browse_filter(now + Duration::from_millis(700));
                demo.advance_browse_filter(now + Duration::from_millis(900));
                settle(&mut demo, now + Duration::from_millis(901));
                assert!(Arc::ptr_eq(
                    &previous,
                    &demo
                        .state
                        .browse()
                        .result
                        .as_ref()
                        .unwrap()
                        .values
                        .identity()
                ));
                assert_eq!(
                    demo.ui.text_input_value(demo.ids.browse_global_filter),
                    Some("beta")
                );
                assert!(demo.state.error_msg.is_some());
                assert_eq!(
                    demo.advance_browse_filter(now + Duration::from_secs(20)),
                    None,
                    "failed criteria are not retried"
                );
                capture(&mut demo, &format!("error-{theme_name}-{width}"));
            }
        }
    }

    #[test]
    fn automatic_filters_suspend_composition_drafts_modals_and_cancel_stale_workers() {
        let mut demo = fixture(1400, 860, Theme::dark());
        let now = Instant::now();
        demo.state.prefs.filter_delay_ms = 0;
        demo.ui.set_focus(Some(demo.ids.browse_global_filter));
        demo.advance_browse_filter(now);
        demo.ui.key_input(UiKey::Char('a'));
        demo.ui.ime_preedit("中文", None);
        assert_eq!(demo.advance_browse_filter(now), None);
        assert!(demo.state.browse_work.is_none());
        demo.ui.ime_preedit("", None);
        demo.state.confirm_open = true;
        assert_eq!(demo.advance_browse_filter(now), None);
        demo.state.confirm_open = false;
        demo.snapshot_inputs();
        demo.state.browse_mut().sel_row = Some(0);
        demo.state.browse_mut().sel_col = Some(0);
        demo.state.sync_edit_cell_from_selection();
        demo.state.edit_value = "pending cell".into();
        assert!(demo.state.has_cell_draft());
        assert_eq!(demo.advance_browse_filter(now), None);
        demo.state.discard_cell_draft();
        demo.advance_browse_filter(now);
        assert!(demo.state.browse_work.is_some());
        demo.ui
            .set_text_input(demo.ids.browse_global_filter, "beta");
        demo.advance_browse_filter(now);
        settle(&mut demo, now);
        // Cancellation restores the original transaction, including uncommitted rows.
        if demo.filter_session.pending {
            demo.advance_browse_filter(now);
            settle(&mut demo, now);
        }
        assert_eq!(demo.state.browse().total_rows, Some(1));
        assert_eq!(
            demo.state.browse().result.as_ref().unwrap().rows[0][0],
            "beta"
        );
        assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());
        assert_eq!(
            db::count_rows(demo.state.conn.as_ref().unwrap(), "names").unwrap(),
            5
        );
        demo.ui
            .set_text_input(demo.ids.browse_global_filter, "alpine");
        demo.advance_browse_filter(now);
        assert!(demo.state.browse_work.is_some());
        demo.state.cancel_browse_filter();
        demo.state.switch_browse_table_in("main", "other").unwrap();
        demo.do_rebuild();
        assert_eq!(
            demo.advance_browse_filter(now + Duration::from_secs(10)),
            None
        );
        assert_eq!(
            demo.state.browse().result.as_ref().unwrap().rows[0][0],
            "42"
        );
    }

    #[test]
    fn expensive_filter_cancellation_restores_the_pending_connection() {
        let mut demo = fixture(1400, 860, Theme::dark());
        let previous = demo
            .state
            .browse()
            .result
            .as_ref()
            .unwrap()
            .values
            .identity();
        demo.state.browse_mut().filter = "amount > (WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100000000) SELECT max(x) FROM n)".into();
        demo.state.start_browse_filter().unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert!(!demo
            .state
            .browse_work
            .as_ref()
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .is_finished());
        let started = Instant::now();
        demo.state.cancel_browse_filter();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(demo.state.browse_work.is_none());
        let conn = demo.state.conn.as_ref().unwrap();
        assert!(!conn.is_autocommit());
        assert_eq!(db::count_rows(conn, "names").unwrap(), 5);
        assert!(Arc::ptr_eq(
            &previous,
            &demo
                .state
                .browse()
                .result
                .as_ref()
                .unwrap()
                .values
                .identity()
        ));
        demo.state.browse_mut().filter.clear();
        demo.state.write_changes().unwrap();
        demo.state.refresh_browse().unwrap();
        assert_eq!(demo.state.browse().total_rows, Some(5));
    }

    #[test]
    fn filter_preferences_validate_atomically_cancel_and_save_escape() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = fixture(width, height, theme);
                demo.state.sync_prefs_drafts();
                let original = serde_json::to_string(&demo.state.prefs).unwrap();
                demo.state.prefs_font_size = "21".into();
                for (delay, escape) in [("5001", "!"), ("-1", "!"), ("200", "two"), ("200", "\0")] {
                    demo.state.prefs_filter_delay = delay.into();
                    demo.state.prefs_filter_escape = escape.into();
                    assert!(demo.state.apply_prefs_overlay().is_err());
                    assert_eq!(serde_json::to_string(&demo.state.prefs).unwrap(), original);
                }
                demo.state.sync_prefs_drafts();
                demo.state.overlay = Some(OverlayKind::Preferences);
                demo.do_rebuild();
                demo.ui
                    .set_text_input(demo.ids.ov_prefs_filter_delay, "350");
                demo.ui.set_text_input(demo.ids.ov_prefs_filter_escape, "!");
                demo.handle_button(demo.ids.ov_btn_cancel);
                demo.do_rebuild();
                assert_eq!(demo.state.prefs.filter_delay_ms, 200);
                assert_eq!(demo.state.prefs.filter_escape, "\\");
                demo.state.sync_prefs_drafts();
                demo.state.overlay = Some(OverlayKind::Preferences);
                demo.do_rebuild();
                demo.ui
                    .set_text_input(demo.ids.ov_prefs_filter_delay, "350");
                demo.ui.set_text_input(demo.ids.ov_prefs_filter_escape, "!");
                demo.ui.set_scroll_offset(demo.ids.ov_scroll, 0.0, 150.0);
                capture(&mut demo, &format!("preferences-{theme_name}-{width}"));
                demo.handle_button(demo.ids.ov_btn_ok);
                demo.do_rebuild();
                assert_eq!(demo.state.prefs.filter_delay_ms, 350);
                assert_eq!(demo.state.prefs.filter_escape, "!");
                demo.ui
                    .set_text_input(demo.ids.browse_col_filters[0], "A!%B");
                let now = Instant::now();
                demo.advance_browse_filter(now);
                demo.advance_browse_filter(now + Duration::from_millis(350));
                settle(&mut demo, now + Duration::from_millis(351));
                assert_eq!(demo.state.browse().total_rows, Some(1));
                assert_eq!(
                    demo.state.browse().result.as_ref().unwrap().rows[0][0],
                    "A%B"
                );
            }
        }
    }
}
