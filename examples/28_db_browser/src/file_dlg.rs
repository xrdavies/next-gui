//! Native file dialogs via `rfd`; cancellation never selects a fallback path.

use std::path::PathBuf;

pub fn pick_open_db() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("SQLite database", &["db", "sqlite", "sqlite3", "DB"])
        .add_filter("All files", &["*"])
        .set_title("Open Database")
        .pick_file()
}

pub fn pick_save_db() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("SQLite database", &["db", "sqlite", "sqlite3"])
        .set_title("Save Database As")
        .save_file()
}

pub fn pick_open_csv() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .add_filter("CSV", &["csv", "txt"])
        .set_title("Import CSV")
        .pick_files()
}


pub fn pick_open_sql() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("SQL", &["sql", "txt"])
        .set_title("Open SQL")
        .pick_file()
}

pub fn pick_save_sql() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("SQL", &["sql"])
        .set_title("Save SQL")
        .save_file()
}

pub fn pick_save_log() -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter("SQL log", &["log", "txt"]).set_title("Export SQL Log").save_file()
}


pub fn pick_open_project() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("DB Browser project", &["sqbpro"])
        .add_filter("Legacy e28 JSON project", &["json"])
        .set_title("Open Project")
        .pick_file()
}

pub fn pick_save_project() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("DB Browser XML project", &["sqbpro"])
        .set_title("Save Project")
        .save_file()
}

/// Export the original image bytes without decoding/re-encoding them.
pub fn pick_save_image() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title("Export Original Image").save_file()
}
