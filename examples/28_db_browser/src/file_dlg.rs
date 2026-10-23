//! Native file dialogs via `rfd`, with path fallbacks for headless / cancel.

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

pub fn pick_open_csv() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("CSV", &["csv", "txt"])
        .set_title("Import CSV")
        .pick_file()
}

pub fn pick_save_csv() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_title("Export CSV")
        .save_file()
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

pub fn pick_save_json() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_title("Export JSON")
        .save_file()
}
