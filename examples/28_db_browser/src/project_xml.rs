//! DB4S 3.13.1 project interchange. Unknown elements/attributes survive save.

use std::borrow::Cow;
use std::path::Path;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use rusqlite::Connection;

use crate::{db, project::ProjectFile, state::BrowseTabState};

#[derive(Clone, Debug)]
enum Content {
    Element(Element),
    Text(String),
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
    content: Vec<Content>,
}

impl Element {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            content: Vec::new(),
        }
    }

    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn set(&mut self, name: &str, value: impl ToString) {
        if let Some((_, old)) = self.attrs.iter_mut().find(|(key, _)| key == name) {
            *old = value.to_string();
        } else {
            self.attrs.push((name.into(), value.to_string()));
        }
    }

    fn children(&self) -> impl Iterator<Item = &Element> {
        self.content.iter().filter_map(|c| match c {
            Content::Element(e) => Some(e),
            _ => None,
        })
    }

    fn child(&self, name: &str) -> Option<&Element> {
        self.children().find(|e| e.name == name)
    }

    fn ensure(&mut self, name: &str) -> &mut Element {
        let index = self
            .content
            .iter()
            .position(|c| matches!(c, Content::Element(e) if e.name == name))
            .unwrap_or_else(|| {
                self.content.push(Content::Element(Element::new(name)));
                self.content.len() - 1
            });
        match &mut self.content[index] {
            Content::Element(e) => e,
            _ => unreachable!(),
        }
    }

    fn push(&mut self, element: Element) {
        self.content.push(Content::Element(element));
    }

    fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                Content::Text(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }

    fn retain_elements(&mut self, keep: impl Fn(&Element) -> bool) {
        self.content.retain(|c| match c {
            Content::Element(e) => keep(e),
            _ => false,
        });
    }

    fn write(&self, writer: &mut Writer<Vec<u8>>) -> Result<(), String> {
        let mut start = BytesStart::new(&self.name);
        for (key, value) in &self.attrs {
            legal_xml(value)?;
            let escaped = quick_xml::escape::escape(value)
                .replace('\n', "&#10;")
                .replace('\r', "&#13;")
                .replace('\t', "&#9;");
            start.push_attribute(quick_xml::events::attributes::Attribute {
                key: quick_xml::name::QName(key.as_bytes()),
                value: Cow::Owned(escaped.into_bytes()),
            });
        }
        if self.content.is_empty() {
            writer
                .write_event(Event::Empty(start))
                .map_err(|e| e.to_string())?;
        } else {
            writer
                .write_event(Event::Start(start))
                .map_err(|e| e.to_string())?;
            for child in &self.content {
                match child {
                    Content::Element(e) => e.write(writer)?,
                    Content::Text(s) => {
                        legal_xml(s)?;
                        writer
                            .write_event(Event::Text(BytesText::new(s)))
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            writer
                .write_event(Event::End(BytesEnd::new(&self.name)))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn legal_xml(text: &str) -> Result<(), String> {
    if text.chars().any(|c| !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        Err("Project text contains a character XML 1.0 cannot represent; save as JSON instead".into())
    } else { Ok(()) }
}

fn parse(text: &str) -> Result<Element, String> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("Project exceeds the 16 MiB XML limit".into());
    }
    legal_xml(text)?;
    let mut reader = Reader::from_str(text.trim_start_matches('\u{feff}'));
    let mut stack = vec![Element::new("document")];
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("Invalid project XML: {e}"))?;
        match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                if stack.len() > 64 {
                    return Err("Project XML is nested too deeply".into());
                }
                let name = std::str::from_utf8(start.name().as_ref())
                    .map_err(|e| e.to_string())?
                    .to_string();
                let mut element = Element::new(&name);
                for attribute in start.attributes() {
                    let attribute = attribute.map_err(|e| e.to_string())?;
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|e| e.to_string())?
                        .into_owned();
                    legal_xml(&value)?;
                    element.attrs.push((
                        std::str::from_utf8(attribute.key.as_ref())
                            .map_err(|e| e.to_string())?
                            .into(),
                        value,
                    ));
                }
                if matches!(event, Event::Empty(_)) {
                    stack.last_mut().unwrap().push(element);
                } else {
                    stack.push(element);
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err("Unexpected XML closing tag".into());
                }
                let element = stack.pop().unwrap();
                stack.last_mut().unwrap().push(element);
            }
            Event::Text(s) => stack.last_mut().unwrap().content.push(Content::Text(
                s.xml10_content().map_err(|e| e.to_string())?.into_owned(),
            )),
            Event::CData(s) => stack.last_mut().unwrap().content.push(Content::Text(
                s.xml10_content().map_err(|e| e.to_string())?.into_owned(),
            )),
            Event::GeneralRef(r) => {
                let value = if let Some(ch) = r.resolve_char_ref().map_err(|e| e.to_string())? {
                    ch.to_string()
                } else {
                    quick_xml::escape::resolve_predefined_entity(
                        &r.decode().map_err(|e| e.to_string())?,
                    )
                    .ok_or("Unsupported XML entity")?
                    .to_string()
                };
                legal_xml(&value)?;
                stack.last_mut().unwrap().content.push(Content::Text(value));
            }
            Event::DocType(_) => {
                return Err("Project XML cannot contain a DTD or external entities".into())
            }
            Event::Eof => break,
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
        }
    }
    if stack.len() != 1 {
        return Err("Unclosed project XML element".into());
    }
    let document = stack.pop().unwrap();
    if document.children().count() != 1
        || document
            .content
            .iter()
            .any(|c| matches!(c, Content::Text(s) if !s.trim().is_empty()))
    {
        return Err("Project XML must contain one root element".into());
    }
    let root = document.children().next().unwrap().clone();
    if root.name != "sqlb_project" {
        return Err("Expected sqlb_project XML root".into());
    }
    Ok(root)
}

fn render(root: &Element) -> Result<String, String> {
    let mut writer = Writer::new(Vec::new());
    root.write(&mut writer)?;
    String::from_utf8(writer.into_inner()).map_err(|e| e.to_string())
}

fn object_id(text: &str) -> Result<(&str, &str), String> {
    let (lengths, value) = text
        .split_once(':')
        .ok_or("Invalid DB4S table identifier")?;
    let (schema_len, table_len) = lengths
        .split_once(',')
        .ok_or("Invalid DB4S table identifier")?;
    let schema_len: usize = schema_len.parse().map_err(|_| "Invalid schema length")?;
    let table_len: usize = table_len.parse().map_err(|_| "Invalid table length")?;
    if schema_len.checked_add(table_len) != Some(value.len()) || !value.is_char_boundary(schema_len)
    {
        return Err("Invalid DB4S table identifier length".into());
    }
    Ok(value.split_at(schema_len))
}

fn relative_path(path: &str, project: &Path) -> String {
    let path = Path::new(path);
    if path.is_absolute() && path.parent() == project.parent() {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    } else {
        path.to_string_lossy().into_owned()
    }
}

pub fn decode(text: &str) -> Result<ProjectFile, String> {
    let root = parse(text)?;
    let mut project = if let Some(extension) = root.child("next_gui") {
        if extension.attr("version") != Some("1") {
            return Err("Unsupported next-gui project version".into());
        }
        serde_json::from_str(&extension.text())
            .map_err(|e| format!("Invalid next-gui project state: {e}"))?
    } else {
        ProjectFile::default()
    };
    let database = root.child("db").ok_or("Project has no database element")?;
    project.db_path = database
        .attr("path")
        .ok_or("Project database is missing path")?
        .into();
    project.read_only = database
        .attr("readonly")
        .is_some_and(|s| s != "0" && !s.is_empty());
    if let Some(window) = root.child("window") {
        if let Some(tabs) = window.child("main_tabs") {
            let current: usize = tabs
                .attr("current")
                .unwrap_or("0")
                .parse()
                .map_err(|_| "Invalid main tab index")?;
            project.main_tab = match tabs
                .attr("open")
                .unwrap_or("structure browser pragmas query")
                .split_whitespace()
                .nth(current)
            {
                Some("browser") => 1,
                Some("pragmas") => 2,
                Some("query") => 3,
                _ => 0,
            };
        }
    }
    if let Some(sql) = root.child("tab_sql") {
        project.sql_tabs.clear();
        project.sql_files.clear();
        for tab in sql.children().filter(|e| e.name == "sql") {
            project.sql_tabs.push(tab.text());
            project
                .sql_files
                .push(tab.attr("filename").map(|s| crate::state::SqlFile {
                    path: s.into(),
                    saved: String::new(),
                }));
        }
        project.sql_tab_idx = sql
            .child("current_tab")
            .and_then(|e| e.attr("id"))
            .unwrap_or("0")
            .parse()
            .map_err(|_| "Invalid SQL tab index")?;
    }
    if let Some(browse) = root.child("tab_browse") {
        let mut tabs = Vec::new();
        for (index, tab) in browse.children().filter(|e| e.name == "table").enumerate() {
            let (schema, name) = object_id(tab.attr("table").unwrap_or("4,0:main"))?;
            // shortcut: attached-schema browse settings are preserved, apply after schema browsing is implemented.
            if schema != "main" {
                continue;
            }
            let mut b = project
                .browse_tabs
                .get(index)
                .filter(|b| b.table.as_deref() == Some(name))
                .or_else(|| project.browse_tabs.iter().find(|b| b.table.as_deref() == Some(name)))
                .cloned()
                .unwrap_or_else(|| BrowseTabState::new((!name.is_empty()).then(|| name.into())));
            b.title = tab.attr("title").unwrap_or(name).into();
            tabs.push(b);
        }
        if !tabs.is_empty() {
            project.browse_tabs = tabs;
        }
    }
    project.db4s_xml = Some(text.into());
    Ok(project)
}

pub fn encode(project: &ProjectFile, path: &Path) -> Result<String, String> {
    let mut root = project
        .db4s_xml
        .as_deref()
        .map(parse)
        .transpose()?
        .unwrap_or_else(|| Element::new("sqlb_project"));
    let database = root.ensure("db");
    database.set("path", relative_path(&project.db_path, path));
    database.set("readonly", u8::from(project.read_only));
    let tabs = root.ensure("window").ensure("main_tabs");
    tabs.set("open", "structure browser pragmas query");
    tabs.set("current", project.main_tab.min(3));
    let sql = root.ensure("tab_sql");
    let old_tabs: Vec<_> = sql
        .children()
        .filter(|e| e.name == "sql")
        .cloned()
        .collect();
    sql.retain_elements(|e| e.name != "sql" && e.name != "current_tab");
    for (index, text) in project.sql_tabs.iter().enumerate() {
        let mut tab = old_tabs
            .get(index)
            .cloned()
            .unwrap_or_else(|| Element::new("sql"));
        if tab.attr("name").is_none() {
            tab.set("name", format!("SQL {}", index + 1));
        }
        tab.attrs.retain(|(key, _)| key != "filename");
        tab.content.clear();
        if let Some(Some(file)) = project.sql_files.get(index) {
            tab.set("filename", file.path.to_string_lossy());
            tab.content.push(Content::Text(format!(
                "-- Reference to file \"{}\" --",
                file.path.display()
            )));
        } else {
            tab.content.push(Content::Text(text.clone()));
        }
        sql.push(tab);
    }
    let mut current = Element::new("current_tab");
    current.set(
        "id",
        project
            .sql_tab_idx
            .min(project.sql_tabs.len().saturating_sub(1)),
    );
    sql.push(current);
    let browse = root.ensure("tab_browse");
    let old_browse: Vec<_> = browse
        .children()
        .filter(|e| e.name == "table")
        .cloned()
        .collect();
    let next_dock = old_browse
        .iter()
        .filter_map(|e| e.attr("dock_id")?.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    browse.retain_elements(|e| {
        e.name != "table"
            || e.attr("table")
                .and_then(|s| object_id(s).ok())
                .is_some_and(|(schema, _)| schema != "main")
    });
    for (index, b) in project.browse_tabs.iter().enumerate() {
        let name = b.table.as_deref().unwrap_or("");
        let mut tab = old_browse
            .iter()
            .find(|e| e.attr("table").and_then(|s| object_id(s).ok()) == Some(("main", name)))
            .cloned()
            .unwrap_or_else(|| Element::new("table"));
        tab.set("table", format!("4,{}:main{name}", name.len()));
        tab.set("title", &b.title);
        tab.set("custom_title", u8::from(b.title != name));
        if tab.attr("dock_id").is_none() {
            tab.set("dock_id", next_dock + index);
        }
        browse.push(tab);
    }
    let mut extension = project.clone();
    extension.db4s_xml = None;
    for (index, file) in extension.sql_files.iter().enumerate() {
        if let Some(file) = file {
            if let Some(text) = extension.sql_tabs.get_mut(index) {
                *text = file.saved.clone();
            }
        }
    }
    let data = root.ensure("next_gui");
    data.set("version", 1);
    data.content = vec![Content::Text(
        serde_json::to_string(&extension).map_err(|e| e.to_string())?,
    )];
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n",
        render(&root)?
    ))
}

const CONNECTION_PRAGMAS: &[&str] = &[
    "foreign_keys",
    "case_sensitive_like",
    "temp_store",
    "wal_autocheckpoint",
    "synchronous",
];

pub fn capture_connection(project: &mut ProjectFile, conn: &Connection) -> Result<(), String> {
    let mut root = project
        .db4s_xml
        .as_deref()
        .map(parse)
        .transpose()?
        .unwrap_or_else(|| Element::new("sqlb_project"));
    let database = root.ensure("db");
    for key in CONNECTION_PRAGMAS {
        let value = if *key == "case_sensitive_like" {
            conn.query_row("SELECT NOT ('a' LIKE 'A')", [], |row| row.get::<_, u32>(0))
                .map_err(|e| e.to_string())?
                .to_string()
        } else {
            db::read_pragma(conn, key)?
        };
        if !value.is_empty() {
            database.set(key, value);
        }
    }
    let attached = root.ensure("attached");
    attached.retain_elements(|e| e.name != "db");
    let mut stmt = conn
        .prepare("PRAGMA database_list")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (schema, path) = row.map_err(|e| e.to_string())?;
        if schema == "main" || schema == "temp" {
            continue;
        }
        if path.is_empty() {
            return Err(format!(
                "Attached in-memory schema {schema} cannot be persisted in a project"
            ));
        }
        let mut db = Element::new("db");
        db.set("schema", schema);
        db.set("path", path);
        attached.push(db);
    }
    let settings = root.ensure("tab_browse").ensure("browse_table_settings");
    let mut table_options = std::collections::BTreeMap::new();
    for tab in &project.browse_tabs {
        table_options.extend(tab.table_options.clone());
    }
    for tab in &project.browse_tabs {
        if let Some(name) = &tab.table {
            table_options.insert(name.clone(), tab.options());
        }
    }
    for (name, options) in table_options {
        let columns = db::table_column_names(conn, &name)?;
        if columns.is_empty() { continue; }
        let index = settings.content.iter().position(|c| matches!(c, Content::Element(e) if e.name == "table" && e.attr("schema").unwrap_or("main") == "main" && e.attr("name") == Some(name.as_str()))).unwrap_or_else(|| {
            let mut table = Element::new("table");
            table.set("schema", "main"); table.set("name", &name);
            settings.push(table);
            settings.content.len() - 1
        });
        let Content::Element(table) = &mut settings.content[index] else {
            unreachable!()
        };
        if let Some(count) = options.frozen_cols {
            table.set("freeze_columns", count.min(columns.len()));
        } else if project.browse_tabs.iter().any(|tab| tab.table.as_deref() == Some(name.as_str())) {
            table.set("freeze_columns", project.frozen_cols.min(columns.len()));
        }
        let sort = table.ensure("sort");
        let old_ordering = sort.children().filter(|e| e.name == "column").filter_map(|column| {
            column.attr("index").and_then(|i| i.parse::<usize>().ok()).and_then(|i| columns.get(i))
                .map(|name| db::OrderBy { column: name.clone(), descending: column.attr("mode") == Some("1") })
        }).collect::<Vec<_>>();
        let ordering = options.ordering;
        if old_ordering != ordering {
            sort.retain_elements(|e| e.name != "column");
            for order in &ordering {
                if let Some(index) = columns.iter().position(|c| c == &order.column) {
                    let mut column = Element::new("column");
                    column.set("index", index);
                    column.set("mode", u8::from(order.descending));
                    sort.push(column);
                }
            }
        }
        let widths = table.ensure("column_widths");
        widths.retain_elements(|e| e.name != "column");
        for (index, width) in options.column_widths.iter().enumerate().take(columns.len()) {
            let mut column = Element::new("column");
            column.set("index", index);
            column.set("value", width.round().max(32.0) as u32);
            widths.push(column);
        }
        let filters = table.ensure("filter_values");
        filters.retain_elements(|e| e.name != "column");
        for (index, value) in options.col_filters.iter().enumerate().take(columns.len()).filter(|(_, value)| !value.is_empty()) {
            let mut column = Element::new("column");
            column.set("index", index);
            column.set("value", value);
            filters.push(column);
        }
        let global = table.ensure("global_filter");
        global.retain_elements(|e| e.name != "filter");
        for value in &options.global_filters {
            let mut filter = Element::new("filter");
            filter.set("value", value);
            global.push(filter);
        }
    }
    project.db4s_xml = Some(render(&root)?);
    Ok(())
}

/// Map column indices only after the database schema is available.
fn restore_table_options(
    tab: &mut BrowseTabState,
    table: &Element,
    columns: &[String],
) -> Result<(), String> {
    if let Some(count) = table.attr("freeze_columns") {
        tab.frozen_cols = Some(
            count.parse::<usize>()
                .map_err(|_| "Invalid project frozen column count")?
                .min(columns.len()),
        );
    }
    if let Some(sort) = table.child("sort") {
        tab.set_ordering(
            sort.children()
                .filter(|e| e.name == "column")
                .filter_map(|column| {
                    column
                        .attr("index")
                        .and_then(|i| i.parse::<usize>().ok())
                        .and_then(|i| columns.get(i))
                        .map(|name| db::OrderBy {
                            column: name.clone(),
                            descending: column.attr("mode") == Some("1"),
                        })
                })
                .collect(),
        );
    }
    if let Some(widths) = table.child("column_widths") {
        tab.column_widths.resize(columns.len(), 140.0);
        for column in widths.children().filter(|e| e.name == "column") {
            if let Some(index) = column
                .attr("index")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|&i| i < columns.len())
            {
                let width = column
                    .attr("value")
                    .unwrap_or("140")
                    .parse::<f32>()
                    .map_err(|_| "Invalid project column width")?;
                if !width.is_finite() || width <= 0.0 {
                    return Err("Invalid project column width".into());
                }
                tab.column_widths[index] = width.clamp(32.0, 10000.0);
            }
        }
    }
    if let Some(filters) = table.child("filter_values") {
        tab.col_filters = vec![String::new(); columns.len()];
        for column in filters.children().filter(|e| e.name == "column") {
            let index = column
                .attr("index")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|&i| i < columns.len())
                .ok_or("Invalid project filter column index")?;
            tab.col_filters[index] = column.attr("value").unwrap_or("").into();
        }
    }
    if let Some(global) = table.child("global_filter") {
        tab.global_filters = global
            .children()
            .filter(|e| e.name == "filter")
            .map(|e| e.attr("value").unwrap_or("").into())
            .collect();
    }
    Ok(())
}

pub fn restore_browse(project: &mut ProjectFile, conn: &Connection) -> Result<(), String> {
    let Some(source) = &project.db4s_xml else {
        return Ok(());
    };
    let root = parse(source)?;
    let Some(settings) = root
        .child("tab_browse")
        .and_then(|e| e.child("browse_table_settings"))
    else {
        return Ok(());
    };
    for tab in &mut project.browse_tabs {
        for table in settings
            .children()
            .filter(|e| e.name == "table" && e.attr("schema").unwrap_or("main") == "main")
        {
            let Some(name) = table.attr("name") else {
                continue;
            };
            let columns = db::table_column_names(conn, name)?;
            if columns.is_empty() {
                continue;
            }
            let active = tab.table.as_deref() == Some(name);
            let mut stored = BrowseTabState::new(Some(name.into()));
            if active {
                stored.set_options(tab.options());
            } else if let Some(options) = tab.table_options.get(name) {
                stored.set_options(options.clone());
            }
            // Per-tab overrides take precedence over DB4S's shared table setting.
            let frozen_override = stored.frozen_cols;
            restore_table_options(&mut stored, table, &columns)?;
            if frozen_override.is_some() {
                stored.frozen_cols = frozen_override;
            }
            if active {
                tab.set_options(stored.options());
                project.frozen_cols = table
                    .attr("freeze_columns")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(project.frozen_cols)
                    .min(columns.len());
            }
            tab.table_options.insert(name.into(), stored.options());
        }
    }
    Ok(())
}

pub fn unapplied_settings(project: &ProjectFile) -> Result<Vec<&'static str>, String> {
    let Some(source) = &project.db4s_xml else {
        return Ok(Vec::new());
    };
    let root = parse(source)?;
    let mut notes = Vec::new();
    if let Some(browse) = root.child("tab_browse") {
        if browse
            .child("dock_state")
            .and_then(|e| e.attr("state"))
            .is_some_and(|s| !s.is_empty())
        {
            notes.push("Qt dock layout");
        }
        if browse.children().filter(|e| e.name == "table").any(|e| {
            e.attr("table")
                .and_then(|s| object_id(s).ok())
                .is_some_and(|(schema, _)| schema != "main")
        }) {
            notes.push("attached-schema browse tabs");
        }
        if let Some(settings) = browse.child("browse_table_settings") {
            for table in settings.children().filter(|e| e.name == "table") {
                if [
                    "hidden_columns",
                    "conditional_formats",
                    "row_id_formats",
                    "display_formats",
                    "plot_y_axes",
                ]
                .iter()
                .any(|key| {
                    table
                        .child(key)
                        .is_some_and(|e| e.children().next().is_some())
                })
                {
                    notes.push("advanced browse filters/formatting/plots");
                    break;
                }
            }
        }
    }
    Ok(notes)
}

pub fn restore_connection(
    project: &ProjectFile,
    conn: &Connection,
    path: &Path,
) -> Result<(), String> {
    let Some(source) = &project.db4s_xml else {
        return Ok(());
    };
    let root = parse(source)?;
    let mut pragmas = Vec::new();
    if let Some(database) = root.child("db") {
        for key in CONNECTION_PRAGMAS {
            if let Some(value) = database.attr(key).filter(|s| !s.is_empty()) {
                let number: u32 = value
                    .parse()
                    .map_err(|_| format!("Invalid project PRAGMA {key}: {value}"))?;
                let max = match *key {
                    "foreign_keys" | "case_sensitive_like" => 1,
                    "temp_store" => 2,
                    "synchronous" => 3,
                    _ => i32::MAX as u32,
                };
                if number > max {
                    return Err(format!("Invalid project PRAGMA {key}: {value}"));
                }
                let old: u32 = if *key == "case_sensitive_like" {
                    conn.query_row("SELECT NOT ('a' LIKE 'A')", [], |row| row.get(0))
                        .map_err(|e| e.to_string())?
                } else {
                    db::read_pragma(conn, key)?
                        .parse()
                        .map_err(|_| format!("Cannot read PRAGMA {key}"))?
                };
                if *key == "temp_store" && number != old {
                    let objects: u32 = conn
                        .query_row("SELECT count(*) FROM temp.sqlite_schema", [], |row| {
                            row.get(0)
                        })
                        .map_err(|e| e.to_string())?;
                    if objects != 0 {
                        return Err("Changing project temp_store would remove existing temporary objects; close the database first".into());
                    }
                }
                if old != number {
                    pragmas.push((*key, number, old));
                }
            }
        }
    }
    let mut stmt = conn
        .prepare("PRAGMA database_list")
        .map_err(|e| e.to_string())?;
    let existing: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(1)?, row.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    let mut attachments = Vec::new();
    let mut names = std::collections::HashSet::new();
    if let Some(attached) = root.child("attached") {
        for database in attached.children().filter(|e| e.name == "db") {
            let schema = database
                .attr("schema")
                .filter(|s| {
                    !s.is_empty()
                        && !s.eq_ignore_ascii_case("main")
                        && !s.eq_ignore_ascii_case("temp")
                })
                .ok_or("Invalid attached schema name")?;
            if !names.insert(schema.to_ascii_lowercase()) {
                return Err(format!("Duplicate attached schema {schema}"));
            }
            let file = Path::new(
                database
                    .attr("path")
                    .filter(|s| !s.is_empty())
                    .ok_or("Missing attached database path")?,
            );
            let file = if file.is_absolute() {
                file.to_path_buf()
            } else {
                path.parent().unwrap_or(Path::new(".")).join(file)
            };
            if !file.is_file() {
                return Err(format!(
                    "Attached database does not exist: {}",
                    file.display()
                ));
            }
            if let Some((_, old)) = existing
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(schema))
            {
                if std::fs::canonicalize(old).ok() != std::fs::canonicalize(&file).ok() {
                    return Err(format!(
                        "Schema {schema} is already attached to another file"
                    ));
                }
            } else {
                attachments.push((schema.to_string(), file));
            }
        }
    }
    let mut newly_attached = Vec::new();
    let result: Result<(), String> = (|| {
        for (key, value, _) in &pragmas {
            conn.pragma_update(None, key, value)
                .map_err(|e| e.to_string())?;
        }
        for (schema, file) in &attachments {
            db::attach_database_mode(conn, &file.to_string_lossy(), schema, project.read_only)?;
            newly_attached.push(schema);
        }
        Ok(())
    })();
    if let Err(mut error) = result {
        for schema in newly_attached.into_iter().rev() {
            if let Err(e) =
                conn.execute_batch(&format!("DETACH DATABASE {}", db::quote_ident(schema)))
            {
                error.push_str(&format!("; restore attachment: {e}"));
            }
        }
        for (key, _, old) in pragmas.into_iter().rev() {
            if let Err(e) = conn.pragma_update(None, key, old) {
                error.push_str(&format!("; restore {key}: {e}"));
            }
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Structure and attribute names follow upstream v3.13.1 MainWindow::saveProject.
    const PROJECT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sqlb_project future="retained">
 <db path="数据库.db" readonly="1" foreign_keys="1" temp_store="2" synchronous="1"/>
 <window><main_tabs open="query structure browser pragmas" current="0"/></window>
 <tab_structure><expanded_item id="2" parent="-1"/></tab_structure>
 <tab_browse><table title="表&#10;one" custom_title="1" dock_id="7" table="4,3:main表" unknown="kept"/>
 <dock_state state="00AABB"/>
 <browse_table_settings><table schema="main" name="表" freeze_columns="1">
 <sort><column index="1" mode="1"/></sort><column_widths><column index="0" value="175"/></column_widths>
 <filter_values><column index="0" value="&gt;=2"/></filter_values>
 <global_filter><filter value="東京"/><filter value="/row(?=x)/"/></global_filter>
 <hidden_columns><column index="1" value="1"/></hidden_columns>
 </table></browse_table_settings></tab_browse>
 <tab_sql><sql name="Unicode">SELECT '&lt;&amp;中文&#x1F600;';<![CDATA[
SELECT ']]&gt;';]]></sql><current_tab id="0"/></tab_sql>
 <unknown key="a&amp;b"><nested>keep &lt;this&gt;</nested></unknown>
</sqlb_project>"#;

    #[test]
    fn xml_roundtrip_keeps_unicode_sql_unknown_settings_and_attribute_whitespace() {
        let mut project = decode(PROJECT).unwrap();
        assert_eq!(project.db_path, "数据库.db");
        assert_eq!(project.main_tab, 3);
        assert!(project.read_only);
        assert_eq!(project.browse_tabs[0].table.as_deref(), Some("表"));
        assert_eq!(project.browse_tabs[0].title, "表\none");
        assert!(project.sql_tabs[0].contains("<&中文😀"));
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE \"表\"(id INTEGER, name TEXT)")
            .unwrap();
        restore_browse(&mut project, &conn).unwrap();
        assert_eq!(project.browse_tabs[0].order_col.as_deref(), Some("name"));
        assert!(project.browse_tabs[0].order_desc);
        assert_eq!(project.browse_tabs[0].column_widths[0], 175.0);
        assert_eq!(project.browse_tabs[0].col_filters, [">=2", ""]);
        assert_eq!(project.browse_tabs[0].global_filters, ["東京", "/row(?=x)/"]);
        project.browse_tabs[0].col_filters = vec!["".into(), "<>NULL".into()];
        project.browse_tabs[0].global_filters = vec!["東京".into()];
        capture_connection(&mut project, &conn).unwrap();
        let encoded = encode(&project, Path::new("demo.sqbpro")).unwrap();
        let mut restored = decode(&encoded).unwrap();
        restore_browse(&mut restored, &conn).unwrap();
        assert_eq!(restored.sql_tabs, project.sql_tabs);
        assert_eq!(restored.browse_tabs[0].title, "表\none");
        assert_eq!(restored.browse_tabs[0].col_filters, ["", "<>NULL"]);
        assert_eq!(restored.browse_tabs[0].global_filters, ["東京"]);
        let root = parse(&encoded).unwrap();
        assert_eq!(root.attr("future"), Some("retained"));
        assert_eq!(
            root.child("unknown")
                .unwrap()
                .child("nested")
                .unwrap()
                .text(),
            "keep <this>"
        );
        let browse = root.child("tab_browse").unwrap();
        assert_eq!(
            browse.child("dock_state").unwrap().attr("state"),
            Some("00AABB")
        );
        assert_eq!(browse.child("table").unwrap().attr("unknown"), Some("kept"));
        assert!(browse
            .child("browse_table_settings")
            .unwrap()
            .child("table")
            .unwrap()
            .child("hidden_columns")
            .is_some());
    }

    #[test]
    fn xml_rejects_malformed_entities_identifiers_and_unrepresentable_text() {
        for source in ["<sqlb_project/>", "<sqlb_project><db path='x'/>", "<sqlb_project><db path='x'/></wrong>",
            "<!DOCTYPE sqlb_project [<!ENTITY file SYSTEM 'file:///etc/passwd'>]><sqlb_project><db path='x'/>&file;</sqlb_project>",
            "<sqlb_project><db path='x'/><tab_sql><sql>&unknown;</sql></tab_sql></sqlb_project>",
            "<sqlb_project><db path='x'/><tab_sql><sql>&#0;</sql></tab_sql></sqlb_project>",
            "<sqlb_project><db path='x'/></sqlb_project><second/>"] {
            assert!(decode(source).is_err(), "{source}");
        }
        for value in [
            "",
            "4,3:mainx",
            "5,2:main表",
            "1,3:表x",
            "9999999999999999999999,0:main",
        ] {
            assert!(object_id(value).is_err(), "{value}");
        }
        let project = ProjectFile {
            sql_tabs: vec!["SELECT '\0';".into()],
            ..Default::default()
        };
        assert!(encode(&project, Path::new("demo.sqbpro")).is_err());
        assert_eq!(object_id("4,3:main表").unwrap(), ("main", "表"));
    }

    #[test]
    fn connection_preflight_and_failure_restore_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open_path(":memory:").unwrap();
        let before = db::read_pragma(&conn, "foreign_keys").unwrap();
        let source = "<sqlb_project><db path='' foreign_keys='1'/><attached><db schema='other' path='missing.db'/></attached></sqlb_project>";
        let project = decode(source).unwrap();
        assert!(restore_connection(&project, &conn, &dir.path().join("project.sqbpro")).is_err());
        assert_eq!(db::read_pragma(&conn, "foreign_keys").unwrap(), before);
        assert!(!dir.path().join("missing.db").exists());
        std::fs::write(dir.path().join("broken.db"), b"not a database").unwrap();
        let source = source.replace("missing.db", "broken.db");
        let project = decode(&source).unwrap();
        assert!(restore_connection(&project, &conn, &dir.path().join("project.sqbpro")).is_err());
        assert_eq!(db::read_pragma(&conn, "foreign_keys").unwrap(), before);
    }

    #[test]
    fn readonly_project_attaches_existing_unicode_path_readonly() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("库? #%.db");
        let db = db::open_path(file.to_str().unwrap()).unwrap();
        db.execute_batch("CREATE TABLE records(value); INSERT INTO records VALUES (7)")
            .unwrap();
        drop(db);
        let project = decode("<sqlb_project><db path='' readonly='1'/><attached><db schema='extra' path='库? #%.db'/></attached></sqlb_project>").unwrap();
        let conn = db::open_path(":memory:").unwrap();
        restore_connection(&project, &conn, &dir.path().join("project.sqbpro")).unwrap();
        assert_eq!(
            conn.query_row("SELECT value FROM extra.records", [], |row| row
                .get::<_, i32>(0))
                .unwrap(),
            7
        );
        assert!(conn
            .execute("UPDATE extra.records SET value=8", [])
            .is_err());
    }
}
