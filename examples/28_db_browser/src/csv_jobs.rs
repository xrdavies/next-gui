//! Cancellable CSV preparation and insertion on owned worker threads.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

use crate::{csv_import, db, sql_log::LoggedConnection};

#[derive(Default)]
pub struct Progress {
    stop: AtomicBool,
    bytes: AtomicU64,
    total_bytes: AtomicU64,
    rows: AtomicU64,
    total_rows: AtomicU64,
}

impl Progress {
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
    pub fn cancelled(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn bytes(&self) -> (u64, u64) {
        (
            self.bytes.load(Ordering::Relaxed),
            self.total_bytes.load(Ordering::Relaxed),
        )
    }
    pub fn rows(&self) -> (u64, u64) {
        (
            self.rows.load(Ordering::Relaxed),
            self.total_rows.load(Ordering::Relaxed),
        )
    }
}

struct CountingRead<R> {
    reader: R,
    progress: Arc<Progress>,
}

impl<R: std::io::Read> std::io::Read for CountingRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.progress.cancelled() {
            return Err(std::io::Error::other("CSV import cancelled"));
        }
        let count = self.reader.read(buffer)?;
        if self.progress.cancelled() {
            return Err(std::io::Error::other("CSV import cancelled"));
        }
        self.progress
            .bytes
            .fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
}

pub enum Completed {
    Read(Result<csv_import::Prepared, String>),
    Write(Option<LoggedConnection>, Result<u64, String>),
}

pub enum Job {
    Read(JoinHandle<Result<csv_import::Prepared, String>>),
    Write(JoinHandle<(Option<LoggedConnection>, Result<u64, String>)>),
    Ready(Completed),
}

impl Job {
    pub fn file(path: PathBuf, options: db::CsvImportOptions, progress: Arc<Progress>) -> Self {
        match std::thread::Builder::new()
            .name("csv-prepare".into())
            .spawn(move || {
                let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
                progress.total_bytes.store(
                    file.metadata().map_err(|error| error.to_string())?.len(),
                    Ordering::Relaxed,
                );
                csv_import::prepare(
                    CountingRead {
                        reader: file,
                        progress,
                    },
                    &options,
                )
            }) {
            Ok(handle) => Self::Read(handle),
            Err(error) => Self::Ready(Completed::Read(Err(error.to_string()))),
        }
    }

    pub fn text(text: String, mut options: db::CsvImportOptions, progress: Arc<Progress>) -> Self {
        match std::thread::Builder::new()
            .name("csv-prepare".into())
            .spawn(move || {
                let bytes = text.trim_start_matches('\u{feff}').as_bytes();
                progress
                    .total_bytes
                    .store(bytes.len() as u64, Ordering::Relaxed);
                options.encoding = "UTF-8".into();
                csv_import::prepare(
                    CountingRead {
                        reader: bytes,
                        progress,
                    },
                    &options,
                )
            }) {
            Ok(handle) => Self::Read(handle),
            Err(error) => Self::Ready(Completed::Read(Err(error.to_string()))),
        }
    }

    pub fn insert(
        conn: LoggedConnection,
        table: String,
        prepared: csv_import::Prepared,
        options: db::CsvImportOptions,
        progress: Arc<Progress>,
    ) -> Self {
        progress
            .total_rows
            .store(prepared.row_count, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel::<LoggedConnection>();
        // Transfer the live connection only after spawning succeeds, preserving it on spawn failure.
        let worker = std::thread::Builder::new()
            .name("csv-insert".into())
            .spawn(move || {
                let Ok(conn) = receiver.recv() else {
                    return (None, Err("CSV connection handoff failed".into()));
                };
                let result = prepared.rows().and_then(|mut input| {
                    let rows = std::iter::from_fn(|| {
                        // SQLite interrupt can roll back the outer transaction; stop only between rows.
                        if progress.cancelled() {
                            return Some(Err("CSV import cancelled".into()));
                        }
                        let row = input.next();
                        if matches!(&row, Some(Ok(_))) {
                            progress.rows.fetch_add(1, Ordering::Relaxed);
                        }
                        row
                    });
                    db::import_csv_rows(
                        &conn,
                        &table,
                        &prepared.preview.columns,
                        &prepared.preview.types,
                        rows,
                        &options,
                    )
                });
                (Some(conn), result)
            });
        match worker {
            Ok(handle) => match sender.send(conn) {
                Ok(()) => Self::Write(handle),
                Err(error) => {
                    let _ = handle.join();
                    Self::Ready(Completed::Write(
                        Some(error.0),
                        Err("CSV connection handoff failed".into()),
                    ))
                }
            },
            Err(error) => Self::Ready(Completed::Write(Some(conn), Err(error.to_string()))),
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            Self::Read(handle) => handle.is_finished(),
            Self::Write(handle) => handle.is_finished(),
            Self::Ready(_) => true,
        }
    }

    pub fn is_reading(&self) -> bool {
        matches!(self, Self::Read(_) | Self::Ready(Completed::Read(_)))
    }

    pub fn join(self) -> Completed {
        match self {
            Self::Read(handle) => Completed::Read(
                handle
                    .join()
                    .unwrap_or_else(|_| Err("CSV preparation worker panicked".into())),
            ),
            Self::Write(handle) => {
                let (conn,result)=handle.join().unwrap_or_else(|_|(None,Err("CSV insertion worker panicked; connection closed and pending changes rolled back".into())));
                Completed::Write(conn, result)
            }
            Self::Ready(result) => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn csv_worker_cancel_during_a_row_keeps_the_outer_transaction_and_returns_connection() {
        let conn = LoggedConnection::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap();
        let (entered, waiting) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        conn.create_scalar_function(
            "wait_csv",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                entered
                    .send(())
                    .map_err(|error| rusqlite::Error::UserFunctionError(Box::new(error)))?;
                resume
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .map_err(|error| rusqlite::Error::UserFunctionError(Box::new(error)))?;
                Ok(0)
            },
        )
        .unwrap();
        conn.execute_batch("CREATE TABLE keeper(v);INSERT INTO keeper VALUES('saved');CREATE TABLE target(v TEXT);INSERT INTO target VALUES('saved');CREATE TRIGGER wait_import AFTER INSERT ON target WHEN new.v='first' BEGIN SELECT wait_csv();END;BEGIN;INSERT INTO keeper VALUES('pending')").unwrap();
        let options = db::CsvImportOptions::default();
        let prepared = csv_import::prepare_text("v\nfirst\nsecond\n", &options).unwrap();
        let progress = Arc::new(Progress::default());
        let job = Job::insert(conn, "target".into(), prepared, options, progress.clone());
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(!job.is_finished());
        progress.cancel();
        release.send(()).unwrap();
        let Completed::Write(Some(conn), result) = job.join() else {
            panic!("CSV worker lost its connection")
        };
        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(progress.rows(), (1, 2));
        assert!(!conn.is_autocommit());
        assert_eq!(db::count_rows(&conn, "keeper").unwrap(), 2);
        assert_eq!(db::count_rows(&conn, "target").unwrap(), 1);
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(db::count_rows(&conn, "keeper").unwrap(), 1);
    }

    #[test]
    fn csv_workers_prepare_native_paths_and_unicode_clipboard_and_report_progress() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("日本.csv");
        let content = "v\n日本\n";
        std::fs::write(&path, content).unwrap();
        for clipboard in [false, true] {
            let progress = Arc::new(Progress::default());
            let options = db::CsvImportOptions::default();
            let job = if clipboard {
                Job::text(
                    content.into(),
                    db::CsvImportOptions {
                        encoding: "bad-codec".into(),
                        ..options
                    },
                    progress.clone(),
                )
            } else {
                Job::file(path.clone(), options, progress.clone())
            };
            let Completed::Read(Ok(prepared)) = job.join() else {
                panic!("CSV preparation failed")
            };
            assert_eq!(prepared.rows().unwrap().next().unwrap().unwrap(), ["日本"]);
            assert_eq!(
                progress.bytes(),
                (content.len() as u64, content.len() as u64)
            );
        }
    }

    #[test]
    fn csv_preparation_cancel_checks_both_sides_of_blocking_reads() {
        struct Reader {
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
        }
        impl Read for Reader {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                out[0] = b'v';
                Ok(1)
            }
        }
        let (entered, waiting) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let progress = Arc::new(Progress::default());
        let control = progress.clone();
        let handle = std::thread::spawn(move || {
            csv_import::prepare(
                CountingRead {
                    reader: Reader {
                        entered,
                        release: resume,
                    },
                    progress: control,
                },
                &db::CsvImportOptions::default(),
            )
        });
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        progress.cancel();
        release.send(()).unwrap();
        assert!(handle.join().unwrap().unwrap_err().contains("cancelled"));
        assert_eq!(progress.bytes(), (0, 0));
    }
}
