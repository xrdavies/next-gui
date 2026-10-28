//! File selection and per-file targets for the original batch import workflow.

use std::path::{Path, PathBuf};

use crate::{csv_import, db::CsvImportOptions};

#[derive(Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub target: String,
    pub selected: bool,
    pub done: bool,
    pub matched: bool,
}

#[derive(Debug)]
pub struct Batch {
    pub files: Vec<Entry>,
    pub current: usize,
    pub separate: bool,
    pub shared_target: String,
    pub scroll: (f32, f32),
    pub widths: Vec<f32>,
    focus: Option<usize>,
}

impl Batch {
    pub fn new(paths: Vec<PathBuf>, separate: bool) -> Option<Self> {
        let files: Vec<_> = paths
            .into_iter()
            .map(|path| {
                let target = path
                    .file_stem()
                    .map(|name| name.to_string_lossy().into_owned())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "imported".into());
                Entry {
                    path,
                    target,
                    selected: true,
                    done: false,
                    matched: false,
                }
            })
            .collect();
        let shared_target = files.first()?.target.clone();
        Some(Self {
            files,
            current: 0,
            separate,
            shared_target,
            scroll: (0.0, 0.0),
            widths: Vec::new(),
            focus: None,
        })
    }

    pub fn target(&self, index: usize) -> &str {
        if self.separate {
            &self.files[index].target
        } else {
            &self.shared_target
        }
    }

    pub fn save_target(&mut self, value: &str) {
        if self.separate {
            self.files[self.current].target = value.into();
        } else {
            self.shared_target = value.into();
        }
    }

    pub fn select(&mut self, index: usize, current_name: &str) -> bool {
        if index >= self.files.len() || self.files[index].done || index == self.current {
            return false;
        }
        self.save_target(current_name);
        self.current = index;
        true
    }

    pub fn set_separate(&mut self, separate: bool, current_name: &str) {
        self.save_target(current_name);
        self.separate = separate;
    }

    pub fn visible(&self) -> Vec<usize> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, file)| !file.done)
            .map(|(index, _)| index)
            .collect()
    }

    pub fn queue(&self) -> Vec<usize> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, file)| file.selected && !file.done)
            .map(|(index, _)| index)
            .collect()
    }

    pub fn all_selected(&self) -> bool {
        self.files.iter().any(|file| !file.done)
            && self
                .files
                .iter()
                .filter(|file| !file.done)
                .all(|file| file.selected)
    }

    pub fn select_all(&mut self, selected: bool) {
        for file in &mut self.files {
            if !file.done {
                file.selected = selected;
            }
        }
    }

    pub fn match_similar(&mut self, options: &CsvImportOptions) -> Result<Vec<String>, String> {
        let headers = |path: &Path| {
            std::fs::File::open(path)
                .map_err(|error| error.to_string())
                .and_then(|file| csv_import::read(file, options, 20))
                .map(|data| data.columns)
        };
        let expected = headers(&self.files[self.current].path)?;
        let mut errors = Vec::new();
        for file in self.files.iter_mut().filter(|file| !file.done) {
            match headers(&file.path) {
                Ok(columns) => {
                    file.matched = columns == expected;
                    file.selected = file.matched;
                }
                Err(error) => {
                    file.matched = false;
                    file.selected = false;
                    errors.push(format!("{}: {error}", file.path.display()));
                }
            }
        }
        Ok(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_selection_preserves_individual_targets_shared_target_and_pending_files() {
        assert!(Batch::new(Vec::new(), false).is_none());
        let mut batch = Batch::new(
            vec![
                PathBuf::from("one.csv"),
                PathBuf::from("dir/two.csv"),
                PathBuf::from("other/one.csv"),
            ],
            false,
        )
        .unwrap();
        assert_eq!(batch.target(2), "one");
        assert_eq!(batch.queue(), [0, 1, 2]);
        assert!(batch.all_selected());
        assert!(batch.select(1, "merged"));
        assert_eq!(batch.target(1), "merged");
        batch.set_separate(true, "shared");
        assert_eq!(batch.target(1), "two");
        assert!(batch.select(2, "renamed two"));
        assert_eq!(batch.target(1), "renamed two");
        assert_eq!(batch.target(2), "one");
        batch.set_separate(false, "renamed one");
        assert_eq!(batch.target(0), "shared");
        batch.set_separate(true, "combined");
        assert_eq!(batch.target(2), "renamed one");
        batch.files[0].done = true;
        assert!(!batch.select(0, "unchanged"));
        assert_eq!(batch.visible(), [1, 2]);
        batch.select_all(false);
        assert!(batch.queue().is_empty());
        assert!(!batch.all_selected());
        batch.files[1].selected = true;
        assert_eq!(batch.queue(), [1]);
        batch.select_all(true);
        assert_eq!(batch.queue(), [1, 2]);
    }

    #[test]
    fn header_matching_checks_names_not_affinities_and_reports_unreadable_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<_> = [
            "first.csv",
            "same.csv",
            "different.csv",
            "wide.csv",
            "missing.csv",
        ]
        .iter()
        .map(|name| dir.path().join(name))
        .collect();
        for (path, text) in paths.iter().zip([
            "id,name\n1,one\n",
            "id,name\ntext,word\n",
            "id,other\n2,two\n",
            "id,name,tail\n1,a,b\n",
        ]) {
            std::fs::write(path, text).unwrap();
        }
        let mut batch = Batch::new(paths, false).unwrap();
        let options = CsvImportOptions {
            has_header: true,
            ..CsvImportOptions::default()
        };
        let errors = batch.match_similar(&options).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("missing.csv"));
        assert_eq!(batch.queue(), [0, 1]);
        assert!(batch.files[0].matched && batch.files[1].matched);
        batch.files[1].done = true;
        batch.select_all(true);
        batch.match_similar(&options).unwrap();
        assert_eq!(batch.queue(), [0]);
        let before: Vec<_> = batch.files.iter().map(|file| file.selected).collect();
        assert!(batch
            .match_similar(&CsvImportOptions {
                encoding: "unknown-codec".into(),
                ..options
            })
            .is_err());
        assert_eq!(
            batch
                .files
                .iter()
                .map(|file| file.selected)
                .collect::<Vec<_>>(),
            before
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Answer {
    Yes,
    No,
    YesAll,
    Cancel,
}

#[derive(Debug)]
enum Source {
    File(PathBuf),
    Clipboard,
}

#[derive(Debug)]
struct Item {
    source: Source,
    target: String,
    slot: Option<usize>,
}

#[derive(Debug)]
pub struct Work {
    items: Vec<Item>,
    index: usize,
    pub options: CsvImportOptions,
    pub pending: Option<csv_import::Data>,
    accepted: std::collections::HashSet<String>,
    yes_all: bool,
    rows: u64,
    imported: usize,
    skipped: usize,
    last_table: Option<String>,
    overlay: crate::state::OverlayKind,
    pub focus: usize,
}

impl Work {
    pub fn target(&self) -> &str {
        &self.items[self.index].target
    }
    pub fn label(&self) -> String {
        match &self.items[self.index].source {
            Source::File(path) => path.display().to_string(),
            Source::Clipboard => "Clipboard".into(),
        }
    }
    fn read(&self, clipboard: &str) -> Result<csv_import::Data, String> {
        // shortcut: full imports retain parsed rows; stream under a savepoint when adding cancellable progress.
        match &self.items[self.index].source {
            Source::File(path) => std::fs::File::open(path)
                .map_err(|error| error.to_string())
                .and_then(|file| csv_import::read(file, &self.options, 0)),
            Source::Clipboard => csv_import::text(clipboard, &self.options, 0),
        }
    }
}

impl crate::state::AppState {
    pub fn start_csv_import_work(&mut self) -> Result<(), String> {
        use crate::state::OverlayKind;
        self.guard_writable()?;
        if self.csv_work.is_some() {
            return Err("Finish the pending CSV import first".into());
        }
        let overlay = self.overlay.ok_or("Open the CSV import dialog first")?;
        let items = if overlay == OverlayKind::PasteCsv {
            vec![Item {
                source: Source::Clipboard,
                target: self.csv_table.trim().into(),
                slot: None,
            }]
        } else if let Some(batch) = &mut self.csv_files {
            batch.save_target(&self.csv_table);
            batch
                .queue()
                .into_iter()
                .map(|index| Item {
                    source: Source::File(batch.files[index].path.clone()),
                    target: batch.target(index).trim().into(),
                    slot: Some(index),
                })
                .collect()
        } else {
            vec![Item {
                source: Source::File(self.selected_csv_path().to_path_buf()),
                target: self.csv_table.trim().into(),
                slot: None,
            }]
        };
        if items.is_empty() {
            return Err("Select at least one CSV file".into());
        }
        if items.iter().any(|item| item.target.is_empty()) {
            return Err("Each selected CSV file requires a table name".into());
        }
        self.csv_work = Some(Work {
            items,
            index: 0,
            options: self.csv_options(),
            pending: None,
            accepted: std::collections::HashSet::new(),
            yes_all: false,
            rows: 0,
            imported: 0,
            skipped: 0,
            last_table: None,
            overlay,
            focus: 1,
        });
        self.continue_csv_import_work(None)
    }

    pub fn continue_csv_import_work(&mut self, answer: Option<Answer>) -> Result<(), String> {
        let Some(mut work) = self.csv_work.take() else {
            return Ok(());
        };
        if !self.read_only && self.conn.as_ref().is_some_and(|conn| conn.is_autocommit()) {
            self.finish_csv_import_work(&work, true);
            return Err("SQLite ended the pending transaction; review database changes and reselect files before retrying".into());
        }
        if let Err(error) = self.guard_writable() {
            self.finish_csv_import_work(&work, true);
            return Err(error);
        }
        let cancelled = matches!(answer, Some(Answer::Cancel));
        let result = (|| {
            if cancelled {
                return Ok(false);
            }
            if let Some(answer) = answer {
                if work.pending.is_none() {
                    return Err("There is no CSV import awaiting confirmation".into());
                }
                match answer {
                    Answer::No => {
                        self.finish_csv_item(&mut work, false);
                        work.pending = None;
                    }
                    Answer::Yes | Answer::YesAll => {
                        if matches!(answer, Answer::YesAll) {
                            work.yes_all = true;
                        }
                        work.accepted.insert(work.target().to_ascii_lowercase());
                        let data = work.pending.take().unwrap();
                        self.insert_csv_item(&mut work, &data)?;
                    }
                    Answer::Cancel => unreachable!(),
                }
            }
            while work.index < work.items.len() {
                let item = &work.items[work.index];
                if let Some(index) = item.slot {
                    if let Some(batch) = &mut self.csv_files {
                        batch.current = index;
                    }
                }
                if let Source::File(path) = &item.source {
                    self.csv_path = path.display().to_string();
                }
                self.csv_table = item.target.clone();
                self.csv_preview = None;
                let data = work
                    .read(&self.csv_paste)
                    .map_err(|error| format!("{}: {error}", work.label()))?;
                let exists:bool=self.conn.as_ref().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM main.sqlite_schema WHERE type='table' AND name=?1 COLLATE NOCASE)",[work.target()],|row|row.get(0)).map_err(|error|error.to_string())?;
                if exists
                    && !work.yes_all
                    && !work.accepted.contains(&work.target().to_ascii_lowercase())
                {
                    work.pending = Some(data);
                    work.focus = 1;
                    return Ok(true);
                }
                self.insert_csv_item(&mut work, &data)?;
            }
            Ok(false)
        })();
        match result {
            Ok(true) => {
                self.csv_work = Some(work);
                Ok(())
            }
            Ok(false) => {
                self.finish_csv_import_work(&work, cancelled);
                Ok(())
            }
            Err(error) => {
                let rolled_back = self.finish_csv_import_work(&work, true);
                Err(if rolled_back {
                    format!("{error}; SQLite rolled back the pending transaction. Review database changes and reselect files before retrying")
                } else {
                    error
                })
            }
        }
    }

    fn finish_csv_item(&mut self, work: &mut Work, imported: bool) {
        if let Some(index) = work.items[work.index].slot {
            if let Some(batch) = &mut self.csv_files {
                batch.files[index].done = true;
            }
        }
        if imported {
            work.imported += 1;
        } else {
            work.skipped += 1;
        }
        work.index += 1;
    }

    fn insert_csv_item(&mut self, work: &mut Work, data: &csv_import::Data) -> Result<(), String> {
        let table = work.target().to_string();
        let rows =
            crate::db::import_csv_data(self.conn.as_ref().unwrap(), &table, data, &work.options)
                .map_err(|error| format!("{}: {error}", work.label()))?;
        self.mark_dirty();
        self.log_sql(format!("-- import csv → {table} ({rows} rows)"));
        work.rows += rows;
        work.last_table = Some(table);
        self.finish_csv_item(work, true);
        Ok(())
    }

    fn finish_csv_import_work(&mut self, work: &Work, cancelled: bool) -> bool {
        use crate::state::{BrowseTabState, OverlayKind};
        if !self.read_only && self.conn.as_ref().is_some_and(|conn| conn.is_autocommit()) {
            let recovery = self.recover_ended_transaction().err();
            // A trigger's explicit ROLLBACK invalidates earlier progress; never reimport files automatically.
            if let Some(batch) = &mut self.csv_files {
                for file in &mut batch.files {
                    file.done = false;
                    file.selected = false;
                    file.matched = false;
                }
            }
            self.csv_preview = None;
            self.overlay = Some(work.overlay);
            self.status_msg="SQLite rolled back pending database changes; review the database and reselect files before retrying".into();
            if let Some(error) = recovery {
                self.status_msg.push_str(&format!("; {error}"));
            }
            return true;
        }
        let remaining = self.csv_files.as_ref().and_then(|batch| {
            work.items
                .get(work.index)
                .and_then(|item| item.slot)
                .filter(|index| !batch.files[*index].done)
                .or_else(|| batch.files.iter().position(|file| !file.done))
        });
        if let Some(index) = remaining {
            let batch = self.csv_files.as_mut().unwrap();
            batch.current = index;
            self.csv_path = batch.files[index].path.display().to_string();
            self.csv_table = batch.target(index).into();
        }
        self.csv_preview = None;
        self.overlay = if cancelled || remaining.is_some() {
            Some(work.overlay)
        } else {
            None
        };
        self.status_msg = format!(
            "Imported {} rows from {} file(s); skipped {} (unwritten)",
            work.rows, work.imported, work.skipped
        );
        if work.imported > 0 {
            self.prefs.csv_delimiter = work.options.delimiter;
            self.prefs.csv_import = Some(work.options.clone());
            if let Some(batch) = &self.csv_files {
                self.prefs.csv_separate_tables = batch.separate;
            }
            if !cfg!(test) && std::env::var_os("NEXT_GUI_DB_BROWSER_QA_TAB").is_none() {
                if let Err(error) = self.prefs.save() {
                    self.status_msg
                        .push_str(&format!("; settings could not be saved: {error}"));
                }
            }
            if let Some(table) = &work.last_table {
                *self.browse_mut() = BrowseTabState::new(Some(table.clone()));
            }
            // Successful files are marked done before refreshing, so a refresh error cannot import them twice.
            if let Err(error) = self.refresh_schema().and_then(|_| self.refresh_browse()) {
                self.status_msg
                    .push_str(&format!("; refresh failed: {error}"));
            }
        }
        if self.overlay.is_none() || work.overlay == OverlayKind::PasteCsv {
            self.csv_files = None;
        }
        false
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub table: next_gui_ui::WidgetId,
    pub separate: next_gui_ui::WidgetId,
    pub toggle: next_gui_ui::WidgetId,
    pub matching: next_gui_ui::WidgetId,
    pub dialog: next_gui_ui::WidgetId,
    pub answers: [next_gui_ui::WidgetId; 4],
    message: next_gui_ui::WidgetId,
}

impl Default for Controls {
    fn default() -> Self {
        let zero = next_gui_ui::WidgetId(0);
        Self {
            table: zero,
            separate: zero,
            toggle: zero,
            matching: zero,
            dialog: zero,
            answers: [zero; 4],
            message: zero,
        }
    }
}

pub fn build_files(
    builder: &mut next_gui_ui::FlexBuilder<'_>,
    state: &crate::state::AppState,
    theme: &next_gui_ui::Theme,
    width: f32,
) -> Controls {
    use next_gui_ui::{RowStyle, SwitchStyle, TableColumn, TableStyle};
    let mut controls = Controls::default();
    let Some(batch) = state
        .csv_files
        .as_ref()
        .filter(|batch| batch.files.len() > 1)
    else {
        return controls;
    };
    builder.text(format!("CSV files · {} remaining", batch.visible().len()));
    let mut grid = TableStyle::from_theme(theme);
    grid.width = width;
    grid.height = 144.0;
    controls.table = builder.virtual_table(
        vec![
            TableColumn::new("Use", 40.0),
            TableColumn::new("File", (width - 40.0 - grid.padding * 2.0 - 1.0).max(100.0)),
        ],
        batch
            .visible()
            .iter()
            .map(|index| {
                let path = &batch.files[*index].path;
                vec![
                    String::new(),
                    format!(
                        "{}  ({})",
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy(),
                        path.parent().unwrap_or_else(|| Path::new("")).display()
                    ),
                ]
            })
            .collect(),
        grid,
    );
    let mut row = RowStyle::from_theme(theme);
    row.width = width;
    row.height = 30.0;
    row.padding = 0.0;
    row.gap = 6.0;
    builder.row(row, |r| {
        controls.toggle = r.tool_button(if batch.all_selected() {
            "Deselect All"
        } else {
            "Select All"
        });
        controls.matching = r.tool_button("Match Similar");
    });
    controls.separate = builder.switch_styled(
        "Separate tables",
        batch.separate,
        SwitchStyle::from_theme(theme),
    );
    controls
}

pub fn configure(ui: &mut next_gui_ui::Ui, controls: Controls, state: &crate::state::AppState) {
    if let Some(batch) = state.csv_files.as_ref().filter(|_| controls.table.0 != 0) {
        let visible = batch.visible();
        ui.set_table_check_cells(
            controls.table,
            visible
                .iter()
                .map(|index| vec![Some(batch.files[*index].selected), None])
                .collect(),
        );
        let tint = next_gui_scene::Color::rgba(34, 197, 94, 35);
        ui.set_table_cell_bg(
            controls.table,
            visible
                .iter()
                .map(|index| vec![batch.files[*index].matched.then_some(tint); 2])
                .collect(),
        );
        for (index, width) in batch.widths.iter().take(2).enumerate() {
            ui.set_table_column_width(controls.table, index, *width);
        }
        ui.set_table_selected_row(
            controls.table,
            visible.iter().position(|index| *index == batch.current),
        );
        ui.set_table_selected_col(controls.table, Some(0));
        ui.set_accessible_name(
            controls.table,
            format!("CSV files to import; selected {}; use Space to include or exclude the selected file",batch.files[batch.current].path.display()),
        );
    }
    if controls.message.0 != 0 {
        ui.set_text_area_read_only(controls.message, true);
        ui.set_accessible_name(controls.message, "CSV existing table import question");
    }
}

pub fn build_confirmation(
    ui: &mut next_gui_ui::Ui,
    controls: &mut Controls,
    state: &crate::state::AppState,
    theme: &next_gui_ui::Theme,
    width: f32,
) {
    use next_gui_ui::{ModalStyle, TextAreaStyle};
    let Some(work) = state
        .csv_work
        .as_ref()
        .filter(|work| work.pending.is_some())
    else {
        return;
    };
    let mut style = ModalStyle::from_theme(theme);
    style.width = (width - 80.0).clamp(360.0, 620.0);
    style.height = 230.0;
    let mut message = TextAreaStyle::from_theme(theme);
    message.width = style.width - 48.0;
    message.height = 112.0;
    let question = format!(
        "File: {}\nExisting table: {}\nImport rows by column position?{}",
        work.label(),
        work.target(),
        if work.options.conflict == crate::db::CsvConflict::Replace {
            "\nConflicting rows will be replaced."
        } else {
            ""
        }
    );
    controls.dialog = ui.dialog(
        "Import into Existing Table",
        style,
        |body| {
            controls.message = body.text_area(question, "", message);
        },
        |footer| {
            controls.answers[0] = footer.tool_button("Yes");
            controls.answers[1] =
                footer.tool_button_styled("No", theme.colors.accent, theme.colors.on_accent);
            controls.answers[2] = footer.tool_button("Yes to All");
            controls.answers[3] = footer.tool_button("Cancel");
        },
    );
    ui.set_text_area_read_only(controls.message, true);
    ui.set_accessible_name(controls.message, "CSV existing table import question");
    ui.set_dialog_open(controls.dialog, true);
}

pub fn restore(ui: &mut next_gui_ui::Ui, controls: Controls, state: &crate::state::AppState) {
    if let Some(batch) = &state.csv_files {
        if controls.table.0 != 0 {
            ui.set_table_scroll_offset(controls.table, batch.scroll.0, batch.scroll.1);
        }
    }
    if let Some(work) = state
        .csv_work
        .as_ref()
        .filter(|work| work.pending.is_some())
    {
        ui.set_focus(Some(controls.answers[work.focus.min(3)]));
    } else if let Some(focus) = state.csv_files.as_ref().and_then(|batch| batch.focus) {
        let id = [
            controls.table,
            controls.separate,
            controls.toggle,
            controls.matching,
        ][focus];
        if id.0 != 0 {
            ui.set_focus(Some(id));
        }
    }
}

impl crate::Demo {
    pub(crate) fn snapshot_csv_view(&mut self) {
        let controls = self.ids.csv_files;
        if let Some(work) = self
            .state
            .csv_work
            .as_mut()
            .filter(|work| work.pending.is_some())
        {
            if let Some(index) = controls
                .answers
                .iter()
                .position(|id| id.0 != 0 && self.ui.focused() == Some(*id))
            {
                work.focus = index;
            }
            return;
        }
        if self.state.overlay != Some(crate::state::OverlayKind::ImportCsv) {
            return;
        }
        let Some(batch) = &mut self.state.csv_files else {
            return;
        };
        if let Some(scroll) = self.ui.table_scroll_offset(controls.table) {
            batch.scroll = scroll;
        }
        if let Some(widths) = self.ui.table_column_widths(controls.table) {
            batch.widths = widths;
        }
        batch.focus = [
            controls.table,
            controls.separate,
            controls.toggle,
            controls.matching,
        ]
        .iter()
        .position(|id| id.0 != 0 && self.ui.focused() == Some(*id));
    }

    pub(crate) fn snapshot_csv_files(&mut self) {
        self.snapshot_csv_view();
        if self.state.csv_work.is_some()
            || self.state.overlay != Some(crate::state::OverlayKind::ImportCsv)
        {
            return;
        }
        let controls = self.ids.csv_files;
        let Some(batch) = &mut self.state.csv_files else {
            return;
        };
        self.ui.set_table_selected_col(controls.table, Some(0));
        if let Some(separate) = self.ui.is_checked(controls.separate) {
            if separate != batch.separate {
                batch.set_separate(separate, &self.state.csv_table);
                self.state.csv_table = batch.target(batch.current).into();
                self.rebuild = true;
            }
        }
        let index = self
            .ui
            .table_selected_row(controls.table)
            .and_then(|row| batch.visible().get(row).copied());
        batch.save_target(&self.state.csv_table);
        if let Some(index) = index {
            if self.state.select_csv_file(index) {
                self.rebuild = true;
            }
        }
    }

    pub(crate) fn handle_csv_files_button(&mut self, id: next_gui_ui::WidgetId) -> bool {
        let controls = self.ids.csv_files;
        if id.0 == 0 || (id != controls.toggle && id != controls.matching) {
            return false;
        }
        let options = self.state.csv_options();
        let Some(batch) = &mut self.state.csv_files else {
            return false;
        };
        if id == controls.toggle {
            batch.select_all(!batch.all_selected());
        } else {
            match batch.match_similar(&options) {
                Ok(errors) => {
                    self.state.status_msg = if errors.is_empty() {
                        format!(
                            "Selected {} files with matching headers",
                            batch.queue().len()
                        )
                    } else {
                        format!(
                            "Selected {} matching files; {}",
                            batch.queue().len(),
                            errors.join("; ")
                        )
                    };
                }
                Err(error) => self.set_error(error),
            }
        }
        self.rebuild = true;
        true
    }

    pub(crate) fn handle_csv_answer_button(&mut self, id: next_gui_ui::WidgetId) {
        let Some(index) = self
            .ids
            .csv_files
            .answers
            .iter()
            .position(|button| button.0 != 0 && *button == id)
        else {
            return;
        };
        match self.state.continue_csv_import_work(Some(
            [Answer::Yes, Answer::No, Answer::YesAll, Answer::Cancel][index],
        )) {
            Ok(()) => self.clear_error(),
            Err(error) => self.set_error(error),
        }
        self.rebuild = true;
    }

    pub(crate) fn handle_csv_confirmation(&mut self) {
        if self.ui.take_dialog_dismiss() == Some(self.ids.csv_files.dialog) {
            match self.state.continue_csv_import_work(Some(Answer::Cancel)) {
                Ok(()) => self.clear_error(),
                Err(error) => self.set_error(error),
            }
            self.rebuild = true;
            return;
        }
        while let Some(id) = self.ui.take_button_activate() {
            self.handle_csv_answer_button(id);
        }
    }
}

#[cfg(test)]
mod interaction_tests {
    use crate::{
        clear_color, encode_scene, ui_build, write_ppm, Demo, GpuRenderer, PresentBackend,
        SoftRenderer,
    };
    use next_gui_render::Renderer;
    use next_gui_ui::{TableStyle, Theme, UiKey, WidgetId};

    fn finish(demo: &mut Demo) {
        demo.handle_field_check_change();
        demo.snapshot_inputs();
        if demo.rebuild {
            demo.do_rebuild();
        }
        demo.ui.sync(&mut demo.scene);
    }

    fn bounds(demo: &Demo, id: WidgetId) -> next_gui_scene::Rect {
        demo.scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(id))
            .unwrap()
            .bounds
    }

    fn capture(demo: &mut Demo, theme: &str, width: u32, phase: &str) {
        let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_CSV_BATCH_QA_OUT") else {
            return;
        };
        let (w, h) = (demo.win_w, demo.win_h);
        demo.scene.mark_full_dirty(w as f32, h as f32);
        let dirty = demo.scene.take_dirty();
        let commands = encode_scene(&demo.scene, &dirty);
        let clear = clear_color(&demo.theme);
        let mut soft = SoftRenderer::new(w, h, clear);
        soft.render(&commands, &dirty);
        let a = soft.frame_pixels().unwrap().0;
        write_ppm(&format!("{prefix}-{theme}-{width}-{phase}.ppm"), a, w, h).unwrap();
        if let Ok(mut gpu) = GpuRenderer::try_new(w, h, clear) {
            gpu.render(&commands, &dirty);
            let b = gpu.frame_pixels().unwrap().0;
            write_ppm(
                &format!("{prefix}-{theme}-{width}-{phase}-metal.ppm"),
                b,
                w,
                h,
            )
            .unwrap();
            let delta = a
                .iter()
                .zip(b)
                .flat_map(|(a, b)| {
                    [0, 8, 16].map(|shift| {
                        (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs()
                    })
                })
                .max()
                .unwrap();
            assert!(delta <= 2, "{theme}/{width}/{phase}: {delta}");
            eprintln!("CSV batch {theme}/{width}/{phase}: Soft/Metal delta {delta}");
        } else {
            eprintln!("CSV batch GPU comparison not_run: adapter unavailable");
        }
    }

    #[test]
    fn csv_batch_file_controls_keep_targets_sessions_and_four_choice_confirmation() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let dir = tempfile::tempdir().unwrap();
                let names = [
                    "first",
                    "two",
                    "same",
                    "different",
                    "four",
                    "five",
                    "six",
                    "seven",
                ];
                let paths = names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        let path = dir.path().join(format!("{name}.csv"));
                        std::fs::write(
                            &path,
                            format!(
                                "id,{}\n{},{}\n",
                                if index == 3 { "other" } else { "name" },
                                index + 1,
                                name
                            ),
                        )
                        .unwrap();
                        path
                    })
                    .collect();
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                assert!(demo.state.open_csv_files(paths));
                assert!(!demo.state.csv_has_header);
                assert!(demo.state.csv_import_settings.trim_fields);
                demo.state.csv_has_header = true;
                demo.do_rebuild();
                capture(&mut demo, theme_name, width, "files");
                let grid = bounds(&demo, demo.ids.csv_files.table);
                let style = TableStyle::from_theme(&demo.theme);
                let (x, y) = (
                    grid.x + style.padding + 20.0,
                    grid.y + style.header_height + style.row_height * 0.5,
                );
                assert_eq!(
                    demo.ui.hit_widget(&demo.scene, x, y),
                    Some(demo.ids.csv_files.table)
                );
                assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
                demo.ui.pointer_released();
                finish(&mut demo);
                assert!(!demo.state.csv_files.as_ref().unwrap().files[0].selected);
                assert_eq!(demo.ui.focused(), Some(demo.ids.csv_files.table));
                assert!(demo.ui.key_input(UiKey::Space));
                finish(&mut demo);
                assert!(demo.state.csv_files.as_ref().unwrap().files[0].selected);
                demo.ui.set_checked(demo.ids.csv_files.separate, true);
                finish(&mut demo);
                demo.ui.set_focus(Some(demo.ids.ov_csv_table));
                demo.ui.key_input(UiKey::SelectAll);
                demo.ui.paste_into_focused_text("renamed first");
                finish(&mut demo);
                demo.ui.set_focus(Some(demo.ids.csv_files.table));
                assert!(demo.ui.key_input(UiKey::Down));
                finish(&mut demo);
                assert_eq!(demo.state.csv_table, "two");
                assert_eq!(demo.state.csv_files.as_ref().unwrap().current, 1);
                demo.ui.set_focus(Some(demo.ids.ov_csv_table));
                demo.ui.key_input(UiKey::Undo);
                finish(&mut demo);
                assert_eq!(demo.ui.text_input_value(demo.ids.ov_csv_table), Some("two"));
                demo.ui.set_focus(Some(demo.ids.csv_files.table));
                demo.ui.key_input(UiKey::Up);
                finish(&mut demo);
                assert_eq!(demo.state.csv_table, "renamed first");
                demo.ui
                    .set_table_column_width(demo.ids.csv_files.table, 1, 510.0);
                demo.ui
                    .set_table_scroll_offset(demo.ids.csv_files.table, 70.0, 60.0);
                finish(&mut demo);
                let scroll = demo.state.csv_files.as_ref().unwrap().scroll;
                demo.win_w = if width == 800 { 1400 } else { 800 };
                demo.win_h = if height == 600 { 1000 } else { 600 };
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.csv_files.table));
                assert_eq!(
                    demo.ui.table_scroll_offset(demo.ids.csv_files.table),
                    Some(scroll)
                );
                assert_eq!(
                    demo.ui
                        .table_column_widths(demo.ids.csv_files.table)
                        .unwrap()[1],
                    510.0
                );
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                demo.ui
                    .set_table_scroll_offset(demo.ids.csv_files.table, 0.0, 0.0);
                finish(&mut demo);
                demo.handle_button(demo.ids.csv_files.toggle);
                demo.do_rebuild();
                assert!(demo.state.csv_files.as_ref().unwrap().queue().is_empty());
                demo.handle_button(demo.ids.csv_files.toggle);
                demo.do_rebuild();
                assert_eq!(demo.state.csv_files.as_ref().unwrap().queue().len(), 8);
                demo.handle_button(demo.ids.csv_files.matching);
                demo.do_rebuild();
                assert_eq!(
                    demo.state.csv_files.as_ref().unwrap().queue(),
                    [0, 1, 2, 4, 5, 6, 7]
                );
                capture(&mut demo, theme_name, width, "separate");
                demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE \"renamed first\"(id INTEGER,name TEXT);INSERT INTO \"renamed first\" VALUES(0,'saved');CREATE TABLE two(id INTEGER,name TEXT);INSERT INTO two VALUES(0,'saved');").unwrap();
                demo.handle_button(demo.ids.ov_btn_ok);
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.csv_files.answers[1]));
                assert_eq!(
                    demo.state.csv_work.as_ref().unwrap().target(),
                    "renamed first"
                );
                capture(&mut demo, theme_name, width, "confirm");
                assert!(demo.ui.key_input(UiKey::Escape));
                demo.handle_csv_confirmation();
                demo.do_rebuild();
                assert!(demo.state.csv_work.is_none());
                assert_eq!(demo.state.csv_table, "renamed first");
                demo.handle_button(demo.ids.ov_btn_ok);
                demo.do_rebuild();
                assert!(demo.ui.key_input(UiKey::Enter));
                demo.handle_csv_confirmation();
                demo.do_rebuild();
                assert_eq!(demo.state.csv_work.as_ref().unwrap().target(), "two");
                assert_eq!(demo.state.csv_table, "two");
                assert!(demo.ui.key_input(UiKey::Tab));
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.csv_files.answers[2]));
                assert!(demo.ui.key_input(UiKey::Enter));
                demo.handle_csv_confirmation();
                demo.do_rebuild();
                assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                assert!(demo.state.csv_work.is_none());
                assert_eq!(demo.state.csv_table, "different");
                let batch = demo.state.csv_files.as_ref().unwrap();
                assert_eq!(batch.visible(), [3]);
                assert!(batch.queue().is_empty());
                capture(&mut demo, theme_name, width, "remaining");
                let conn = demo.state.conn.as_ref().unwrap();
                assert_eq!(crate::db::count_rows(conn, "renamed first").unwrap(), 1);
                assert_eq!(crate::db::count_rows(conn, "two").unwrap(), 2);
                for name in ["same", "four", "five", "six", "seven"] {
                    assert_eq!(crate::db::count_rows(conn, name).unwrap(), 1);
                }
                demo.ui.set_focus(Some(demo.ids.csv_files.table));
                demo.ui
                    .set_table_selected_col(demo.ids.csv_files.table, Some(0));
                assert!(demo.ui.key_input(UiKey::Space));
                finish(&mut demo);
                demo.handle_button(demo.ids.ov_btn_ok);
                demo.do_rebuild();
                assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                assert!(demo.state.overlay.is_none());
                assert!(demo.state.csv_files.is_none());
                assert_eq!(
                    demo.state
                        .conn
                        .as_ref()
                        .unwrap()
                        .query_row("SELECT other FROM different", [], |row| row
                            .get::<_, String>(0))
                        .unwrap(),
                    "different"
                );
            }
        }
    }
}
