//! Cancellable staging, followed by non-cancellable atomic file publication.
use crate::{data_export, sql_log::LoggedConnection};
use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    mpsc, Arc,
};

pub const CANCELLED: &str = "Data export cancelled";

#[derive(Default)]
pub struct Progress {
    phase: AtomicU8,
    rows: AtomicU64,
    files: AtomicU64,
}
impl Progress {
    pub fn cancel(&self) {
        let _ = self
            .phase
            .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed);
    }
    pub fn cancelled(&self) -> bool {
        self.phase.load(Ordering::Relaxed) == 1
    }
    pub fn can_cancel(&self) -> bool {
        self.phase.load(Ordering::Relaxed) == 0
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled() {
            Err(CANCELLED.into())
        } else {
            Ok(())
        }
    }
    pub fn begin_publish(&self) -> Result<(), String> {
        self.phase
            .compare_exchange(0, 2, Ordering::Relaxed, Ordering::Relaxed)
            .map(|_| ())
            .map_err(|_| CANCELLED.to_string())
    }
    pub fn row(&self) {
        self.rows.fetch_add(1, Ordering::Relaxed);
    }
    pub fn staged(&self) {
        self.files.fetch_add(1, Ordering::Relaxed);
    }
    pub fn counts(&self) -> (u64, u64) {
        (
            self.rows.load(Ordering::Relaxed),
            self.files.load(Ordering::Relaxed),
        )
    }
}

type Completed = (Option<LoggedConnection>, Result<(), String>);
pub enum Job {
    Running(std::thread::JoinHandle<Completed>),
    Ready(Completed),
}
impl Job {
    pub fn start(
        conn: LoggedConnection,
        outputs: Vec<data_export::Output>,
        query: Option<data_export::QuerySource>,
        json: bool,
        options: data_export::Options,
        replace: bool,
        progress: Arc<Progress>,
    ) -> Self {
        Self::start_task(conn, move |conn| {
            if let Some(source) = query {
                outputs
                    .first()
                    .ok_or("Export needs a destination".into())
                    .and_then(|output| {
                        data_export::export_query_progress(
                            conn,
                            &source,
                            &output.path,
                            json,
                            &options,
                            replace,
                            &progress,
                        )
                    })
            } else {
                data_export::export_progress(conn, &outputs, json, &options, replace, &progress)
            }
        })
    }
    pub fn start_sql(
        conn: LoggedConnection,
        path: std::path::PathBuf,
        options: crate::db::ExportSqlOptions,
        progress: Arc<Progress>,
    ) -> Self {
        Self::start_task(conn, move |conn| {
            crate::db::export_sql_dump_progress(conn, &path, &options, &progress)
        })
    }
    fn start_task(
        conn: LoggedConnection,
        task: impl FnOnce(&LoggedConnection) -> Result<(), String> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<LoggedConnection>();
        let worker = std::thread::Builder::new()
            .name("data-export".into())
            .spawn(move || {
                let conn = match receiver.recv() {
                    Ok(conn) => conn,
                    Err(error) => return (None, Err(error.to_string())),
                };
                let result = task(&conn);
                (Some(conn), result)
            });
        match worker {
            Ok(handle) => match sender.send(conn) {
                Ok(()) => Self::Running(handle),
                Err(error) => {
                    let _ = handle.join();
                    Self::Ready((
                        Some(error.0),
                        Err("Export connection handoff failed".into()),
                    ))
                }
            },
            Err(error) => Self::Ready((Some(conn), Err(error.to_string()))),
        }
    }
    pub fn is_finished(&self) -> bool {
        match self {
            Self::Running(handle) => handle.is_finished(),
            Self::Ready(_) => true,
        }
    }
    pub fn join(self) -> Completed {
        match self {
            Self::Running(handle) => handle.join().unwrap_or_else(|_| {
                (
                    None,
                    Err(
                        "Export worker panicked; connection closed and pending changes rolled back"
                            .into(),
                    ),
                )
            }),
            Self::Ready(result) => result,
        }
    }
}

pub enum Options {
    Data(data_export::Options),
    Sql(crate::db::ExportSqlOptions),
}

pub struct Work {
    pub job: Job,
    pub progress: Arc<Progress>,
    pub options: Options,
    pub count: usize,
}
impl Work {
    pub fn info(&self) -> (f32, String) {
        let (rows, files) = self.progress.counts();
        let caption = if self.progress.cancelled() {
            "Canceling export…"
        } else if !self.progress.can_cancel() {
            "Finishing export…"
        } else {
            match self.options {
                Options::Data(_) => "Exporting data…",
                Options::Sql(_) => "Exporting SQL…",
            }
        };
        (
            files as f32 / self.count.max(1) as f32,
            format!(
                "{caption}\n{files} of {} files prepared · {rows} rows\n{}",
                self.count,
                if self.progress.cancelled() {
                    "Waiting for the current row or write to finish."
                } else if self.progress.can_cancel() {
                    "Cancel keeps existing destination files."
                } else {
                    "Writing completed files to their destinations."
                }
            ),
        )
    }
}

pub fn build_progress(
    ui: &mut next_gui_ui::Ui,
    work: &Work,
    theme: &next_gui_ui::Theme,
    width: f32,
    height: f32,
) -> (
    next_gui_ui::WidgetId,
    next_gui_ui::WidgetId,
    next_gui_ui::WidgetId,
    next_gui_ui::WidgetId,
) {
    use next_gui_ui::{ModalStyle, TextAreaStyle};
    let mut progress_style = ModalStyle::from_theme(theme);
    progress_style.width = (width - 80.0).min(620.0);
    progress_style.height = (height - 48.0).min(230.0);
    progress_style.close_on_backdrop = false;
    let mut message = TextAreaStyle::from_theme(theme);
    message.width = progress_style.width - 48.0;
    message.height = 100.0;
    let mut bar = next_gui_ui::ProgressStyle::from_theme(theme);
    bar.width = message.width;
    let (value, caption) = work.info();
    let mut text = next_gui_ui::WidgetId(0);
    let mut bar_id = next_gui_ui::WidgetId(0);
    let mut stop = next_gui_ui::WidgetId(0);
    let dialog = ui.dialog(
        if matches!(work.options, Options::Sql(_)) {
            "Exporting SQL"
        } else {
            "Exporting data"
        },
        progress_style,
        |body| {
            text = body.text_area(caption, "", message);
            bar_id = body.progress_styled(value, true, bar);
        },
        |footer| {
            stop = footer.tool_button("Cancel");
        },
    );
    ui.set_text_area_read_only(text, true);
    ui.set_accessible_name(text, "Data export progress");
    ui.set_button_enabled(stop, work.progress.can_cancel());
    ui.set_dialog_open(dialog, true);
    ui.set_focus(Some(stop));
    (dialog, text, bar_id, stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn cancel_during_later_table_preserves_every_destination_and_pending_changes() {
        let conn = LoggedConnection::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap();
        let (entered, waiting) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        conn.create_scalar_function(
            "wait_export",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                entered.send(()).unwrap();
                resume.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(8)
            },
        )
        .unwrap();
        conn.execute_batch("CREATE TABLE first(v);INSERT INTO first VALUES('saved');CREATE VIEW later AS SELECT wait_export() AS value;BEGIN;INSERT INTO first VALUES('pending')").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.csv");
        let later = directory.path().join("later.csv");
        std::fs::write(&first, "keep first").unwrap();
        std::fs::write(&later, "keep later").unwrap();
        let outputs = vec![
            data_export::Output {
                schema: "main".into(),
                table: "first".into(),
                path: first.clone(),
            },
            data_export::Output {
                schema: "main".into(),
                table: "later".into(),
                path: later.clone(),
            },
        ];
        let progress = Arc::new(Progress::default());
        let job = Job::start(
            conn,
            outputs,
            None,
            false,
            data_export::Options::default(),
            true,
            progress.clone(),
        );
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!job.is_finished());
        assert_eq!(progress.counts(), (2, 1));
        progress.cancel();
        release.send(()).unwrap();
        let (Some(conn), result) = job.join() else {
            panic!("Connection lost");
        };
        assert_eq!(result.unwrap_err(), CANCELLED);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "keep first");
        assert_eq!(std::fs::read_to_string(&later).unwrap(), "keep later");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
        assert!(!conn.is_autocommit());
        assert_eq!(crate::db::count_rows(&conn, "first").unwrap(), 2);
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(crate::db::count_rows(&conn, "first").unwrap(), 1);
    }

    #[test]
    fn worker_errors_survive_cancel_and_publication_has_a_final_cancellation_boundary() {
        let conn = LoggedConnection::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap();
        let (entered, waiting) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        conn.create_scalar_function(
            "fail_export",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| -> rusqlite::Result<i64> {
                entered.send(()).unwrap();
                resume.recv_timeout(Duration::from_secs(5)).unwrap();
                Err(rusqlite::Error::UserFunctionError(Box::new(
                    std::io::Error::other("actual export failure"),
                )))
            },
        )
        .unwrap();
        conn.execute_batch("CREATE VIEW broken AS SELECT fail_export() AS v;BEGIN")
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("keep.csv");
        std::fs::write(&path, "previous").unwrap();
        let progress = Arc::new(Progress::default());
        let job = Job::start(
            conn,
            vec![data_export::Output {
                schema: "main".into(),
                table: "broken".into(),
                path: path.clone(),
            }],
            None,
            false,
            data_export::Options::default(),
            true,
            progress.clone(),
        );
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        progress.cancel();
        release.send(()).unwrap();
        let (Some(conn), result) = job.join() else {
            panic!("Connection lost");
        };
        assert!(result.unwrap_err().contains("actual export failure"));
        assert!(!conn.is_autocommit());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
        let progress = Progress::default();
        progress.cancel();
        assert_eq!(progress.begin_publish().unwrap_err(), CANCELLED);
        let progress = Progress::default();
        progress.begin_publish().unwrap();
        progress.cancel();
        assert!(!progress.can_cancel() && !progress.cancelled());
    }
}
