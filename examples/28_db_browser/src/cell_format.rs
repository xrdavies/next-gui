use roxmltree::{Document, ParsingOptions};

pub fn xml_document(text: &str) -> Result<Document<'_>, String> {
    // shortcut: bounded whole-cell XML; upgrade to streaming validation for larger cell editors.
    if text.len() > 16 * 1024 * 1024 { return Err("XML exceeds the 16 MiB editor limit".into()); }
    Document::parse_with_options(text, ParsingOptions { allow_dtd: true, nodes_limit: 250_000, entity_resolver: None })
        .map_err(|error| format!("Invalid XML: {error}"))
}

pub fn indent_xml(text: &str) -> Result<String, String> {
    let document = xml_document(text)?;
    let mut styles = vec![(false, false, 0usize); document.descendants().count()];
    let mut insertions = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        let (in_mixed, inherited_space, depth) = node.parent_element()
            .map(|parent| { let (mixed, space, depth) = styles[parent.id().get_usize()]; (mixed, space, depth + 1) })
            .unwrap_or((false, false, 0));
        if depth > 128 { return Err("XML nesting exceeds the indentation limit".into()); }
        let range = node.range();
        let mixed = in_mixed || node.children().any(|child| child.is_text()
            || child.range().start < range.start || child.range().end > range.end);
        let preserve = match node.attribute((roxmltree::NS_XML_URI, "space")) {
            Some("preserve") => true,
            Some("default") => false,
            _ => inherited_space,
        };
        styles[node.id().get_usize()] = (mixed, preserve, depth);
        if mixed || preserve || !node.has_children() { continue; }
        for child in node.children() {
            insertions.push((child.range().start, format!("\n{}", "    ".repeat(depth + 1))));
        }
        if let Some(end) = text[range.clone()].rfind("</") {
            insertions.push((range.start + end, format!("\n{}", "    ".repeat(depth))));
        }
    }
    insertions.sort_by_key(|(offset, _)| *offset);
    let size = insertions.iter().try_fold(text.len(), |size, (_, added)| size.checked_add(added.len()))
        .filter(|size| *size <= 32 * 1024 * 1024).ok_or("Indented XML exceeds the editor limit")?;
    let mut out = String::with_capacity(size);
    let mut previous = 0;
    for (offset, added) in insertions {
        out.push_str(&text[previous..offset]); out.push_str(&added); previous = offset;
    }
    out.push_str(&text[previous..]);
    Ok(out)
}

pub fn evaluate(conn: &rusqlite::Connection, expression: &str) -> Result<rusqlite::types::Value, String> {
    // The editor computes a scalar; its SQL must never execute a statement tail.
    let sql = format!("SELECT {expression}");
    let mut statement = crate::db::prepare_single_statement(conn, &sql)?;
    if !statement.readonly() || statement.column_count() != 1 {
        return Err("Enter one read-only SQL expression".into());
    }
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    let row = rows.next().map_err(|error| error.to_string())?.ok_or("Expression returned no value")?;
    let value = row.get_ref(0).map_err(|error| error.to_string())?;
    // rusqlite's Value conversion panics on invalid UTF-8 TEXT.
    if let rusqlite::types::ValueRef::Text(bytes) = value {
        return std::str::from_utf8(bytes).map(|text| rusqlite::types::Value::Text(text.into()))
            .map_err(|_| "Expression returned invalid UTF-8 TEXT; use CAST(… AS BLOB) to preserve its bytes".into());
    }
    Ok(value.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_keeps_sqlite_types_reads_pending_values_and_rejects_tails_and_parameters() {
        use rusqlite::types::Value;
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES(41)").unwrap();
        for (expression, value) in [("(SELECT v+1 FROM t)",Value::Integer(42)), ("1.25*2",Value::Real(2.5)),
            ("upper('中文a')",Value::Text("中文A".into())), ("x'00FF80'",Value::Blob(vec![0,255,128])), ("NULL",Value::Null),
            ("';DROP TABLE t;'",Value::Text(";DROP TABLE t;".into()))] {
            assert_eq!(evaluate(&conn,expression).unwrap(),value);
        }
        for expression in ["", "1; DELETE FROM t", "1,2", "?1", ":value", "(SELECT v FROM missing)", "CAST(x'ff' AS TEXT)"] {
            assert!(evaluate(&conn,expression).is_err(),"{expression}");
        }
        assert_eq!(conn.query_row("SELECT v FROM t",[],|row|row.get::<_,i64>(0)).unwrap(),41);
    }

    #[test]
    fn xml_indentation_preserves_raw_tokens_mixed_content_spaces_entities_and_is_idempotent() {
        let input = "\u{feff}<?xml version='1.0'?><!DOCTYPE r [<!ENTITY word '中文'>]><r xmlns:n='urn:x' a='&quot;'><?keep x?><!-- comment --><n:item>&word;</n:item><items><item/></items></r>";
        let output = indent_xml(input).unwrap();
        assert!(output.contains("\n    <?keep x?>\n    <!-- comment -->\n    <n:item>&word;</n:item>\n    <items>\n        <item/>\n    </items>\n</r>"));
        assert!(output.starts_with("\u{feff}<?xml version='1.0'?><!DOCTYPE r [<!ENTITY word '中文'>]>"));
        assert_eq!(indent_xml(&output).unwrap(), output);
        for input in ["<p>before<b><i/></b>after</p>", "<p> <b/> </p>", "<r xml:space='preserve'><a><b/></a></r>", "<r><![CDATA[ x < y ]]><b/></r>", "<!DOCTYPE r [<!ENTITY markup '<a><b/></a>'>]><r>&markup;</r>"] {
            assert_eq!(indent_xml(input).unwrap(), input);
        }
        let reset = indent_xml("<r xml:space='preserve'><a xml:space='default'><b/></a></r>").unwrap();
        assert!(reset.contains("<a xml:space='default'>\n        <b/>\n    </a>"));
        assert_eq!(xml_document(input).unwrap().root_element().tag_name().name(), "r");
    }

    #[test]
    fn xml_validation_rejects_malformed_namespaces_duplicates_entities_and_external_resolution() {
        for input in ["", "<r>", "<r></a>", "<r/><r/>", "<1bad/>", "<r a='1' a='2'/>", "<n:r/>", "<r>&missing;</r>", "<r>&#0;</r>", "<!DOCTYPE r [<!ENTITY secret SYSTEM 'file:///etc/passwd'>]><r>&secret;</r>"] {
            assert!(xml_document(input).is_err(), "{input}");
        }
        let too_deep = format!("{}{}", "<r>".repeat(130), "</r>".repeat(130));
        assert!(indent_xml(&too_deep).unwrap_err().contains("nesting"));
        assert!(xml_document(&" ".repeat(16*1024*1024+1)).unwrap_err().contains("16 MiB"));
    }
}
