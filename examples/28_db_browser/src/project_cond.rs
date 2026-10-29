//! Native DB4S conditional-format maps, resolved after attaching the project databases.
use super::*;
use crate::state::{CondFormatRule, NativeCondFormat};

fn model_columns(conn: &Connection, schema: &str, table: &str) -> Result<Vec<String>, String> {
    let mut columns = vec![db::implicit_rowid_column_in(conn, schema, table)?.unwrap_or_default()];
    columns.extend(db::table_column_names_in(conn, schema, table)?);
    Ok(columns)
}

fn rgb(text: &str) -> Result<u32, String> {
    let hex = text
        .strip_prefix('#')
        .ok_or("Project format color must be #RRGGBB")?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Project format color must be #RRGGBB or #AARRGGBB".into());
    }
    // shortcut: retain native alpha in XML; add per-cell alpha colors for translucent project styles.
    u32::from_str_radix(&hex[hex.len() - 6..], 16).map_err(|error| error.to_string())
}

fn condition(rule: &CondFormatRule) -> Result<String, String> {
    let quoted = || rule.value.replace("\\E", "\\E\\\\E\\Q");
    Ok(match rule.op {
        0 => format!("/\\A\\Q{}\\E\\z/", quoted()),
        1 => format!("/\\A(?!\\Q{}\\E\\z)[\\s\\S]*\\z/", quoted()),
        2 => format!("/\\Q{}\\E/", quoted()),
        // shortcut: native export uses SQLite numeric comparisons; the extension preserves legacy f64 edges.
        3 | 4 => format!(
            "{}{}",
            if rule.op == 3 { '>' } else { '<' },
            rule.value.trim()
        ),
        5 => rule.value.clone(),
        _ => return Err("Unsupported project conditional-format operator".into()),
    })
}

fn format_element(rule: &CondFormatRule) -> Result<Element, String> {
    crate::cond_style::Draft::from(&rule.style).validate()?;
    if rule.color_rgb > 0xFFFFFF || rule.style.alignment > 3 {
        return Err("Invalid project conditional-format style".into());
    }
    let mut element = if let Some(source) = &rule.native_format {
        let root = parse(&format!("<sqlb_project>{}</sqlb_project>", source.xml))?;
        if root.children().count() != 1 {
            return Err("Invalid native format metadata".into());
        }
        root.child("format")
            .ok_or("Invalid native format metadata")?
            .clone()
    } else {
        Element::new("format")
    };
    element.set("condition", condition(rule)?);
    for (key, value) in [
        ("background", rule.color_rgb),
        ("foreground", rule.style.foreground_rgb.unwrap_or(0x101018)),
    ] {
        if element.attr(key).and_then(|text| rgb(text).ok()) != Some(value) {
            element.set(key, format!("#{value:06x}"));
        }
    }
    if let Some(font) = crate::cond_font::encode(&rule.style) {
        element.set("font", font);
    } else {
        element.attrs.retain(|(key, _)| key != "font");
    }
    element.set("align", rule.style.alignment);
    Ok(element)
}

pub(super) fn restore(
    project: &mut ProjectFile,
    conn: &Connection,
    root: &Element,
) -> Result<(), String> {
    let Some(settings) = root
        .child("tab_browse")
        .and_then(|browse| browse.child("browse_table_settings"))
    else {
        return Ok(());
    };
    let databases = db::database_names(conn)?;
    let mut restored = Vec::new();
    for table in settings.children().filter(|table| table.name == "table") {
        let (schema, Some(name)) = (table.attr("schema").unwrap_or("main"), table.attr("name"))
        else {
            continue;
        };
        let Some(formats) = table.child("conditional_formats") else {
            continue;
        };
        if root.child("next_gui").is_some() && formats.attr("next_gui_managed") == Some("1") {
            continue;
        }
        if !databases.iter().any(|database| database == schema) {
            continue;
        }
        let columns = model_columns(conn, schema, name)?;
        if columns.len() == 1 {
            continue;
        }
        for column in formats.children().filter(|column| column.name == "column") {
            let Some(index) = setting_column_index(column, &columns, 0) else {
                continue;
            };
            let Some(name_column) = columns.get(index).filter(|name| !name.is_empty()) else {
                continue;
            };
            for format in column.children().filter(|format| format.name == "format") {
                let mut style = format
                    .attr("font")
                    .map(crate::cond_font::decode)
                    .unwrap_or_default();
                style.foreground_rgb = format
                    .attr("foreground")
                    .filter(|value| !value.is_empty())
                    .map(rgb)
                    .transpose()?;
                style.alignment = format
                    .attr("align")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| "Invalid project format alignment")?;
                if style.alignment > 3 {
                    return Err("Invalid project format alignment".into());
                }
                restored.push(CondFormatRule {
                    schema: schema.into(),
                    table: Some(name.into()),
                    column: name_column.clone(),
                    op: 5,
                    value: format.attr("condition").unwrap_or_default().into(),
                    color_rgb: rgb(format
                        .attr("background")
                        .ok_or("Project format has no background color")?)?,
                    style,
                    native_format: Some(NativeCondFormat {
                        column: index,
                        xml: render(format)?,
                    }),
                });
            }
        }
    }
    // Preserve old local priorities and native duplicates, without duplicating a repeated restore.
    let mut present = std::collections::HashMap::new();
    let key = |rule: &CondFormatRule| {
        (
            rule.schema.clone(),
            rule.table.clone(),
            rule.column.clone(),
            rule.native_format.clone(),
        )
    };
    for rule in &project.cond_rules {
        *present.entry(key(rule)).or_insert(0usize) += 1;
    }
    let mut required = std::collections::HashMap::new();
    for rule in restored {
        let key = key(&rule);
        let count = required.entry(key.clone()).or_insert(0usize);
        *count += 1;
        if present.get(&key).copied().unwrap_or(0) < *count {
            project.cond_rules.push(rule);
        }
    }
    Ok(())
}

pub(super) fn capture(
    project: &ProjectFile,
    conn: &Connection,
    root: &mut Element,
) -> Result<(), String> {
    let settings = root.ensure("tab_browse").ensure("browse_table_settings");
    let mut sources = std::collections::BTreeSet::new();
    for table in settings
        .children()
        .filter(|table| table.name == "table" && table.child("conditional_formats").is_some())
    {
        if let Some(name) = table.attr("name") {
            sources.insert((
                table.attr("schema").unwrap_or("main").to_string(),
                name.to_string(),
            ));
        }
    }
    for rule in &project.cond_rules {
        if let Some(table) = &rule.table {
            sources.insert((rule.schema.clone(), table.clone()));
        }
    }
    let databases = db::database_names(conn)?;
    for (schema, name) in sources {
        if !databases.contains(&schema) {
            continue;
        }
        let columns = model_columns(conn, &schema, &name)?;
        if columns.len() == 1 {
            continue;
        }
        let index = settings.content.iter().position(|content| matches!(content, Content::Element(table) if table.name == "table" && table.attr("schema").unwrap_or("main") == schema && table.attr("name") == Some(name.as_str()))).unwrap_or_else(|| {
            let mut table = Element::new("table"); table.set("schema", &schema); table.set("name", &name);
            settings.push(table); settings.content.len()-1
        });
        let Content::Element(table) = &mut settings.content[index] else {
            unreachable!()
        };
        let formats = table.ensure("conditional_formats");
        formats.set("next_gui_managed", 1);
        for (index, name_column) in columns
            .iter()
            .enumerate()
            .filter(|(_, name)| !name.is_empty())
        {
            let rules: Vec<_> = project
                .cond_rules
                .iter()
                .filter(|rule| {
                    rule.schema == schema
                        && rule.table.as_ref() == Some(&name)
                        && rule.column.eq_ignore_ascii_case(name_column)
                })
                .collect();
            let previous = formats.content.iter().position(|content| matches!(content,Content::Element(column) if column.name == "column" && setting_column_index(column,&columns,0) == Some(index)));
            if rules.is_empty() && previous.is_none() {
                continue;
            }
            let position = previous.unwrap_or_else(|| {
                formats.push(Element::new("column"));
                formats.content.len() - 1
            });
            for content in &mut formats.content {
                if let Content::Element(column) = content {
                    if column.name == "column"
                        && setting_column_index(column, &columns, 0) == Some(index)
                    {
                        column.retain_elements(|element| element.name != "format");
                    }
                }
            }
            let Content::Element(column) = &mut formats.content[position] else {
                unreachable!()
            };
            column.set("index", index);
            column.set("next_gui_name", name_column);
            column.retain_elements(|element| element.name != "format");
            for rule in rules {
                column.push(format_element(rule)?);
            }
        }
    }
    Ok(())
}

pub(super) fn unapplied(project: &ProjectFile, table: &Element, root: &Element) -> bool {
    let schema = table.attr("schema").unwrap_or("main");
    let name = table.attr("name");
    table.child("conditional_formats").is_some_and(|formats| {
        let managed =
            root.child("next_gui").is_some() && formats.attr("next_gui_managed") == Some("1");
        formats
            .children()
            .filter(|column| column.name == "column")
            .any(|column| {
                let index = column
                    .attr("index")
                    .and_then(|text| text.parse::<usize>().ok());
                column
                    .children()
                    .filter(|format| format.name == "format")
                    .any(|format| {
                        render(format).is_ok_and(|text| {
                            !project.cond_rules.iter().any(|rule| {
                                rule.schema == schema
                                    && rule.table.as_deref() == name
                                    && (rule.native_format.as_ref().is_some_and(|native| {
                                        Some(native.column) == index && native.xml == text
                                    }) || (managed
                                        && column.attr("next_gui_name")
                                            == Some(rule.column.as_str())
                                        && format_element(rule)
                                            .and_then(|element| render(&element))
                                            .is_ok_and(|current| current == text)))
                            })
                        })
                    })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_conditions_round_trip_names_priority_fonts_metadata_and_clear() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE customers(id INTEGER PRIMARY KEY,name TEXT,revenue REAL);INSERT INTO customers VALUES(1,'alpha 中文',1.5),(2,'z',2.0),(3,'beta',3.0);ATTACH ':memory:' AS aux;CREATE TABLE aux.customers(name TEXT,id INTEGER);").unwrap();
        let source = r##"<sqlb_project><db path=':memory:'/><window><main_tabs open='structure browser pragmas query' current='1'/></window>
<tab_browse><table table='4,9:maincustomers'/><table table='3,9:auxcustomers'/><browse_table_settings>
<table schema='main' name='customers' show_row_id='0'><conditional_formats future='keep'>
<column index='2' extra='keep'><format condition='' background='#235263' foreground='#f8d56b' font='Menlo,18.5,-1,5,63,2,1,1,1,0,Oblique' align='1' future='keep'><future> untouched </future></format></column>
<column index='2'><format condition='=z' background='#416fa8' foreground='#eaf4ff' font='Menlo,-1,25,5,50,0,0,0,1,0' align='2'/></column>
<column index='0'><format condition='>2' background='#e67e22' foreground='#101018'/></column>
<column index='99'><format condition='future' background='#abcdef' foreground='#101018'/></column>
</conditional_formats></table><table schema='aux' name='customers'><conditional_formats><column index='1'><format condition='a%' background='#288976' foreground='#102b22'/></column></conditional_formats></table>
</browse_table_settings></tab_browse><tab_sql><sql name='SQL'>  SELECT ' keep spacing ';
 </sql></tab_sql><future> before <nested/> after </future></sqlb_project>"##;
        let duplicated = source.replace("<column index='0'>", "<column index='2'><format condition='=z' background='#416fa8' foreground='#eaf4ff' font='Menlo,-1,25,5,50,0,0,0,1,0' align='2'/></column><column index='0'>");
        let mut duplicates = decode(&duplicated).unwrap();
        for _ in 0..2 {
            restore_browse(&mut duplicates, &conn).unwrap();
            assert_eq!(duplicates.cond_rules.len(), 5);
        }
        let unknown_duplicate = source.replace("<format condition='future' background='#abcdef' foreground='#101018'/>",
            "<format condition='=z' background='#416fa8' foreground='#eaf4ff' font='Menlo,-1,25,5,50,0,0,0,1,0' align='2'/>");
        let mut unknown = decode(&unknown_duplicate).unwrap();
        restore_browse(&mut unknown, &conn).unwrap();
        assert_eq!(unknown.cond_rules.len(), 4);
        assert!(unapplied_settings(&unknown)
            .unwrap()
            .contains(&"advanced browse filters/formatting"));
        let mut project = decode(source).unwrap();
        restore_browse(&mut project, &conn).unwrap();
        assert_eq!(project.cond_rules.len(), 4);
        assert_eq!(project.cond_rules[0].column, "name");
        assert_eq!(
            project.cond_rules[2].column,
            db::implicit_rowid_column_in(&conn, "main", "customers")
                .unwrap()
                .unwrap()
        );
        assert_eq!(project.cond_rules[3].schema, "aux");
        assert_eq!(project.cond_rules[3].column, "name");
        assert_eq!(
            project.cond_rules[0].style.qfont.as_deref(),
            Some("Menlo,18.5,-1,5,63,2,1,1,1,0,Oblique")
        );
        assert!(unapplied_settings(&project)
            .unwrap()
            .contains(&"advanced browse filters/formatting"));
        restore_browse(&mut project, &conn).unwrap();
        assert_eq!(project.cond_rules.len(), 4);
        let mut known_root = parse(project.db4s_xml.as_ref().unwrap()).unwrap();
        known_root
            .ensure("tab_browse")
            .ensure("browse_table_settings")
            .ensure("table")
            .ensure("conditional_formats")
            .retain_elements(|column| column.attr("index") != Some("99"));
        let mut known = project.clone();
        known.db4s_xml = Some(render(&known_root).unwrap());
        assert!(!unapplied_settings(&known)
            .unwrap()
            .contains(&"advanced browse filters/formatting"));
        let original_font = project.cond_rules[0].style.qfont.clone();
        let mut draft = crate::cond_style::Draft::from(&project.cond_rules[0].style);
        draft.foreground = "#F5F0DC".into();
        project.cond_rules[0].style = draft.validate().unwrap();
        assert_eq!(project.cond_rules[0].style.qfont, original_font);
        project.cond_rules.swap(0, 1);
        let mut root = parse(project.db4s_xml.as_ref().unwrap()).unwrap();
        capture(&project, &conn, &mut root).unwrap();
        project.db4s_xml = Some(render(&root).unwrap());
        let path = Path::new("/tmp/native-conditional-project.sqbpro");
        let encoded = encode(&project, path).unwrap();
        assert!(unapplied_settings(&decode(&encoded).unwrap())
            .unwrap()
            .contains(&"advanced browse filters/formatting"));
        assert!(
            encoded.contains("future=\"keep\"") && encoded.contains("<future> untouched </future>")
        );
        assert!(encoded.contains(" before <nested/> after "));
        for native_only in [false, true] {
            let mut root = parse(&encoded).unwrap();
            if native_only {
                root.retain_elements(|element| element.name != "next_gui");
            }
            let mut restored = decode(&render(&root).unwrap()).unwrap();
            restore_browse(&mut restored, &conn).unwrap();
            assert_eq!(restored.cond_rules.len(), 4);
            assert_eq!(
                restored
                    .cond_rules
                    .iter()
                    .filter(|rule| rule.schema == "main" && rule.column == "name")
                    .map(|rule| rule.value.as_str())
                    .collect::<Vec<_>>(),
                vec!["=z", ""]
            );
            assert_eq!(restored.sql_tabs, project.sql_tabs);
            assert_eq!(
                restored
                    .cond_rules
                    .iter()
                    .find(|rule| rule.value.is_empty())
                    .unwrap()
                    .style
                    .qfont,
                original_font
            );
        }
        if let Ok(out) = std::env::var("NEXT_GUI_PROJECT_COND_QA_OUT") {
            std::fs::create_dir_all(&out).unwrap();
            let database = Path::new(&out).join("fixture.sqlite");
            assert!(
                !database.exists(),
                "QA output must be a fresh owned directory"
            );
            db::copy_database_file(&conn, &database).unwrap();
            let mut exported = project.clone();
            exported.db_path = database.to_string_lossy().into_owned();
            exported.read_only = true;
            exported.browse_tabs.retain(|tab| tab.schema == "main");
            exported.cond_rules.retain(|rule| rule.schema == "main");
            let mut root = parse(exported.db4s_xml.as_ref().unwrap()).unwrap();
            root.ensure("tab_browse")
                .ensure("browse_table_settings")
                .retain_elements(|table| table.attr("schema").unwrap_or("main") == "main");
            exported.db4s_xml = Some(render(&root).unwrap());
            std::fs::write(
                Path::new(&out).join("exported.sqbpro"),
                encode(&exported, &Path::new(&out).join("exported.sqbpro")).unwrap(),
            )
            .unwrap();
        }
        project
            .cond_rules
            .retain(|rule| rule.schema != "main" || rule.column != "name");
        let mut root = parse(project.db4s_xml.as_ref().unwrap()).unwrap();
        capture(&project, &conn, &mut root).unwrap();
        root.retain_elements(|element| element.name != "next_gui");
        let mut cleared = decode(&render(&root).unwrap()).unwrap();
        restore_browse(&mut cleared, &conn).unwrap();
        assert_eq!(cleared.cond_rules.len(), 2);
        assert!(cleared
            .cond_rules
            .iter()
            .all(|rule| rule.column != "name" || rule.schema == "aux"));
        assert!(render(&root).unwrap().contains("index=\"99\""));
        let original = project.cond_rules.clone();
        for bad in [
            source.replace("#235263", "invalid"),
            source.replace("align='1'", "align='4'"),
        ] {
            let mut invalid = decode(&bad).unwrap();
            invalid.cond_rules = original.clone();
            assert!(restore_browse(&mut invalid, &conn).is_err());
            assert_eq!(invalid.cond_rules, original);
        }
    }

    #[test]
    fn legacy_literal_conditions_export_as_case_sensitive_regular_expressions() {
        let mut state = crate::state::AppState::empty();
        crate::cond_format::prepare_qa(&mut state);
        let mut rule = state.cond_rules[0].clone();
        for value in ["", "a%_~", "a/b", "line\nnext", "x\\Ey", "a'b", "中文"] {
            rule.value = value.into();
            for op in 0..=2 {
                rule.op = op;
                let filter = condition(&rule).unwrap();
                crate::cond_match::validate(
                    state.conn.as_ref().unwrap(),
                    &filter,
                    &state.prefs.filter_escape,
                )
                .unwrap();
                let sql = crate::filters::condition(&filter, &state.prefs.filter_escape).unwrap();
                for candidate in [
                    value.to_string(),
                    format!("prefix{value}suffix"),
                    "other".into(),
                ] {
                    let actual: bool = state
                        .conn
                        .as_ref()
                        .unwrap()
                        .query_row(&format!("SELECT ?1 {sql}"), [&candidate], |row| row.get(0))
                        .unwrap();
                    assert_eq!(
                        actual,
                        match op {
                            0 => candidate == value,
                            1 => candidate != value,
                            _ => candidate.contains(value),
                        },
                        "{filter} {candidate:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn opening_native_conditional_projects_applies_formats_and_keeps_the_session_on_invalid_styles()
    {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("fixture.sqlite");
        let conn = Connection::open(&database).unwrap();
        conn.execute_batch(
            "CREATE TABLE t(id INTEGER PRIMARY KEY,name TEXT);INSERT INTO t VALUES(1,'native');",
        )
        .unwrap();
        drop(conn);
        let before = std::fs::read(&database).unwrap();
        let source = r##"<sqlb_project><db path='fixture.sqlite' readonly='1'/><tab_browse><table table='4,1:maint'/><browse_table_settings><table schema='main' name='t'><conditional_formats><column index='2'><format condition='' background='#235263' foreground='#f8d56b' font='Menlo,18.5,-1,5,63,2,1,1,1,0' align='1'/></column></conditional_formats></table></browse_table_settings></tab_browse></sqlb_project>"##;
        let path = dir.path().join("native.sqbpro");
        std::fs::write(&path, source).unwrap();
        let mut state = crate::state::AppState::empty();
        state.open_project(&path).unwrap();
        assert_eq!(state.cond_rules.len(), 1);
        assert_eq!(state.cond_rules[0].column, "name");
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][1], "native");
        assert!(!state
            .status_msg
            .contains("advanced browse filters/formatting"));
        let original = ProjectFile::from_state(&state);
        for bad in [
            source.replace("#235263", "bad-color"),
            source.replace("align='1'", "align='4'"),
        ] {
            let invalid = dir.path().join("invalid.sqbpro");
            std::fs::write(&invalid, bad).unwrap();
            assert!(state.open_project(&invalid).is_err());
            assert_eq!(state.path, original.db_path);
            assert_eq!(state.cond_rules, original.cond_rules);
            assert_eq!(state.browse().result.as_ref().unwrap().rows[0][1], "native");
            assert_eq!(state.sql_tabs, original.sql_tabs);
        }
        let roundtrip = dir.path().join("saved.sqbpro");
        state.save_project(&roundtrip).unwrap();
        state.open_project(&roundtrip).unwrap();
        assert_eq!(state.cond_rules, original.cond_rules);
        assert!(!state
            .status_msg
            .contains("advanced browse filters/formatting"));
        assert_eq!(std::fs::read(&database).unwrap(), before);
    }
}
