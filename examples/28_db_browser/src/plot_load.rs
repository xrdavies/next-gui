//! Complete Plot snapshots on the existing connection without replaying SQL results.
use crate::{db, export_jobs::Job, plot, sql_log::LoggedConnection, state::{AppState, MainTab}};
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};

pub const CANCELLED: &str = "Plot loading cancelled";

#[derive(Default)]
pub struct Progress {
    cancel: AtomicBool,
    rows: AtomicUsize,
}
impl Progress {
    pub fn cancel(&self) { self.cancel.store(true, Ordering::Relaxed); }
    pub fn cancelled(&self) -> bool { self.cancel.load(Ordering::Relaxed) }
    fn check(&self, rows: usize) -> Result<(), String> {
        if self.cancelled() { return Err(CANCELLED.into()); }
        self.rows.store(rows, Ordering::Relaxed);
        Ok(())
    }
    fn caption(&self) -> String {
        format!("{}\n{} rows processed\nCancel keeps the previous plot and pending database changes.",
            if self.cancelled() { "Cancelling plot load…" } else { "Reading the complete result and preparing the plot…" }, self.rows.load(Ordering::Relaxed))
    }
}

pub struct Work {
    job: Option<Job<plot::Model>>,
    pub progress: Arc<Progress>,
}
impl Work {
    fn finished(&self) -> bool { self.job.as_ref().is_some_and(Job::is_finished) }
    fn join(mut self) -> (Option<LoggedConnection>, Result<plot::Model, String>) { self.job.take().unwrap().join() }
}
impl Drop for Work {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() { self.progress.cancel(); let _ = job.join(); }
    }
}

impl AppState {
    pub fn start_plot_load(&mut self) -> Result<(), String> {
        self.guard_background_idle()?;
        if self.sql_busy { return Err("Stop or finish SQL before loading the plot".into()); }
        if self.plot.settings.manual { return Err("Select Current data to load all plot rows".into()); }
        let (source, preview, offset, total, browse) = if self.tab == MainTab::ExecuteSql {
            (plot::Source::Query(self.sql_tab_idx), self.sql_result.as_ref().ok_or("Execute a query first")?.clone(), 0, None, None)
        } else {
            let tab = self.browse();
            let name = tab.table.as_ref().ok_or("Select a table first")?;
            let result = tab.result.as_ref().ok_or("No Browse result")?.clone();
            let filter = Self::composed_where(tab, &result.columns)?;
            (plot::Source::Table { schema: tab.schema.clone(), table: name.clone() }, result, tab.offset, tab.total_rows, Some((tab.ordering(), filter)))
        };
        let cached = self.plot.full_matches(&source, &preview);
        if !cached { self.plot.sync(source.clone(), &preview, offset, total)?; }
        let mut model = self.plot.clone();
        let conn = self.conn.take().ok_or("No idle database available")?;
        let progress = Arc::new(Progress::default());
        let worker_progress = progress.clone();
        let job = Job::start_task(conn, move |conn| {
            let interrupted = worker_progress.clone();
            conn.progress_handler(500, Some(move || interrupted.cancelled()));
            let result = (|| {
                worker_progress.check(0)?;
                let versions = if browse.is_some() && !cached { Some(db::data_versions(conn)?) } else { None };
                let input = if let (plot::Source::Table { schema, table }, Some((ordering, filter))) = (&source, browse) {
                    if cached { preview.clone() } else { db::browse_snapshot_in(conn, schema, table, &ordering, filter.as_deref())? }
                } else { preview.clone() };
                model.load_all(source, &preview, &input, |rows| worker_progress.check(rows))?;
                if let Some(versions) = versions {
                    if versions != db::data_versions(conn)? { return Err("Database changed while loading the plot; refresh and try again".into()); }
                    model.set_full_versions(versions);
                }
                Ok(model)
            })();
            conn.progress_handler(0, None::<fn() -> bool>);
            if worker_progress.cancelled() { Err(CANCELLED.into()) } else { result }
        });
        self.plot_work = Some(Work { job: Some(job), progress });
        Ok(())
    }

    pub fn poll_plot_load(&mut self) -> Result<bool, String> {
        if !self.plot_work.as_ref().is_some_and(Work::finished) { return Ok(false); }
        let work = self.plot_work.take().unwrap();
        let cancelled = work.progress.cancelled();
        let (conn, result) = work.join();
        let result = if cancelled { Err(CANCELLED.into()) } else { result };
        self.conn = conn;
        if self.conn.is_none() { self.close_db(); }
        if let Err(error) = self.recover_ended_transaction() { self.plot.reject_full_changes(); return Err(error); }
        match result {
            Ok(model) => { self.plot = model; self.status_msg = format!("Plot loaded · {}", self.plot.summary); }
            Err(error) => {
                self.plot.reject_full_changes();
                if error == CANCELLED { self.status_msg = CANCELLED.into(); } else { return Err(error); }
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub dialog: next_gui_ui::WidgetId,
    text: next_gui_ui::WidgetId,
    cancel: next_gui_ui::WidgetId,
}
impl Default for Controls {
    fn default() -> Self { Self { dialog: next_gui_ui::WidgetId(0), text: next_gui_ui::WidgetId(0), cancel: next_gui_ui::WidgetId(0) } }
}

pub fn build(ui: &mut next_gui_ui::Ui, state: &AppState, theme: &next_gui_ui::Theme, width: f32, height: f32) -> Controls {
    let Some(work) = &state.plot_work else { return Controls::default(); };
    let mut style = next_gui_ui::ModalStyle::from_theme(theme);
    style.width = (width - 80.0).min(620.0);
    style.height = (height - 48.0).min(230.0);
    style.close_on_backdrop = false;
    let mut message = next_gui_ui::TextAreaStyle::from_theme(theme);
    message.width = style.width - 48.0;
    message.height = 100.0;
    let mut bar = next_gui_ui::ProgressStyle::from_theme(theme);
    bar.width = message.width;
    let mut ids = Controls::default();
    ids.dialog = ui.dialog("Load all plot rows", style, |body| {
        ids.text = body.text_area(work.progress.caption(), "", message);
        body.progress_styled(0.0, true, bar);
    }, |footer| { ids.cancel = footer.tool_button("Cancel"); });
    ui.set_text_area_read_only(ids.text, true);
    ui.set_accessible_name(ids.text, "Complete plot loading progress");
    ui.set_accessible_name(ids.cancel, "Cancel complete plot loading");
    ui.set_button_enabled(ids.cancel, !work.progress.cancelled());
    ui.set_dialog_open(ids.dialog, true);
    ui.set_focus(Some(ids.cancel));
    ids
}

impl crate::Demo {
    pub(crate) fn cancel_plot_load(&mut self) {
        if let Some(work) = &self.state.plot_work { work.progress.cancel(); self.rebuild = true; }
    }
    pub(crate) fn handle_plot_load_button(&mut self, id: next_gui_ui::WidgetId) {
        if id == self.ids.plot_load.cancel { self.cancel_plot_load(); }
    }
    pub(crate) fn handle_plot_load_dismiss(&mut self) {
        if self.ui.take_dialog_dismiss() == Some(self.ids.plot_load.dialog) { self.cancel_plot_load(); }
    }
    pub(crate) fn poll_plot_load(&mut self) {
        match self.state.poll_plot_load() {
            Ok(true) => { self.clear_error(); self.rebuild = true; }
            Ok(false) => {
                if let Some(work) = &self.state.plot_work {
                    self.ui.set_text_area_value(self.ids.plot_load.text, work.progress.caption());
                    self.ui.set_button_enabled(self.ids.plot_load.cancel, !work.progress.cancelled());
                    self.ui.sync(&mut self.scene);
                    self.needs_paint = true;
                }
            }
            Err(error) => { self.set_error(error); self.rebuild = true; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn settle(state: &mut AppState) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.plot_work.is_some() {
            assert!(Instant::now() < deadline, "Plot worker did not finish");
            state.poll_plot_load()?;
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn full_browse_keeps_namespaces_filter_order_pages_and_invalidates_after_refresh() {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE t(x,y); CREATE TEMP TABLE t(x,y); ATTACH ':memory:' AS aux; CREATE TABLE aux.t(x,y); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1203) INSERT INTO aux.t SELECT x,x*2 FROM n").unwrap();
        state.refresh_schema().unwrap();
        state.switch_browse_table_in("aux", "t").unwrap();
        state.tab = MainTab::Browse;
        state.browse_mut().filter = "x%2=1".into();
        state.set_browse_ordering(vec![db::OrderBy { column: "x".into(), descending: true }]).unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "1203");
        state.sync_current_plot().unwrap();
        state.plot.check(1, 1, true);
        state.plot.check(2, 2, true);
        state.sync_current_plot().unwrap();
        state.start_plot_load().unwrap();
        assert!(state.conn.is_none());
        assert!(state.guard_background_idle().is_err());
        assert!(state.browse_page(true).is_err());
        settle(&mut state).unwrap();
        assert_eq!(state.plot.data.series[0].points.len(), 602);
        assert_eq!(state.plot.data.series[0].points[0].x, 1203.0);
        assert_eq!(state.plot.data.series[0].points[601].x, 1.0);
        let points = state.plot.data.series[0].points.clone();
        state.browse_page(true).unwrap();
        state.sync_current_plot().unwrap();
        assert_eq!(state.browse().offset, 500);
        assert!(Arc::ptr_eq(&points, &state.plot.data.series[0].points));
        state.goto_browse_record(3).unwrap();
        state.sync_current_plot().unwrap();
        assert!(Arc::ptr_eq(&points, &state.plot.data.series[0].points));
        state.plot.check(0, 1, true);
        state.sync_current_plot().unwrap();
        assert!(state.plot_work.is_some());
        settle(&mut state).unwrap();
        assert_eq!(state.plot.data.series[0].points[601].x, 602.0);
        state.conn.as_ref().unwrap().execute_batch("UPDATE aux.t SET y=7").unwrap();
        state.refresh_browse().unwrap();
        state.sync_current_plot().unwrap();
        assert!(!state.plot.has_full());
        assert_eq!(state.plot.data.series[0].points.len(), 500);
        assert_eq!(state.plot.data.series[0].points[0].y, 7.0);
    }

    #[test]
    fn external_writes_and_commit_boundaries_invalidate_full_browse_page_linkage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.sqlite");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(x,y); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1003) INSERT INTO t SELECT x,x FROM n").unwrap();
        let mut state = AppState::empty();
        state.open_readonly_path(path.to_str().unwrap()).unwrap();
        state.switch_browse_table_in("main", "t").unwrap();
        state.tab = MainTab::Browse;
        state.sync_current_plot().unwrap();
        state.plot.check(1, 1, true);
        state.plot.check(2, 2, true);
        state.sync_current_plot().unwrap();
        let old = state.plot.data.series[0].points.clone();
        writer.execute_batch("UPDATE t SET y=2").unwrap();
        state.start_plot_load().unwrap();
        assert!(settle(&mut state).unwrap_err().contains("Browse data changed"));
        assert!(Arc::ptr_eq(&old, &state.plot.data.series[0].points));
        assert!(!state.plot.has_full());
        state.refresh_browse().unwrap();
        state.sync_current_plot().unwrap();
        state.start_plot_load().unwrap();
        settle(&mut state).unwrap();
        assert!(state.plot.has_full());
        writer.execute_batch("UPDATE t SET y=9").unwrap();
        state.browse_page(true).unwrap();
        state.sync_current_plot().unwrap();
        assert!(!state.plot.has_full());
        assert_eq!(state.plot.data.series[0].points.len(), 500);
        assert_eq!(state.plot.data.series[0].points[0].y, 9.0);
        state.start_plot_load().unwrap();
        settle(&mut state).unwrap();
        assert!(state.plot.has_full());
        state.close_db();
        state.open_path(path.to_str().unwrap()).unwrap();
        state.switch_browse_table_in("main", "t").unwrap();
        state.tab = MainTab::Browse;
        state.sync_current_plot().unwrap();
        state.plot.check(1, 1, true);
        state.plot.check(2, 2, true);
        state.sync_current_plot().unwrap();
        state.start_plot_load().unwrap();
        settle(&mut state).unwrap();
        state.write_changes().unwrap();
        state.sync_current_plot().unwrap();
        assert!(!state.plot.has_full());
    }

    #[test]
    fn cancelled_browse_load_preserves_the_previous_plot_connection_and_pending_transaction() {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let mut first = true;
        state.conn.as_ref().unwrap().create_scalar_function("plot_wait", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, move |_| {
            if first {
                first = false;
                entered.send(()).unwrap();
                resume.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            Ok(1003)
        }).unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE pending(x); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1004) INSERT INTO pending SELECT x FROM n; CREATE VIEW gated AS SELECT x,CASE WHEN x=1003 THEN plot_wait() ELSE x END AS y FROM pending").unwrap();
        state.dirty = true;
        state.refresh_schema().unwrap();
        state.switch_browse_table_in("main", "gated").unwrap();
        state.tab = MainTab::Browse;
        state.sync_current_plot().unwrap();
        state.plot.check(1, 1, true);
        state.plot.check(2, 2, true);
        state.sync_current_plot().unwrap();
        let points = state.plot.data.series[0].points.clone();
        state.start_plot_load().unwrap();
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        state.plot_work.as_ref().unwrap().progress.cancel();
        release.send(()).unwrap();
        settle(&mut state).unwrap();
        assert!(state.dirty);
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert_eq!(db::count_rows(state.conn.as_ref().unwrap(), "pending").unwrap(), 1004);
        assert!(Arc::ptr_eq(&points, &state.plot.data.series[0].points));
        assert!(!state.plot.has_full());
        assert_eq!(state.status_msg, CANCELLED);
        state.start_plot_load().unwrap();
        settle(&mut state).unwrap();
        assert_eq!(state.plot.data.series[0].points.len(), 1004);
        assert!(state.dirty);
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        state.conn.as_ref().unwrap().execute_batch("ROLLBACK").unwrap();
        assert!(!db::list_schema(state.conn.as_ref().unwrap()).unwrap().iter().any(|o| o.name == "pending"));
    }

    #[test]
    fn full_sql_plot_reads_returning_once_and_reuses_the_snapshot_for_style_changes() {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE counter(x); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1003) INSERT INTO counter SELECT 0 FROM n").unwrap();
        state.tab = MainTab::ExecuteSql;
        state.sql_result = Some(db::execute_sql(state.conn.as_ref().unwrap(), "UPDATE counter SET x=x+1 RETURNING x").unwrap());
        state.sync_current_plot().unwrap();
        state.plot.check(0, 1, true);
        state.plot.check(1, 2, true);
        state.sync_current_plot().unwrap();
        state.start_plot_load().unwrap();
        settle(&mut state).unwrap();
        assert_eq!(state.plot.data.series[0].points.len(), 1003);
        assert_eq!(state.sql_result.as_ref().unwrap().rows.len(), 500);
        state.plot.set_styles(2, 4);
        state.sync_current_plot().unwrap();
        settle(&mut state).unwrap();
        assert_eq!(state.plot.data.series[0].points.len(), 1003);
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT sum(x) FROM counter", [], |r| r.get::<_, i64>(0)).unwrap(), 1003);
    }

    #[test]
    fn plot_load_controls_cancel_keep_full_rows_link_pages_and_restore_rejected_styles() {
        use next_gui_platform::PresentBackend;
        use next_gui_ui::{PlotLine, UiKey};
        let mut demo = crate::Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch(crate::FULL_PLOT_QA_SQL).unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("main", "telemetry").unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.body_ratio = 0.52;
        demo.state.dock_focus = crate::state::DockKind::Plot;
        demo.state.dock_plot = true;
        demo.state.sync_current_plot().unwrap();
        for (row, axis) in [(1, 1), (2, 2), (3, 3)] { demo.state.plot.check(row, axis, true); }
        demo.do_rebuild();
        demo.handle_button(demo.ids.plot.load_all);
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.plot_load.cancel));
        assert!(demo.state.plot_work.is_some());
        let points = demo.state.plot.data.series[0].points.clone();
        capture(&mut demo, "loading");
        demo.handle_button(demo.ids.plot_load.cancel);
        settle(&mut demo.state).unwrap();
        demo.do_rebuild();
        assert!(Arc::ptr_eq(&points, &demo.state.plot.data.series[0].points));
        assert!(!demo.state.plot.has_full());
        demo.handle_button(demo.ids.plot.load_all);
        settle(&mut demo.state).unwrap();
        demo.do_rebuild();
        assert_eq!(demo.state.plot.data.series[0].points.len(), 3000);
        assert_eq!(demo.ui.button_enabled(demo.ids.plot.load_all), Some(false));
        let points = demo.state.plot.data.series[0].points.clone();
        capture(&mut demo, "loaded");
        assert!(demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().select_row(2501, false));
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!((demo.state.browse().offset, demo.state.browse().sel_row), (2500, Some(1)));
        assert!(Arc::ptr_eq(&points, &demo.state.plot.data.series[0].points));
        demo.ui.set_select_selected(demo.ids.plot.line, Some(2));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.plot_load.cancel));
        assert!(demo.ui.key_input(UiKey::Escape));
        demo.handle_plot_load_dismiss();
        settle(&mut demo.state).unwrap();
        demo.do_rebuild();
        assert_eq!(demo.state.plot.data.series[0].line, PlotLine::Line);
        assert!(Arc::ptr_eq(&points, &demo.state.plot.data.series[0].points));
        capture(&mut demo, "cancelled");
    }

    fn capture(demo: &mut crate::Demo, stage: &str) {
        use next_gui_render::{encode_scene, Renderer};
        let Ok(prefix) = std::env::var("NEXT_GUI_PLOT_LOAD_QA_OUT") else { return; };
        for (name, theme) in [("dark", next_gui_ui::Theme::dark()), ("light", next_gui_ui::Theme::light())] {
            demo.theme = crate::ui_build::browser_theme(theme, demo.state.prefs.font_size);
            for (width, height) in [(1400, 1000), (800, 600)] {
                demo.win_w = width; demo.win_h = height;
                demo.do_rebuild();
                demo.scene.mark_full_dirty(width as f32, height as f32);
                let dirty = demo.scene.take_dirty();
                let commands = encode_scene(&demo.scene, &dirty);
                let clear = crate::ui_build::clear_color(&demo.theme);
                let mut soft = next_gui_render_soft::SoftRenderer::new(width, height, clear);
                soft.render(&commands, &dirty);
                let pixels = soft.frame_pixels().unwrap().0;
                crate::write_ppm(&format!("{prefix}-{stage}-{name}-{width}.ppm"), pixels, width, height).unwrap();
                if let Ok(mut gpu) = next_gui_render_gpu::GpuRenderer::try_new(width, height, clear) {
                    gpu.render(&commands, &dirty);
                    let rendered = gpu.frame_pixels().unwrap().0;
                    crate::write_ppm(&format!("{prefix}-{stage}-{name}-{width}-metal.ppm"), rendered, width, height).unwrap();
                    if stage != "loading" {
                        let bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.chart)).unwrap().bounds;
                        let mut delta = 0; let mut checked = 0;
                        for y in (bounds.y.ceil().max(0.0) as usize + 2)..((bounds.y + bounds.height).floor().min(height as f32) as usize).saturating_sub(2) {
                            for x in (bounds.x.ceil().max(0.0) as usize + 2)..((bounds.x + bounds.width).floor().min(width as f32) as usize).saturating_sub(2) {
                                for shift in [0, 8, 16] { delta = delta.max((((pixels[y * width as usize + x] >> shift) & 255) as i32 - ((rendered[y * width as usize + x] >> shift) & 255) as i32).abs()); }
                                checked += 1;
                            }
                        }
                        assert!(checked > 1000);
                        assert!(delta <= 2, "{stage}/{name}/{width}: delta {delta}");
                        eprintln!("Full plot {stage}/{name}/{width}: {checked} interior pixels, actual Soft/Metal max channel delta {delta}");
                    }
                } else { eprintln!("Full plot Metal capture not_run: adapter unavailable"); }
            }
        }
    }
}
