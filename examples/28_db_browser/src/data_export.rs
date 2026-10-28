//! Original CSV/JSON table export options and staged multi-table output.
use rusqlite::{types::ValueRef, Connection};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Options {
    pub header: bool,
    pub separator: Option<char>,
    pub quote: Option<char>,
    pub newline: String,
    pub pretty: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            header: true,
            separator: Some(','),
            quote: Some('"'),
            newline: if cfg!(target_os = "windows") {
                "\r\n"
            } else {
                "\n"
            }
            .into(),
            pretty: true,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if self.newline.is_empty() || self.newline.contains('\0') {
            return Err("Choose nonempty line-ending characters without NUL".into());
        }
        for ch in [self.separator, self.quote].into_iter().flatten() {
            if matches!(ch, '\0' | '\r' | '\n') {
                return Err("Separator and quote cannot be NUL or line breaks".into());
            }
        }
        if self.separator.is_some() && self.separator == self.quote {
            return Err("Separator and quote must differ".into());
        }
        Ok(())
    }
    fn field(&self, value: &str) -> String {
        let quote = self
            .quote
            .or_else(|| value.contains(['\r', '\n']).then_some('"'));
        match quote {
            Some(ch)
                if value.contains(ch)
                    || value.contains(['\r', '\n'])
                    || self
                        .separator
                        .is_some_and(|separator| value.contains(separator))
                    || value.contains(&self.newline) =>
            {
                format!("{ch}{}{ch}", value.replace(ch, &format!("{ch}{ch}")))
            }
            _ => value.into(),
        }
    }
    fn record(
        &self,
        out: &mut impl Write,
        values: impl IntoIterator<Item = String>,
    ) -> Result<(), String> {
        for (index, value) in values.into_iter().enumerate() {
            if index > 0 {
                if let Some(ch) = self.separator {
                    write!(out, "{ch}").map_err(|e| e.to_string())?;
                }
            }
            write!(out, "{}", self.field(&value)).map_err(|e| e.to_string())?;
        }
        write!(out, "{}", self.newline).map_err(|e| e.to_string())
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (position, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            result.push(if position > chunk.len() {
                '='
            } else {
                ALPHABET[((value >> shift) & 63) as usize] as char
            });
        }
    }
    result
}
fn csv_value(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => match std::str::from_utf8(bytes) {
            Ok(text)
                if text.chars().all(|ch| {
                    (!ch.is_control() || ch.is_whitespace())
                        && !matches!(ch as u32, 0xfdd0..=0xfdef)
                        && ch as u32 & 0xfffe != 0xfffe
                }) =>
            {
                text.into()
            }
            _ => base64(bytes),
        },
    }
}
fn json_value(value: ValueRef<'_>) -> serde_json::Value {
    match value {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(v) => v.into(),
        ValueRef::Real(v) => serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned().into(),
        ValueRef::Blob(v) => base64(v).into(),
    }
}

pub struct Output {
    pub schema: String,
    pub table: String,
    pub path: PathBuf,
}

pub fn filename(schema: &str, table: &str, json: bool) -> String {
    let encode = |text: &str| {
        let mut output = String::new();
        for ch in text.chars() {
            if ch.is_control() || "<>:\"/\\|?*%.".contains(ch) {
                let mut bytes = [0; 4];
                for byte in ch.encode_utf8(&mut bytes).as_bytes() {
                    output.push_str(&format!("%{byte:02X}"));
                }
            } else {
                output.push(ch);
            }
        }
        if output.ends_with(' ') {
            output.pop();
            output.push_str("%20");
        }
        if [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&output.to_ascii_uppercase().as_str())
        {
            output.insert(0, '_');
        }
        output
    };
    let name = if schema == "main" {
        encode(table)
    } else {
        format!("{}.{}", encode(schema), encode(table))
    };
    format!("{name}.{}", if json { "json" } else { "csv" })
}

fn write_rows(
    output: &mut impl Write,
    columns: &[String],
    rows: impl IntoIterator<Item = Result<Vec<crate::db::CellValue>, String>>,
    json: bool,
    options: &Options,
) -> Result<(), String> {
    if json
        && columns
            .iter()
            .enumerate()
            .any(|(i, name)| columns[..i].contains(name))
    {
        return Err("JSON needs unique column names; use SQL aliases or export CSV".into());
    }
    if !json {
        options.validate()?;
    }
    let mut out = std::io::BufWriter::new(output);
    if json {
        write!(out, "[").map_err(|e| e.to_string())?;
    } else if options.header {
        options.record(&mut out, columns.iter().cloned())?;
    }
    let mut count = 0u64;
    for cells in rows {
        let cells = cells?;
        if cells.len() != columns.len() {
            return Err("Stored result has an incomplete row".into());
        }
        if json {
            if count > 0 {
                write!(out, ",").map_err(|e| e.to_string())?;
            }
            let object: serde_json::Map<_, _> = columns
                .iter()
                .cloned()
                .zip(cells.iter().map(|cell| json_value(cell.as_ref())))
                .collect();
            if options.pretty {
                writeln!(out).map_err(|e| e.to_string())?;
                let text = serde_json::to_string_pretty(&object).map_err(|e| e.to_string())?;
                for (index, line) in text.lines().enumerate() {
                    if index > 0 {
                        writeln!(out).map_err(|e| e.to_string())?;
                    }
                    write!(out, "    {line}").map_err(|e| e.to_string())?;
                }
            } else {
                serde_json::to_writer(&mut out, &object).map_err(|e| e.to_string())?;
            }
        } else {
            options.record(&mut out, cells.iter().map(|cell| csv_value(cell.as_ref())))?;
        }
        count += 1;
    }
    if json {
        if count > 0 && options.pretty {
            writeln!(out).map_err(|e| e.to_string())?;
        }
        write!(out, "]").map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())
}

fn stage(
    conn: &Connection,
    output: &Output,
    json: bool,
    options: &Options,
) -> Result<tempfile::NamedTempFile, String> {
    crate::db::guard_output_path(conn, &output.path)?;
    let sql = format!(
        "SELECT * FROM {}",
        crate::db::qualified_name(&output.schema, &output.table)
    );
    let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|s| (*s).into())
        .collect();
    let parent = output
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut out = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            (0..columns.len())
                .map(|index| row.get_ref(index).map(crate::db::CellValue::from))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|e| e.to_string())?;
    write_rows(
        &mut out,
        &columns,
        rows.map(|row| row.map_err(|e| e.to_string())),
        json,
        options,
    )?;
    out.as_file_mut().sync_all().map_err(|e| e.to_string())?;
    Ok(out)
}

pub struct QuerySource {
    columns: Vec<String>,
    rows: std::sync::Arc<tempfile::NamedTempFile>,
}
impl QuerySource {
    pub fn new(result: &crate::db::QueryResult) -> Result<Self, String> {
        if result.columns.is_empty() {
            return Err("The statement returned no columns".into());
        }
        Ok(Self {
            columns: result.columns.clone(),
            rows: result
                .export_rows
                .clone()
                .ok_or("Execute SQL before exporting results")?,
        })
    }
}

pub fn export_query(
    conn: &Connection,
    source: &QuerySource,
    path: &Path,
    json: bool,
    options: &Options,
    replace: bool,
) -> Result<(), String> {
    use std::io::BufRead;
    crate::db::guard_output_path(conn, path)?;
    if !replace && path.exists() {
        return Err(format!("Output already exists: {}", path.display()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut out = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let rows = std::io::BufReader::new(source.rows.reopen().map_err(|e| e.to_string())?)
        .lines()
        .map(|line| {
            serde_json::from_str::<Vec<crate::db::CellValue>>(&line.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())
        });
    write_rows(&mut out, &source.columns, rows, json, options)?;
    out.as_file_mut().sync_all().map_err(|e| e.to_string())?;
    let result = if replace {
        out.persist(path)
    } else {
        out.persist_noclobber(path)
    };
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Stage every selected result before replacing any destination. Each final file is atomic.
// shortcut: staging runs synchronously; move connection-owned export work to a worker for large-file UI responsiveness.
pub fn export(
    conn: &Connection,
    outputs: &[Output],
    json: bool,
    options: &Options,
    replace: bool,
) -> Result<(), String> {
    if !json {
        options.validate()?;
    }
    if outputs.is_empty() {
        return Err("Select at least one table or view".into());
    }
    let mut paths = Vec::new();
    for output in outputs {
        crate::db::guard_output_path(conn, &output.path)?;
        let parent = output
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let name = output.path.file_name().ok_or("Export needs a filename")?;
        let key = parent.join(name).to_string_lossy().to_lowercase();
        if paths.contains(&key) {
            return Err("Selected exports have colliding filenames; export them separately".into());
        }
        paths.push(key);
        if output.path.is_dir() {
            return Err(format!(
                "Destination is a directory: {}",
                output.path.display()
            ));
        }
        if !replace && output.path.exists() {
            return Err(format!("Output already exists: {}", output.path.display()));
        }
    }
    let staged = outputs
        .iter()
        .map(|output| stage(conn, output, json, options))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, (output, file)) in outputs.iter().zip(staged).enumerate() {
        let result = if replace {
            file.persist(&output.path)
        } else {
            file.persist_noclobber(&output.path)
        };
        if let Err(error) = result {
            return Err(format!(
                "Export stopped after {index} completed files: {}: {error}",
                output.path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_formats_use_raw_snapshot_types_and_original_csv_json_options() {
        let conn = Connection::open_in_memory().unwrap();
        let result = crate::db::execute_sql(&conn, "SELECT 9223372036854775807 AS integer_value, X'4142' AS printable_blob, X'00FF' AS binary_blob, 'AP8=' AS literal_text, CAST(X'80' AS TEXT) AS invalid_text, 9e999 AS infinite, NULL AS missing, 'a§b' AS quoted").unwrap();
        let source = QuerySource::new(&result).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("result");
        let options = Options {
            header: false,
            separator: Some('§'),
            quote: Some('«'),
            newline: "\r\n".into(),
            pretty: false,
        };
        export_query(&conn, &source, &path, false, &options, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "9223372036854775807§AB§AP8=§AP8=§gA==§inf§§«a§b«\r\n"
        );
        export_query(&conn, &source, &path, true, &options, true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains('\n'));
        let rows: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(rows[0]["integer_value"].as_i64(), Some(i64::MAX));
        assert_eq!(rows[0]["printable_blob"], "QUI=");
        assert_eq!(rows[0]["binary_blob"], "AP8=");
        assert_eq!(rows[0]["literal_text"], "AP8=");
        assert_eq!(rows[0]["invalid_text"], "�");
        assert!(rows[0]["infinite"].is_null() && rows[0]["missing"].is_null());
        assert!(export_query(&conn, &source, &path, true, &options, false).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn query_export_errors_preserve_existing_files_and_live_databases() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("live.db");
        let conn = Connection::open(&database).unwrap();
        conn.execute_batch("CREATE TABLE t(v);INSERT INTO t VALUES(7)")
            .unwrap();
        let result = crate::db::execute_sql(&conn, "SELECT v FROM t").unwrap();
        let source = QuerySource::new(&result).unwrap();
        assert!(export_query(&conn, &source, &database, true, &Options::default(), true).is_err());
        assert_eq!(crate::db::count_rows(&conn, "t").unwrap(), 1);
        let path = directory.path().join("keep.json");
        std::fs::write(&path, "previous").unwrap();
        let duplicate = crate::db::execute_sql(&conn, "SELECT 1 AS v, 2 AS v").unwrap();
        assert!(export_query(
            &conn,
            &QuerySource::new(&duplicate).unwrap(),
            &path,
            true,
            &Options::default(),
            true
        )
        .is_err());
        let invalid = Options {
            newline: String::new(),
            ..Options::default()
        };
        assert!(export_query(&conn, &source, &path, false, &invalid, true).is_err());
        let malformed = serde_json::to_string(&vec![
            crate::db::CellValue::Integer(1),
            crate::db::CellValue::Null,
        ])
        .unwrap();
        std::fs::write(source.rows.path(), format!("{malformed}\n")).unwrap();
        assert!(
            export_query(&conn, &source, &path, true, &Options::default(), true)
                .unwrap_err()
                .contains("incomplete row")
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
        std::fs::write(source.rows.path(), "corrupt\n").unwrap();
        assert!(export_query(&conn, &source, &path, true, &Options::default(), true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
    }

    #[test]
    fn sql_result_buttons_share_compact_options_and_capture_the_executed_snapshot() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let directory = tempfile::tempdir().unwrap();
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                demo.state.tab = crate::state::MainTab::ExecuteSql;
                demo.state.sql_result = Some(
                    crate::db::execute_sql(
                        demo.state.conn.as_ref().unwrap(),
                        "SELECT 1234.5 AS number, X'00FF' AS binary",
                    )
                    .unwrap(),
                );
                demo.do_rebuild();
                demo.handle_button(demo.ids.btn_sql_csv);
                demo.do_rebuild();
                assert_eq!(demo.ui.active_dialog(), Some(demo.ids.data_export.dialog));
                assert_eq!(demo.ids.data_export.table.0, 0);
                assert!(demo.ui.button_enabled(demo.ids.data_export.export).unwrap());
                demo.ui.set_focus(Some(demo.ids.data_export.header));
                demo.ui.key_input(UiKey::Space);
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert!(!demo.state.export_dialog.as_ref().unwrap().options.header);
                capture(&mut demo, theme_name, width, "query-export-csv");
                demo.state.sql_result = Some(
                    crate::db::execute_sql(
                        demo.state.conn.as_ref().unwrap(),
                        "SELECT 'later result' AS changed",
                    )
                    .unwrap(),
                );
                let path = directory.path().join("query.csv");
                std::fs::write(&path, "previous").unwrap();
                demo.state.export_dialog.as_mut().unwrap().pending = Some(vec![Output {
                    schema: "main".into(),
                    table: "query".into(),
                    path: path.clone(),
                }]);
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.data_export.keep));
                demo.ui.key_input(UiKey::Escape);
                demo.handle_data_export_dismiss();
                demo.do_rebuild();
                assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
                demo.state.export_dialog.as_mut().unwrap().pending = Some(vec![Output {
                    schema: "main".into(),
                    table: "query".into(),
                    path: path.clone(),
                }]);
                demo.do_rebuild();
                demo.handle_button(demo.ids.data_export.replace);
                assert!(demo.state.export_dialog.is_none());
                assert_eq!(
                    std::fs::read_to_string(&path).unwrap(),
                    format!("1234.5,AP8={}", Options::default().newline)
                );
                demo.do_rebuild();
                demo.handle_button(demo.ids.btn_sql_json);
                demo.do_rebuild();
                demo.ui.set_checked(demo.ids.data_export.pretty, false);
                demo.snapshot_inputs();
                demo.do_rebuild();
                capture(&mut demo, theme_name, width, "query-export-json");
                demo.state.export_dialog.as_mut().unwrap().pending = Some(vec![Output {
                    schema: "main".into(),
                    table: "query".into(),
                    path: path.clone(),
                }]);
                demo.do_rebuild();
                demo.handle_button(demo.ids.data_export.replace);
                assert_eq!(
                    std::fs::read_to_string(&path).unwrap(),
                    "[{\"changed\":\"later result\"}]"
                );
                assert!(!demo.state.prefs.data_export.pretty);
            }
        }
    }
    #[test]
    fn export_dialog_keeps_multi_selection_options_focus_and_safe_replace_decision() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let directory = tempfile::tempdir().unwrap();
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE first(v);INSERT INTO first VALUES('日本');CREATE TABLE second(v);INSERT INTO second VALUES(42);CREATE VIEW viewed AS SELECT * FROM second").unwrap();
                demo.state.refresh_schema().unwrap();
                demo.open_data_export(false);
                demo.do_rebuild();
                assert_eq!(demo.ui.active_dialog(), Some(demo.ids.data_export.dialog));
                assert_eq!(demo.state.export_dialog.as_ref().unwrap().tables.len(), 3);
                demo.ui.set_focus(Some(demo.ids.data_export.table));
                demo.ui
                    .set_table_selected_row(demo.ids.data_export.table, Some(1));
                demo.ui.key_input(UiKey::Space);
                demo.handle_field_check_change();
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert!(demo.state.export_dialog.as_ref().unwrap().tables[1].2);
                demo.handle_button(demo.ids.data_export.toggle);
                demo.do_rebuild();
                assert!(demo
                    .state
                    .export_dialog
                    .as_ref()
                    .unwrap()
                    .tables
                    .iter()
                    .all(|entry| entry.2));
                demo.ui.set_focus(Some(demo.ids.data_export.separator));
                demo.ui.key_input(UiKey::Up);
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert_ne!(demo.ids.data_export.custom_separator.0, 0);
                demo.ui
                    .set_focus(Some(demo.ids.data_export.custom_separator));
                demo.ui.key_input(UiKey::SelectAll);
                demo.ui.paste_into_focused_text("§");
                demo.snapshot_inputs();
                demo.win_w = if width == 800 { 1400 } else { 800 };
                demo.win_h = if height == 600 { 1000 } else { 600 };
                demo.do_rebuild();
                assert_eq!(
                    demo.ui.focused(),
                    Some(demo.ids.data_export.custom_separator)
                );
                assert_eq!(
                    demo.ui
                        .text_input_value(demo.ids.data_export.custom_separator),
                    Some("§")
                );
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                capture(&mut demo, theme_name, width, "export-csv");
                let output = directory.path().join("keep.csv");
                std::fs::write(&output, "previous").unwrap();
                demo.state.export_dialog.as_mut().unwrap().pending = Some(vec![Output {
                    schema: "main".into(),
                    table: "first".into(),
                    path: output.clone(),
                }]);
                demo.do_rebuild();
                assert_eq!(demo.ui.active_dialog(), Some(demo.ids.data_export.confirm));
                assert_eq!(demo.ui.focused(), Some(demo.ids.data_export.keep));
                capture(&mut demo, theme_name, width, "export-confirm");
                demo.ui.key_input(UiKey::Escape);
                demo.handle_data_export_dismiss();
                demo.do_rebuild();
                assert!(demo.state.export_dialog.as_ref().unwrap().pending.is_none());
                assert_eq!(std::fs::read_to_string(&output).unwrap(), "previous");
                let options = demo
                    .state
                    .export_dialog
                    .as_ref()
                    .unwrap()
                    .options()
                    .unwrap();
                assert_eq!(options.separator, Some('§'));
                demo.state.export_dialog.as_mut().unwrap().pending = Some(vec![Output {
                    schema: "main".into(),
                    table: "first".into(),
                    path: output.clone(),
                }]);
                demo.do_rebuild();
                demo.handle_button(demo.ids.data_export.replace);
                assert!(demo.state.export_dialog.is_none());
                assert!(std::fs::read_to_string(&output).unwrap().contains("日本"));
                assert_eq!(demo.state.prefs.data_export.separator, Some('§'));
                demo.open_data_export(true);
                demo.do_rebuild();
                capture(&mut demo, theme_name, width, "export-json");
                demo.ui.key_input(UiKey::Escape);
                demo.handle_data_export_dismiss();
                assert!(demo.state.export_dialog.is_none());
                let restored: crate::prefs::Prefs =
                    serde_json::from_str(&serde_json::to_string(&demo.state.prefs).unwrap())
                        .unwrap();
                assert_eq!(restored.data_export.separator, Some('§'));
            }
        }
    }

    #[test]
    fn export_streams_all_rows_and_rejects_invalid_options_before_touching_files() {
        let directory = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v);WITH RECURSIVE n(v) AS (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<5001) INSERT INTO t SELECT v FROM n;CREATE TABLE empty(v)").unwrap();
        let output = Output {
            schema: "main".into(),
            table: "t".into(),
            path: directory.path().join("all.csv"),
        };
        let options = Options {
            header: false,
            quote: None,
            separator: None,
            newline: "\r\n".into(),
            ..Options::default()
        };
        export(&conn, std::slice::from_ref(&output), false, &options, false).unwrap();
        let text = std::fs::read_to_string(&output.path).unwrap();
        assert_eq!(text.lines().count(), 5001);
        assert!(text.ends_with("5001\r\n"));
        let old = std::fs::read(&output.path).unwrap();
        for options in [
            Options {
                newline: String::new(),
                ..Options::default()
            },
            Options {
                separator: Some('x'),
                quote: Some('x'),
                ..Options::default()
            },
        ] {
            assert!(export(&conn, std::slice::from_ref(&output), false, &options, true).is_err());
            assert_eq!(std::fs::read(&output.path).unwrap(), old);
        }
        let path = directory.path().join("empty.json");
        export(
            &conn,
            &[Output {
                schema: "main".into(),
                table: "empty".into(),
                path: path.clone(),
            }],
            true,
            &Options::default(),
            false,
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "[]");
        let old: crate::prefs::Prefs = serde_json::from_str("{}").unwrap();
        assert_eq!(old.data_export, Options::default());
    }

    #[test]
    fn original_export_options_preserve_types_binary_and_custom_unicode_csv() {
        for (bytes, expected) in [
            (b"".as_slice(), ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (&[0, 255], "AP8="),
        ] {
            assert_eq!(base64(bytes), expected);
        }
        let directory = tempfile::tempdir().unwrap();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(id, note, binary, nullable);INSERT INTO t VALUES(9223372036854775807,'日本§a«b'||char(10)||'c',X'00FF',NULL)").unwrap();
        let output = Output {
            schema: "main".into(),
            table: "t".into(),
            path: directory.path().join("t.csv"),
        };
        let options = Options {
            separator: Some('§'),
            quote: Some('«'),
            newline: "\r\n".into(),
            ..Options::default()
        };
        export(&conn, std::slice::from_ref(&output), false, &options, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&output.path).unwrap(),
            "id§note§binary§nullable\r\n9223372036854775807§«日本§a««b\nc«§AP8=§\r\n"
        );
        for pretty in [true, false] {
            let path = directory.path().join(format!("{pretty}.json"));
            export(
                &conn,
                &[Output {
                    schema: "main".into(),
                    table: "t".into(),
                    path: path.clone(),
                }],
                true,
                &Options {
                    pretty,
                    ..Options::default()
                },
                false,
            )
            .unwrap();
            let text = std::fs::read_to_string(path).unwrap();
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(value[0]["id"].as_i64(), Some(i64::MAX));
            assert!(value[0]["nullable"].is_null());
            assert_eq!(value[0]["binary"], "AP8=");
            assert_eq!(text.contains('\n'), pretty);
        }
    }
    #[test]
    fn multi_export_stages_queries_and_guards_all_outputs_before_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let db = directory.path().join("owned.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t(v);INSERT INTO t VALUES('saved');ATTACH ':memory:' AS aux;CREATE TABLE aux.t(v);INSERT INTO aux.t VALUES('attached');CREATE VIEW viewed AS SELECT * FROM t;BEGIN;INSERT INTO t VALUES('pending')").unwrap();
        let first = directory.path().join("main.csv");
        std::fs::write(&first, "keep").unwrap();
        let mut outputs = vec![
            Output {
                schema: "main".into(),
                table: "t".into(),
                path: first.clone(),
            },
            Output {
                schema: "aux".into(),
                table: "missing".into(),
                path: directory.path().join("aux.csv"),
            },
        ];
        assert!(export(&conn, &outputs, false, &Options::default(), true).is_err());
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "keep");
        outputs[1].table = "t".into();
        assert!(export(&conn, &outputs, false, &Options::default(), false).is_err());
        outputs[1].path = db.clone();
        assert!(export(&conn, &outputs, false, &Options::default(), true).is_err());
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "keep");
        outputs[1].path = directory.path().join("aux.csv");
        export(&conn, &outputs, false, &Options::default(), true).unwrap();
        assert!(std::fs::read_to_string(&first).unwrap().contains("pending"));
        assert!(std::fs::read_to_string(&outputs[1].path)
            .unwrap()
            .contains("attached"));
        assert!(!conn.is_autocommit());
        outputs[0].table = "viewed".into();
        outputs[0].path = directory.path().join("view.csv");
        export(
            &conn,
            std::slice::from_ref(&outputs[0]),
            false,
            &Options::default(),
            false,
        )
        .unwrap();
        assert!(std::fs::read_to_string(&outputs[0].path)
            .unwrap()
            .contains("pending"));
        outputs[1].path = outputs[0].path.clone();
        assert!(export(&conn, &outputs, false, &Options::default(), false).is_err());
        assert_eq!(filename("main", "../a:b", false), "%2E%2E%2Fa%3Ab.csv");
        assert_eq!(filename("aux", "t", true), "_aux.t.json");
        assert_eq!(filename("main", "CON", false), "_CON.csv");
    }
}

pub struct Draft {
    pub json: bool,
    query: Option<QuerySource>,
    pub tables: Vec<(String, String, bool)>,
    pub options: Options,
    separator: usize,
    quote: usize,
    newline: usize,
    custom_separator: String,
    custom_quote: String,
    custom_newline: String,
    pending: Option<Vec<Output>>,
    scroll: (f32, f32),
    selected: Option<usize>,
}
impl Draft {
    pub fn new(state: &crate::state::AppState, json: bool) -> Self {
        let options = state.prefs.data_export.clone();
        let mut tables: Vec<_> = state
            .schema
            .iter()
            .filter(|object| {
                matches!(
                    object.kind,
                    crate::db::ObjectKind::Table | crate::db::ObjectKind::View
                )
            })
            .map(|object| (object.schema.clone(), object.name.clone(), false))
            .collect();
        tables.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        let selected = state
            .selected
            .as_ref()
            .and_then(|selected| {
                tables.iter().position(|(schema, table, _)| {
                    schema == &selected.schema && table == &selected.name
                })
            })
            .or_else(|| {
                state.browse().table.as_ref().and_then(|name| {
                    tables.iter().position(|(schema, table, _)| {
                        schema == &state.browse().schema && table == name
                    })
                })
            })
            .or_else(|| (!tables.is_empty()).then_some(0));
        if let Some(index) = selected {
            tables[index].2 = true;
        }
        let separator = [Some(','), Some(';'), Some('\t'), Some('|'), None]
            .iter()
            .position(|v| *v == options.separator)
            .unwrap_or(5);
        let quote = [Some('"'), Some('\''), None]
            .iter()
            .position(|v| *v == options.quote)
            .unwrap_or(3);
        let newline = ["\n", "\r\n", "\r"]
            .iter()
            .position(|v| *v == options.newline)
            .unwrap_or(3);
        Self {
            json,
            query: None,
            tables,
            separator,
            quote,
            newline,
            custom_separator: options.separator.map(String::from).unwrap_or_default(),
            custom_quote: options.quote.map(String::from).unwrap_or_default(),
            custom_newline: options.newline.clone(),
            options,
            pending: None,
            scroll: (0.0, 0.0),
            selected,
        }
    }
    pub fn set_query(&mut self, result: &crate::db::QueryResult) -> Result<(), String> {
        self.query = Some(QuerySource::new(result)?);
        self.tables.clear();
        Ok(())
    }
    fn options(&self) -> Result<Options, String> {
        let mut options = self.options.clone();
        if !self.json {
            options.separator = match self.separator {
                0 => Some(','),
                1 => Some(';'),
                2 => Some('\t'),
                3 => Some('|'),
                4 => None,
                _ => Some(
                    crate::csv_import::character(&self.custom_separator, false)
                        .ok_or("Choose one separator character")?,
                ),
            };
            options.quote = match self.quote {
                0 => Some('"'),
                1 => Some('\''),
                2 => None,
                _ => Some(
                    crate::csv_import::character(&self.custom_quote, false)
                        .ok_or("Choose one quote character")?,
                ),
            };
            options.newline = match self.newline {
                0 => "\n",
                1 => "\r\n",
                2 => "\r",
                _ => &self.custom_newline,
            }
            .into();
            options.validate()?;
        }
        Ok(options)
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub dialog: next_gui_ui::WidgetId,
    pub table: next_gui_ui::WidgetId,
    toggle: next_gui_ui::WidgetId,
    header: next_gui_ui::WidgetId,
    separator: next_gui_ui::WidgetId,
    quote: next_gui_ui::WidgetId,
    newline: next_gui_ui::WidgetId,
    custom_separator: next_gui_ui::WidgetId,
    custom_quote: next_gui_ui::WidgetId,
    custom_newline: next_gui_ui::WidgetId,
    pretty: next_gui_ui::WidgetId,
    pub export: next_gui_ui::WidgetId,
    pub cancel: next_gui_ui::WidgetId,
    confirm: next_gui_ui::WidgetId,
    replace: next_gui_ui::WidgetId,
    keep: next_gui_ui::WidgetId,
}

impl Default for Controls {
    fn default() -> Self {
        let zero = next_gui_ui::WidgetId(0);
        Self {
            dialog: zero,
            table: zero,
            toggle: zero,
            header: zero,
            separator: zero,
            quote: zero,
            newline: zero,
            custom_separator: zero,
            custom_quote: zero,
            custom_newline: zero,
            pretty: zero,
            export: zero,
            cancel: zero,
            confirm: zero,
            replace: zero,
            keep: zero,
        }
    }
}

pub struct Session {
    focus: usize,
    text: Option<next_gui_ui::TextInputState>,
    select: Option<next_gui_ui::SelectState>,
}
impl Controls {
    fn focus_ids(&self) -> [next_gui_ui::WidgetId; 15] {
        [
            self.table,
            self.toggle,
            self.header,
            self.separator,
            self.quote,
            self.newline,
            self.custom_separator,
            self.custom_quote,
            self.custom_newline,
            self.pretty,
            self.export,
            self.cancel,
            self.keep,
            self.replace,
            self.confirm,
        ]
    }
    pub fn session(&self, ui: &next_gui_ui::Ui) -> Option<Session> {
        let (focus, id) = self
            .focus_ids()
            .into_iter()
            .enumerate()
            .find(|(_, id)| id.0 != 0 && ui.focused() == Some(*id))?;
        Some(Session {
            focus,
            text: ui.text_input_state(id),
            select: ui.select_state(id),
        })
    }
    pub fn restore(&self, ui: &mut next_gui_ui::Ui, session: &Session) {
        let id = self.focus_ids()[session.focus];
        if id.0 == 0 {
            return;
        }
        if let Some(text) = &session.text {
            ui.restore_text_input_state(id, text);
        }
        if let Some(select) = &session.select {
            ui.restore_select_state(id, select);
        }
        ui.set_focus(Some(id));
    }
}

pub fn build(
    ui: &mut next_gui_ui::Ui,
    state: &crate::state::AppState,
    theme: &next_gui_ui::Theme,
    width: f32,
    height: f32,
) -> Controls {
    use next_gui_ui::{
        ModalStyle, SelectStyle, SwitchStyle, TableColumn, TableStyle, TextAreaStyle,
        TextInputStyle,
    };
    let mut ids = Controls::default();
    let mut error_text = next_gui_ui::WidgetId(0);
    let Some(draft) = &state.export_dialog else {
        return ids;
    };
    let mut style = ModalStyle::from_theme(theme);
    style.width = (width - 48.0).min(740.0);
    style.height = (height - 48.0).min(if draft.query.is_some() {
        if draft.json {
            250.0
        } else {
            440.0
        }
    } else {
        690.0
    });
    style.close_on_backdrop = false;
    let inner = style.width - 2.0 * style.padding;
    ids.dialog = ui.dialog(
        if draft.json {
            "Export data as JSON"
        } else {
            "Export data as CSV"
        },
        style,
        |body| {
            if let Some(error) = &state.error_msg {
                let mut message = TextAreaStyle::from_theme(theme);
                message.width = inner;
                message.height = 64.0;
                error_text = body.text_area(error, "", message);
            }
            if draft.query.is_some() {
                body.text("SQL query result");
            } else {
                body.text("Tables and views");
                let mut grid = TableStyle::from_theme(theme);
                grid.width = inner;
                grid.height = (style.height * 0.33).clamp(132.0, 230.0);
                let rows = draft
                    .tables
                    .iter()
                    .map(|(schema, name, _)| {
                        vec![
                            String::new(),
                            if schema == "main" {
                                name.clone()
                            } else {
                                crate::db::qualified_name(schema, name)
                            },
                            filename(schema, name, draft.json),
                        ]
                    })
                    .collect();
                ids.table = body.table(
                    vec![
                        TableColumn::new("Use", 44.0),
                        TableColumn::new("Table / view", (inner - 44.0) * 0.5),
                        TableColumn::new("Filename", (inner - 44.0) * 0.5),
                    ],
                    rows,
                    grid,
                );
                ids.toggle =
                    body.tool_button(if draft.tables.iter().all(|(_, _, selected)| *selected) {
                        "Deselect All"
                    } else {
                        "Select All"
                    });
            }
            let sw = SwitchStyle::from_theme(theme);
            if draft.json {
                ids.pretty = body.switch_styled("Pretty print", draft.options.pretty, sw);
            } else {
                ids.header =
                    body.switch_styled("Column names in first line", draft.options.header, sw);
                let mut select = SelectStyle::from_theme(theme);
                select.width = 240.0;
                select.bg = theme.colors.surface;
                let mut row = next_gui_ui::RowStyle::from_theme(theme);
                row.width = inner;
                row.height = 34.0;
                row.padding = 0.0;
                row.background = None;
                body.row(row, |row| {
                    row.text("Field separator");
                    ids.separator = row.select(
                        ["Comma ,", "Semicolon ;", "Tab", "Pipe |", "None", "Other"],
                        Some(draft.separator),
                        "Separator",
                        select,
                    );
                });
                let mut input = TextInputStyle::from_theme(theme);
                input.width = 240.0;
                input.bg = theme.colors.surface;
                if draft.separator == 5 {
                    ids.custom_separator =
                        body.text_input_styled(&draft.custom_separator, "One character", input);
                }
                body.row(row, |row| {
                    row.text("Quote character");
                    ids.quote = row.select(
                        ["Double quote", "Single quote", "None", "Other"],
                        Some(draft.quote),
                        "Quote",
                        select,
                    );
                });
                if draft.quote == 3 {
                    ids.custom_quote =
                        body.text_input_styled(&draft.custom_quote, "One character", input);
                }
                body.row(row, |row| {
                    row.text("Line endings");
                    ids.newline = row.select(
                        ["LF · Unix", "CRLF · Windows", "CR · Classic Mac", "Other"],
                        Some(draft.newline),
                        "Line endings",
                        select,
                    );
                });
                if draft.newline == 3 {
                    ids.custom_newline = body.text_input_styled(
                        &draft.custom_newline,
                        "Line-ending characters",
                        input,
                    );
                }
            }
        },
        |footer| {
            ids.cancel = footer.tool_button("Cancel");
            ids.export =
                footer.tool_button_styled("Export…", theme.colors.accent, theme.colors.on_accent);
        },
    );
    ui.set_text_area_read_only(error_text, true);
    ui.set_table_check_cells(
        ids.table,
        draft
            .tables
            .iter()
            .map(|(_, _, selected)| vec![Some(*selected), None, None])
            .collect(),
    );
    ui.set_table_selected_col(ids.table, Some(0));
    ui.set_table_selected_row(ids.table, draft.selected);
    ui.set_table_scroll_offset(ids.table, draft.scroll.0, draft.scroll.1);
    ui.set_accessible_name(
        ids.table,
        "Tables and views to export; Space toggles inclusion",
    );
    ui.set_button_enabled(
        ids.export,
        draft.query.is_some() || draft.tables.iter().any(|(_, _, selected)| *selected),
    );
    ui.set_dialog_open(ids.dialog, true);
    if let Some(outputs) = &draft.pending {
        let mut decision = ModalStyle::from_theme(theme);
        decision.width = (width - 80.0).min(580.0);
        decision.height = 260.0;
        decision.close_on_backdrop = false;
        let mut message = TextAreaStyle::from_theme(theme);
        message.width = decision.width - 48.0;
        message.height = 140.0;
        let names = outputs
            .iter()
            .filter(|output| output.path.exists())
            .map(|output| output.path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut text = next_gui_ui::WidgetId(0);
        ids.confirm = ui.dialog(
            "Replace existing export files?",
            decision,
            |body| {
                text = body.text_area(names, "", message);
            },
            |footer| {
                ids.keep = footer.tool_button_styled(
                    "Cancel",
                    theme.colors.accent,
                    theme.colors.on_accent,
                );
                ids.replace = footer.tool_button("Replace");
            },
        );
        ui.set_text_area_read_only(text, true);
        ui.set_dialog_open(ids.confirm, true);
        ui.set_focus(Some(ids.keep));
    }
    ids
}

impl crate::Demo {
    pub(crate) fn open_data_export(&mut self, json: bool) {
        if self.state.conn.is_none() || self.state.sql_busy {
            self.set_error("Open an idle database before exporting");
            return;
        }
        if let Err(error) = self.state.guard_csv_idle() {
            self.set_error(error);
            return;
        }
        self.clear_error();
        self.state.export_dialog = Some(Draft::new(&self.state, json));
        self.rebuild = true;
    }
    pub(crate) fn open_query_export(&mut self, json: bool) {
        let query = self
            .state
            .sql_result
            .as_ref()
            .ok_or("Execute SQL before exporting results".into())
            .and_then(QuerySource::new);
        match query {
            Ok(query) => {
                self.open_data_export(json);
                if let Some(draft) = &mut self.state.export_dialog {
                    draft.query = Some(query);
                    draft.tables.clear();
                }
            }
            Err(error) => self.set_error(error),
        }
    }
    pub(crate) fn snapshot_data_export(&mut self) {
        let ids = self.ids.data_export;
        if ids.dialog.0 == 0 {
            return;
        }
        let Some(draft) = &mut self.state.export_dialog else {
            return;
        };
        if draft.pending.is_some() {
            return;
        }
        if let Some(value) = self.ui.is_checked(ids.header) {
            draft.options.header = value;
        }
        if let Some(value) = self.ui.is_checked(ids.pretty) {
            draft.options.pretty = value;
        }
        let previous = (draft.separator, draft.quote, draft.newline);
        if let Some(value) = self.ui.select_selected(ids.separator) {
            draft.separator = value;
        }
        if let Some(value) = self.ui.select_selected(ids.quote) {
            draft.quote = value;
        }
        if let Some(value) = self.ui.select_selected(ids.newline) {
            draft.newline = value;
        }
        for (id, value) in [
            (ids.custom_separator, &mut draft.custom_separator),
            (ids.custom_quote, &mut draft.custom_quote),
            (ids.custom_newline, &mut draft.custom_newline),
        ] {
            if let Some(text) = self.ui.text_input_value(id) {
                *value = text.into();
            }
        }
        if let Some(scroll) = self.ui.table_scroll_offset(ids.table) {
            draft.scroll = scroll;
        }
        draft.selected = self.ui.table_selected_row(ids.table);
        if previous != (draft.separator, draft.quote, draft.newline) {
            self.rebuild = true;
        }
    }
    pub(crate) fn handle_data_export_dismiss(&mut self) {
        let ids = self.ids.data_export;
        match self.ui.take_dialog_dismiss() {
            Some(id) if id == ids.confirm => {
                if let Some(draft) = &mut self.state.export_dialog {
                    draft.pending = None;
                }
                self.rebuild = true;
            }
            Some(id) if id == ids.dialog => {
                self.state.export_dialog = None;
                self.rebuild = true;
            }
            _ => {}
        }
    }
    fn perform_data_export(&mut self, outputs: Vec<Output>, replace: bool) {
        let draft = self.state.export_dialog.as_ref().unwrap();
        let options = match draft.options() {
            Ok(options) => options,
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        let conn = self.state.conn.as_ref().unwrap();
        let result = if let Some(query) = &draft.query {
            export_query(conn, query, &outputs[0].path, draft.json, &options, replace)
        } else {
            export(conn, &outputs, draft.json, &options, replace)
        };
        match result {
            Ok(()) => {
                self.state.prefs.data_export = options;
                self.state.export_dialog = None;
                self.state.status_msg = format!("Exported {} file(s)", outputs.len());
                self.clear_error();
                if !cfg!(test) && std::env::var_os("NEXT_GUI_DB_BROWSER_QA_TAB").is_none() {
                    if let Err(error) = self.state.prefs.save() {
                        self.set_error(format!(
                            "Files exported; saving export preferences failed: {error}"
                        ));
                    }
                }
            }
            Err(error) => {
                if let Some(draft) = &mut self.state.export_dialog {
                    draft.pending = None;
                }
                self.set_error(error);
            }
        }
        self.rebuild = true;
    }
    pub(crate) fn handle_data_export_button(&mut self, id: next_gui_ui::WidgetId) {
        self.snapshot_data_export();
        let ids = self.ids.data_export;
        if id == ids.cancel {
            self.state.export_dialog = None;
            self.rebuild = true;
            return;
        }
        if id == ids.keep {
            if let Some(draft) = &mut self.state.export_dialog {
                draft.pending = None;
            }
            self.rebuild = true;
            return;
        }
        if id == ids.replace {
            if let Some(outputs) = self
                .state
                .export_dialog
                .as_mut()
                .and_then(|draft| draft.pending.take())
            {
                self.perform_data_export(outputs, true);
            }
            return;
        }
        let draft = self.state.export_dialog.as_mut().unwrap();
        if id == ids.toggle {
            let selected = !draft.tables.iter().all(|(_, _, selected)| *selected);
            for (_, _, value) in &mut draft.tables {
                *value = selected;
            }
            self.rebuild = true;
            return;
        }
        if id != ids.export {
            return;
        }
        if let Err(error) = draft.options() {
            self.set_error(error);
            self.rebuild = true;
            return;
        }
        let selected: Vec<_> = if draft.query.is_some() {
            vec![("main".into(), "query".into())]
        } else {
            draft
                .tables
                .iter()
                .filter(|(_, _, selected)| *selected)
                .map(|(schema, table, _)| (schema.clone(), table.clone()))
                .collect()
        };
        if selected.is_empty() {
            self.set_error("Select at least one table or view");
            return;
        }
        let extension = if draft.json { "json" } else { "csv" };
        let outputs = if selected.len() == 1 {
            let (schema, table) = &selected[0];
            let Some(path) = rfd::FileDialog::new()
                .set_title("Export data")
                .set_file_name(filename(schema, table, draft.json))
                .add_filter(extension, &[extension])
                .save_file()
            else {
                return;
            };
            vec![Output {
                schema: schema.clone(),
                table: table.clone(),
                path,
            }]
        } else {
            let Some(directory) = rfd::FileDialog::new()
                .set_title("Choose export directory")
                .pick_folder()
            else {
                return;
            };
            selected
                .into_iter()
                .map(|(schema, table)| {
                    let path = directory.join(filename(&schema, &table, draft.json));
                    Output {
                        schema,
                        table,
                        path,
                    }
                })
                .collect()
        };
        if outputs.iter().any(|output| output.path.exists()) {
            draft.pending = Some(outputs);
            self.rebuild = true;
        } else {
            self.perform_data_export(outputs, false);
        }
    }
}
