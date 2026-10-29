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

pub fn pick_save_sql(filename: Option<&str>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .add_filter("SQL", &["sql"])
        .set_title("Save SQL");
    if let Some(filename) = filename { dialog = dialog.set_file_name(filename); }
    dialog.save_file()
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

pub fn pick_open_cell() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title("Import Cell Data").pick_file()
}

pub fn pick_save_cell() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title("Export Cell Data").save_file()
}

pub fn pick_save_plot() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title("Save Plot").set_file_name("plot.png")
        .add_filter("PNG", &["png"]).add_filter("JPEG", &["jpg", "jpeg"])
        .add_filter("PDF", &["pdf"]).add_filter("BMP", &["bmp"])
        .add_filter("All files", &["*"]).save_file()
}

pub fn pick_save_readable_cell() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title("Export Cell as Hex + ASCII").add_filter("Text", &["txt"]).save_file()
}
