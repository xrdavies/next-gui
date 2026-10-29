use super::*;
use crate::{ui_build::mid, Demo};
use next_gui_platform::desktop::OpenRequest;
use next_gui_ui::{MenuItem, ModalAction};

pub struct ExternalCell {
    pub path: tempfile::TempPath,
    pub read_only: bool,
    revision: u64,
    source: Option<CellApplyConfirmation>,
    bytes: Vec<u8>,
}

pub(super) fn read_cell_file(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    // shortcut: bounded whole-cell editing; use a streaming binary editor for larger imports.
    const LIMIT: u64 = 16 * 1024 * 1024;
    if !std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("Select a regular cell data file".into());
    }
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("Select a regular cell data file".into());
    }
    if metadata.len() > LIMIT {
        return Err("Cell import exceeds the 16 MiB editor limit".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Cell import exceeds the 16 MiB editor limit".into());
    }
    Ok(bytes)
}

impl AppState {
    pub fn import_cell_link(&mut self, path: &Path) -> Result<(), String> {
        if self.edit_cell_read_only() {
            return Err("This cell is read-only".into());
        }
        if !path.is_file() {
            return Err("Select a regular file to link".into());
        }
        let path = std::path::absolute(path).map_err(|error| error.to_string())?;
        let text = path
            .to_str()
            .ok_or("The file path cannot be represented as UTF-8 text")?;
        self.edit_type = 3;
        self.edit_original = rusqlite::types::Value::Text(text.into());
        self.show_loaded_edit_value();
        self.status_msg = "File link loaded into cell draft; Apply to update the record".into();
        Ok(())
    }

    pub fn cell_open_target(&self) -> Result<(String, bool), String> {
        if self.edit_col.is_empty()
            || matches!(self.edit_mode, EditCellMode::Hex | EditCellMode::Image)
        {
            return Err("Select a text cell or text editor mode to open a link".into());
        }
        let text = self.edit_value.trim();
        if text.is_empty() || text.chars().any(char::is_control) {
            return Err("The cell does not contain a single URL or file path".into());
        }
        let uri = text.split_once(':').is_some_and(|(scheme, _)| {
            scheme
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && scheme
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"+.-".contains(&byte))
                && !(scheme.len() == 1
                    && text
                        .as_bytes()
                        .get(2)
                        .is_some_and(|byte| b"/\\".contains(byte)))
        });
        if uri {
            return Ok((text.into(), true));
        }
        let path = Path::new(text);
        let base = if self.path.is_empty() || self.path == ":memory:" {
            std::env::current_dir().map_err(|error| error.to_string())?
        } else {
            std::path::absolute(&self.path)
                .map_err(|error| error.to_string())?
                .parent()
                .ok_or("Database has no directory")?
                .to_path_buf()
        };
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        };
        Ok((
            path.to_str()
                .ok_or("The file path cannot be represented as UTF-8 text")?
                .into(),
            false,
        ))
    }

    fn external_cell_bytes(&self) -> Result<Vec<u8>, String> {
        use rusqlite::types::Value;
        if self.edit_col.is_empty()
            || (self.edit_type == 0 && self.edit_mode != EditCellMode::Evaluation)
        {
            return Err("Select a non-NULL cell first".into());
        }
        if self.edit_mode == EditCellMode::Hex {
            return Ok(self.edit_bytes.as_ref().clone());
        }
        let original = match &self.edit_original {
            Value::Text(text) => text.as_bytes().to_vec(),
            Value::Blob(bytes) => bytes.clone(),
            Value::Integer(value) => value.to_string().into_bytes(),
            Value::Real(value) => value.to_string().into_bytes(),
            Value::Null => Vec::new(),
        };
        if self.edit_mode == EditCellMode::Image {
            return Ok(original);
        }
        let text = &self.edit_value;
        if std::str::from_utf8(&original).is_err() && *text == String::from_utf8_lossy(&original) {
            return Err("Use Hex to open binary data without replacing invalid UTF-8".into());
        }
        let mut bytes = Vec::new();
        if self.edit_mode == EditCellMode::Text && original.starts_with(b"\xef\xbb\xbf") {
            bytes.extend_from_slice(b"\xef\xbb\xbf");
        }
        bytes.extend_from_slice(text.as_bytes());
        Ok(bytes)
    }

    pub fn begin_external_cell(&mut self) -> Result<PathBuf, String> {
        use std::io::Write;
        self.guard_background_idle()?;
        if self.external_cell.is_some() || self.confirm_open || self.overlay.is_some() {
            return Err("Finish the current dialog first".into());
        }
        let bytes = self.external_cell_bytes()?;
        let read_only = self.edit_cell_read_only();
        let source = if read_only {
            None
        } else {
            Some(self.cell_apply_confirmation()?)
        };
        let suffix = external_suffix(&bytes, self.edit_mode);
        let mut file = tempfile::Builder::new()
            .prefix("next-gui-cell-")
            .suffix(suffix)
            .tempfile()
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.flush().map_err(|error| error.to_string())?;
        let file = file.into_temp_path();
        let path = file.to_path_buf();
        self.external_cell = Some(ExternalCell {
            path: file,
            read_only,
            revision: self.edit_revision,
            source,
            bytes,
        });
        self.confirm_kind = ConfirmKind::CellExternal;
        self.confirm_open = true;
        Ok(path)
    }

    pub fn cancel_external_cell(&mut self) {
        self.external_cell = None;
        if self.confirm_kind == ConfirmKind::CellExternal {
            self.confirm_kind = ConfirmKind::None;
            self.confirm_open = false;
        }
    }

    pub fn load_external_cell(&mut self) -> Result<(), String> {
        let external = self
            .external_cell
            .as_ref()
            .ok_or("The external editing session has ended")?;
        if external.read_only || self.edit_cell_read_only() {
            return Err("This cell is read-only".into());
        }
        let revision = external.revision;
        let source = external.source.clone();
        let path = external.path.to_path_buf();
        if revision != self.edit_revision
            || source.as_ref() != Some(&self.cell_apply_confirmation()?)
            || self.external_cell.as_ref().unwrap().bytes != self.external_cell_bytes()?
        {
            return Err(
                "The cell changed; cancel this external session and open the current cell again"
                    .into(),
            );
        }
        if !std::fs::symlink_metadata(&path)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_file()
        {
            return Err("The external copy must remain a regular file".into());
        }
        let bytes = read_cell_file(&path)?;
        self.cancel_external_cell();
        self.load_cell_file_bytes(bytes);
        Ok(())
    }
}

fn external_suffix(bytes: &[u8], mode: EditCellMode) -> &'static str {
    if let Some((_, format)) = crate::cell_format::cell_image_format(bytes) {
        return match format {
            "PNG" => ".png",
            "JPEG" => ".jpg",
            _ => ".bmp",
        };
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return ".bin";
    };
    if text
        .chars()
        .any(|ch| ch.is_control() && !ch.is_whitespace())
    {
        return ".bin";
    }
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text.starts_with("<svg") {
        ".svg"
    } else if mode == EditCellMode::Xml || text.starts_with("<?xml") {
        ".xml"
    } else if mode == EditCellMode::Json || crate::cell_format::validate_json(text).is_ok() {
        ".json"
    } else {
        ".txt"
    }
}

impl Demo {
    pub(crate) fn open_cell_import_menu(&mut self) {
        if self.state.edit_cell_read_only() {
            return;
        }
        let bounds = self
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|node| self.ui.widget_from_node(node.id) == Some(self.ids.btn_cell_import))
            .map(|node| node.bounds);
        if let Some(bounds) = bounds {
            self.ui
                .set_accessible_name(self.ids.context_menu, "Import cell data");
            self.ui.open_context_menu(
                self.ids.context_menu,
                vec![
                    MenuItem::new(mid::CELL_IMPORT, "Import from File…"),
                    MenuItem::new(mid::CELL_IMPORT_LINK, "Import as Link…"),
                ],
                bounds.x,
                bounds.y + bounds.height,
            );
        }
    }

    pub(crate) fn handle_cell_import_action(&mut self, item: u64) -> bool {
        if !matches!(item, mid::CELL_IMPORT | mid::CELL_IMPORT_LINK) {
            return false;
        }
        self.ui.close_popup();
        if self.state.edit_cell_read_only() {
            self.set_error("This cell is read-only");
            return true;
        }
        if let Some(path) = crate::file_dlg::pick_open_cell() {
            let result = if item == mid::CELL_IMPORT_LINK {
                self.state.import_cell_link(&path)
            } else {
                self.state.import_cell_file(&path)
            };
            match result {
                Ok(()) => self.clear_error(),
                Err(error) => self.set_error(error),
            }
            self.rebuild = true;
        }
        true
    }

    pub(crate) fn open_cell_link(&mut self) {
        let result = self.state.cell_open_target().and_then(|(target, uri)| {
            let request = if uri {
                OpenRequest::url(&target)
            } else {
                OpenRequest::path(Path::new(&target))
            }
            .map_err(|error| error.to_string())?;
            self.pending_opens.push((request, None));
            self.state.status_msg = format!("Open requested: {target}");
            Ok(())
        });
        if let Err(error) = result {
            self.set_error(error);
        }
        self.rebuild = true;
    }

    pub(crate) fn open_external_cell(&mut self) {
        let result =
            self.state
                .begin_external_cell()
                .and_then(|path| match OpenRequest::path(&path) {
                    Ok(request) => {
                        self.pending_opens.push((request, Some(path)));
                        self.clear_error();
                        Ok(())
                    }
                    Err(error) => {
                        self.state.cancel_external_cell();
                        Err(error.to_string())
                    }
                });
        if let Err(error) = result {
            self.set_error(error);
        }
        self.rebuild = true;
    }

    pub(crate) fn resolve_external_cell(&mut self, action: ModalAction) {
        if action != ModalAction::Primary
            || self
                .state
                .external_cell
                .as_ref()
                .is_none_or(|external| external.read_only)
        {
            self.state.cancel_external_cell();
            self.state.status_msg = "External copy closed; cell draft retained".into();
        } else {
            match self.state.load_external_cell().and_then(|()| {
                if self.state.prepare_cell_format_confirmation()? {
                    Ok(())
                } else {
                    self.state.apply_cell_draft()
                }
            }) {
                Ok(()) => self.clear_error(),
                Err(error) => {
                    if self.state.external_cell.is_some() {
                        self.state.confirm_kind = ConfirmKind::CellExternal;
                        self.state.confirm_open = true;
                    }
                    self.set_error(error);
                }
            }
        }
        self.rebuild = true;
        self.do_rebuild();
        if !self.state.confirm_open {
            let editor = if self.ids.edit_value.0 != 0 {
                self.ids.edit_value
            } else {
                self.ids.edit_image
            };
            self.ui.set_focus(Some(editor));
            self.ui.sync(&mut self.scene);
        }
    }

    pub(crate) fn poll_desktop_opens(&mut self) {
        for (request, external) in std::mem::take(&mut self.pending_opens) {
            match request.poll() {
                None => self.pending_opens.push((request, external)),
                Some(Ok(())) => {}
                Some(Err(error)) => {
                    if let Some(path) = external {
                        if self.state.external_cell.as_ref().is_none_or(|cell| path != cell.path.to_path_buf()) {
                            continue;
                        }
                        self.state.cancel_external_cell();
                    }
                    self.set_error(error.to_string());
                    self.rebuild = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppCallbacks, PresentBackend, Theme, UiKey};
    use rusqlite::types::Value;

    fn editor(value: Value, mode: EditCellMode) -> Demo {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE content(id INTEGER PRIMARY KEY, value); INSERT INTO content VALUES(1,NULL),(2,'other');").unwrap();
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute("UPDATE content SET value=?1 WHERE id=1", [value])
            .unwrap();
        demo.state.write_changes().unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.prefs.auto_switch_mode = false;
        demo.state.prefs.indent_compact = false;
        demo.state
            .switch_browse_table_in("main", "content")
            .unwrap();
        demo.state.goto_browse_record(1).unwrap();
        demo.state.browse_mut().sel_col = Some(1);
        demo.state.sync_edit_cell_from_selection();
        demo.state.set_edit_mode(mode).unwrap();
        demo.do_rebuild();
        demo
    }

    fn stored(demo: &Demo) -> Value {
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .query_row("SELECT value FROM content WHERE id=1", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn cell_links_store_paths_without_reading_data_and_resolve_against_database() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("--中文 ' ; $ example.txt");
        std::fs::write(&file, b"the file contents must not become the cell").unwrap();
        let mut demo = editor(Value::Blob(vec![0, 255]), EditCellMode::Hex);
        demo.state.import_cell_link(&file).unwrap();
        assert_eq!(demo.state.edit_type, 3);
        assert_eq!(
            demo.state.edited_cell_value().unwrap(),
            Value::Text(file.to_str().unwrap().into())
        );
        assert_eq!(stored(&demo), Value::Blob(vec![0, 255]));
        assert!(!demo.state.dirty);
        assert!(demo.state.has_cell_draft());
        demo.state.apply_cell_draft().unwrap();
        assert_eq!(stored(&demo), Value::Text(file.to_str().unwrap().into()));
        demo.state.set_edit_mode(EditCellMode::Text).unwrap();
        demo.state.path = directory
            .path()
            .join("data.sqlite")
            .to_str()
            .unwrap()
            .into();
        demo.state.edit_value = "sub dir/report.txt".into();
        assert_eq!(
            demo.state.cell_open_target().unwrap(),
            (
                directory
                    .path()
                    .join("sub dir/report.txt")
                    .to_str()
                    .unwrap()
                    .into(),
                false
            )
        );
        demo.state.edit_value = "  https://example.invalid/中文?a=1&b=';$()  ".into();
        assert_eq!(
            demo.state.cell_open_target().unwrap(),
            ("https://example.invalid/中文?a=1&b=';$()".into(), true)
        );
        for text in ["", "\n", "https://example.invalid/a\nother"] {
            demo.state.edit_value = text.into();
            assert!(demo.state.cell_open_target().is_err());
        }
        demo.state.tab = MainTab::Pragmas;
        let revision = demo.state.edit_revision;
        assert!(demo.state.import_cell_link(&file).is_err());
        assert_eq!(demo.state.edit_revision, revision);
        assert!(demo.state.import_cell_link(directory.path()).is_err());
    }

    #[test]
    fn external_cell_suffixes_bytes_readonly_and_failure_guards_are_lossless() {
        for (value, mode, suffix, expected) in [
            (
                Value::Text("\u{feff}A中文".into()),
                EditCellMode::Text,
                "txt",
                "\u{feff}A中文".as_bytes().to_vec(),
            ),
            (
                Value::Text("{\"a\":1}".into()),
                EditCellMode::Json,
                "json",
                b"{\"a\":1}".to_vec(),
            ),
            (
                Value::Text("<root/>".into()),
                EditCellMode::Xml,
                "xml",
                b"<root/>".to_vec(),
            ),
            (
                Value::Text("<svg/>".into()),
                EditCellMode::Text,
                "svg",
                b"<svg/>".to_vec(),
            ),
            (
                Value::Blob(vec![0, 255, 65]),
                EditCellMode::Hex,
                "bin",
                vec![0, 255, 65],
            ),
            (
                Value::Text("1 + 2".into()),
                EditCellMode::Evaluation,
                "txt",
                b"1 + 2".to_vec(),
            ),
        ] {
            let mut demo = editor(value.clone(), mode);
            let revision = demo.state.edit_revision;
            let path = demo.state.begin_external_cell().unwrap();
            assert_eq!(path.extension().unwrap(), suffix);
            assert_eq!(std::fs::read(&path).unwrap(), expected);
            assert!(!demo.can_close());
            assert_eq!(stored(&demo), value);
            assert!(!demo.state.dirty);
            assert!(demo.state.begin_external_cell().is_err());
            demo.state.edit_revision = revision.wrapping_add(1);
            assert!(demo
                .state
                .load_external_cell()
                .unwrap_err()
                .contains("changed"));
            assert!(path.exists());
            assert_eq!(stored(&demo), value);
            demo.state.cancel_external_cell();
            assert!(!path.exists());
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([20, 40, 80, 255]),
        ))
        .write_to(&mut png, image::ImageOutputFormat::Png)
        .unwrap();
        let png = png.into_inner();
        let mut picture = editor(Value::Blob(png.clone()), EditCellMode::Image);
        let path = picture.state.begin_external_cell().unwrap();
        assert_eq!(path.extension().unwrap(), "png");
        assert_eq!(std::fs::read(&path).unwrap(), png);
        picture.state.cancel_external_cell();
        assert!(!path.exists());
        let mut demo = editor(Value::Blob(vec![0xff, 0xfe]), EditCellMode::Text);
        assert!(demo
            .state
            .begin_external_cell()
            .unwrap_err()
            .contains("Hex"));
        demo.state.set_edit_mode(EditCellMode::Hex).unwrap();
        let path = demo.state.begin_external_cell().unwrap();
        demo.state.edit_bytes = Arc::new(vec![1, 2]);
        assert!(demo
            .state
            .load_external_cell()
            .unwrap_err()
            .contains("changed"));
        demo.state.cancel_external_cell();
        assert!(!path.exists());
        demo.state.discard_cell_draft();
        let path = demo.state.begin_external_cell().unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(16 * 1024 * 1024 + 1)
            .unwrap();
        assert!(demo
            .state
            .load_external_cell()
            .unwrap_err()
            .contains("16 MiB"));
        assert!(demo.state.external_cell.is_some());
        assert_eq!(stored(&demo), Value::Blob(vec![0xff, 0xfe]));
        std::fs::remove_file(&path).unwrap();
        assert!(demo.state.load_external_cell().is_err());
        assert!(demo.state.external_cell.is_some());
        std::fs::write(&path, [9u8, 0, 255]).unwrap();
        demo.state.load_external_cell().unwrap();
        assert!(!path.exists());
        assert_eq!(demo.state.edit_bytes.as_slice(), [9, 0, 255]);
        assert!(demo.state.has_cell_draft());
        demo.state.apply_cell_draft().unwrap();
        assert_eq!(stored(&demo), Value::Blob(vec![9, 0, 255]));
        demo.state.tab = MainTab::Pragmas;
        demo.do_rebuild();
        let path = demo.state.begin_external_cell().unwrap();
        std::fs::write(&path, b"must not replace the stored readonly value").unwrap();
        assert!(demo
            .state
            .load_external_cell()
            .unwrap_err()
            .contains("read-only"));
        demo.resolve_external_cell(ModalAction::Primary);
        assert!(!path.exists());
        assert_eq!(stored(&demo), Value::Blob(vec![9, 0, 255]));
        let directory = tempfile::tempdir().unwrap();
        assert!(read_cell_file(directory.path()).is_err());
        let mut json = editor(Value::Text("{\"a\":1}".into()), EditCellMode::Json);
        let path = json.state.begin_external_cell().unwrap();
        std::fs::write(&path, b"{bad}").unwrap();
        json.resolve_external_cell(ModalAction::Primary);
        assert!(!path.exists());
        assert_eq!(json.state.confirm_kind, ConfirmKind::CellFormat);
        assert_eq!(stored(&json), Value::Text("{\"a\":1}".into()));
        assert_eq!(json.state.edit_value, "{bad}");
        json.resolve_cell_format(ModalAction::Primary);
        assert!(json.state.has_cell_draft());
        assert!(!json.state.dirty);
    }

    #[test]
    fn external_cells_reject_symlinks_and_keep_reloaded_drafts_on_database_errors() {
        let mut demo = editor(Value::Text("original".into()), EditCellMode::Text);
        let path = demo.state.begin_external_cell().unwrap();
        #[cfg(unix)]
        {
            let directory = tempfile::tempdir().unwrap();
            let target = directory.path().join("target");
            std::fs::write(&target, b"untouched").unwrap();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(&target, &path).unwrap();
            assert!(demo
                .state
                .load_external_cell()
                .unwrap_err()
                .contains("regular file"));
            demo.state.cancel_external_cell();
            assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
            assert!(!path.exists());
        }
        #[cfg(not(unix))]
        demo.state.cancel_external_cell();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TRIGGER deny_value BEFORE UPDATE ON content BEGIN SELECT RAISE(IGNORE); END;").unwrap();
        let path = demo.state.begin_external_cell().unwrap();
        std::fs::write(&path, b"changed").unwrap();
        demo.resolve_external_cell(ModalAction::Primary);
        assert!(!path.exists());
        assert!(demo.state.error_msg.as_ref().unwrap().contains("ignored"));
        assert_eq!(stored(&demo), Value::Text("original".into()));
        assert_eq!(demo.state.edit_value, "changed");
        assert!(demo.state.has_cell_draft());
        assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
    }

    #[cfg(unix)]
    #[test]
    fn desktop_open_errors_only_affect_the_current_external_copy() {
        let mut demo = editor(Value::Text("original".into()), EditCellMode::Text);
        let closed = demo.state.begin_external_cell().unwrap();
        demo.state.cancel_external_cell();
        let current = demo.state.begin_external_cell().unwrap();
        for external in [Some(closed), Some(current.clone()), None] {
            demo.state.status_msg = "Current activity".into();
            demo.state.error_msg = None;
            demo.rebuild = false;
            // NUL makes spawn fail before launching any desktop application.
            let request = OpenRequest::path(Path::new("\0")).unwrap();
            let stale = external.as_ref().is_some_and(|path| path != &current);
            demo.pending_opens.push((request, external));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !demo.pending_opens.is_empty() {
                demo.poll_desktop_opens();
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if stale {
                assert_eq!(demo.state.status_msg, "Current activity");
                assert!(demo.state.error_msg.is_none());
                assert!(!demo.rebuild);
                assert_eq!(demo.state.external_cell.as_ref().unwrap().path.to_path_buf(), current);
                assert!(current.is_file());
            } else {
                assert!(demo.state.error_msg.is_some());
                assert!(demo.rebuild);
                assert!(demo.state.external_cell.is_none());
                assert!(!current.exists());
            }
        }
    }

    #[test]
    fn cell_external_modals_and_import_menu_retain_drafts_history_and_explicit_apply() {
        let folder = std::env::var("NEXT_GUI_CELL_ACTIONS_QA_OUT")
            .ok()
            .map(PathBuf::from);
        if let Some(folder) = &folder {
            std::fs::create_dir_all(folder).unwrap();
        }
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height, size) in [(1400, 860, 13.0), (800, 600, 18.0)] {
                let mut demo = editor(Value::Text("original".into()), EditCellMode::Text);
                demo.state.dock_focus = DockKind::Plot;
                demo.state.body_ratio = 0.45;
                demo.state.dock_ratio = 0.62;
                demo.state.sync_current_plot().unwrap();
                demo.state.plot.check(0, 1, true);
                demo.win_w = width;
                demo.win_h = height;
                demo.state.prefs.font_size = size;
                demo.theme = crate::ui_build::browser_theme(theme, size);
                demo.do_rebuild();
                demo.ui.set_focus(Some(demo.ids.edit_value));
                demo.ui.key_input(UiKey::SelectAll);
                demo.ui.paste_into_focused_text("draft");
                demo.snapshot_inputs();
                demo.open_cell_import_menu();
                demo.ui.sync(&mut demo.scene);
                assert_eq!(
                    demo.ui.accessible_name(demo.ids.context_menu).as_deref(),
                    Some("Import cell data")
                );
                if let Some(folder) = &folder {
                    crate::plot_export::capture(
                        &mut demo,
                        folder,
                        &format!("import-menu-{name}-{width}"),
                    );
                }
                demo.ui.key_input(UiKey::End);
                demo.ui.key_input(UiKey::Enter);
                assert_eq!(
                    demo.ui.take_menu_action(),
                    Some((demo.ids.context_menu, mid::CELL_IMPORT_LINK))
                );
                demo.ui.close_popup();
                let revision = demo.state.edit_revision;
                let path = demo.state.begin_external_cell().unwrap();
                demo.do_rebuild();
                assert_eq!(std::fs::read(&path).unwrap(), b"draft");
                assert!(demo.ui.modal_state(demo.ids.modal).is_some());
                if let Some(folder) = &folder {
                    crate::plot_export::capture(
                        &mut demo,
                        folder,
                        &format!("external-modal-{name}-{width}"),
                    );
                }
                demo.resolve_external_cell(ModalAction::Secondary);
                assert!(!path.exists());
                assert_eq!(demo.state.edit_value, "draft");
                assert_eq!(demo.state.edit_revision, revision);
                assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
                assert!(demo.ui.key_input(UiKey::Undo));
                demo.snapshot_inputs();
                assert_ne!(demo.state.edit_value, "draft");
                assert!(demo.ui.key_input(UiKey::Redo));
                demo.snapshot_inputs();
                assert_eq!(demo.state.edit_value, "draft");
                let path = demo.state.begin_external_cell().unwrap();
                std::fs::write(&path, "saved externally 中文").unwrap();
                demo.do_rebuild();
                demo.resolve_external_cell(ModalAction::Primary);
                assert!(!path.exists());
                assert_eq!(stored(&demo), Value::Text("saved externally 中文".into()));
                assert!(!demo.state.has_cell_draft());
                assert!(demo.state.dirty);
                if let Some(folder) = &folder {
                    crate::plot_export::capture(
                        &mut demo,
                        folder,
                        &format!("external-applied-{name}-{width}"),
                    );
                }
                demo.state.write_changes().unwrap();
                demo.state.tab = MainTab::Pragmas;
                demo.do_rebuild();
                let path = demo.state.begin_external_cell().unwrap();
                demo.do_rebuild();
                if let Some(folder) = &folder {
                    crate::plot_export::capture(
                        &mut demo,
                        folder,
                        &format!("external-readonly-{name}-{width}"),
                    );
                }
                demo.resolve_external_cell(ModalAction::Primary);
                assert!(!path.exists());
                assert_eq!(stored(&demo), Value::Text("saved externally 中文".into()));
            }
        }
    }
}
