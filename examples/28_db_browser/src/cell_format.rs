use roxmltree::{Document, ParsingOptions};

pub fn validate_json(text: &str) -> Result<(), String> {
    let mut parser=serde_json::Deserializer::from_str(text);
    serde::Deserialize::deserialize(&mut parser).map(|_:serde::de::IgnoredAny|())
        .and_then(|()|parser.end()).map_err(|error|format!("Invalid JSON: {error}"))
}

pub fn format_json(text: &str, indent: bool) -> Result<String, String> {
    // shortcut: bounded whole-cell formatting; use streaming editing for larger JSON cells.
    if text.len()>16*1024*1024 {return Err("JSON exceeds the 16 MiB editor limit".into());}
    validate_json(text)?;
    if text.trim()=="null" {return Ok(text.into());}
    // Keep numeric spellings, escaped strings, duplicate keys and object order intact.
    let mut out=String::with_capacity(text.len());
    let (mut quoted,mut escaped,mut depth,mut previous)=(false,false,0usize,None);
    for (at,ch) in text.char_indices() {
        if quoted {
            out.push(ch);
            if out.len()>32*1024*1024 {return Err("Formatted JSON exceeds the editor limit".into());}
            if escaped {escaped=false;} else if ch=='\\' {escaped=true;} else if ch=='"' {quoted=false;}
            previous=Some(ch);continue;
        }
        if ch.is_ascii_whitespace() {continue;}
        match ch {
            '"'=>{quoted=true;out.push(ch);}
            '{'|'['=>{
                out.push(ch);depth+=1;
                if depth>128 {return Err("JSON nesting exceeds the formatting limit".into());}
                if indent && !text[at+1..].trim_start().starts_with(if ch=='{' {'}'}else{']'}) {out.push('\n');out.push_str(&"    ".repeat(depth));}
            }
            '}'|']'=>{
                depth-=1;
                if indent && previous!=Some(if ch=='}' {'{'}else{'['}) {out.push('\n');out.push_str(&"    ".repeat(depth));}
                out.push(ch);
            }
            ','=>{out.push(ch);if indent {out.push('\n');out.push_str(&"    ".repeat(depth));}}
            ':'=>{out.push(ch);if indent {out.push(' ');}}
            _=>out.push(ch),
        }
        previous=Some(ch);
        if out.len()>32*1024*1024 {return Err("Formatted JSON exceeds the editor limit".into());}
    }
    Ok(out)
}

pub fn cell_image_format(bytes: &[u8]) -> Option<(image::ImageFormat, &'static str)> {
    use image::ImageFormat;
    let format = image::guess_format(bytes).ok()?;
    let name = match format { ImageFormat::Png => "PNG", ImageFormat::Jpeg => "JPEG", ImageFormat::Bmp => "BMP", _ => return None };
    Some((format, name))
}

pub fn automatic_mode(value: &rusqlite::types::Value) -> crate::state::EditCellMode {
    use crate::state::EditCellMode;
    use rusqlite::types::Value;
    let bytes = match value { Value::Text(text) => text.as_bytes(), Value::Blob(bytes) => bytes, _ => return EditCellMode::Text };
    if cell_image_format(bytes).is_some() { return EditCellMode::Image; }
    let Ok(text) = std::str::from_utf8(bytes) else { return EditCellMode::Hex; };
    if text.chars().any(|ch| (ch.is_control() && !ch.is_whitespace())
        || matches!(ch as u32, 0xfdd0..=0xfdef) || ch as u32 & 0xfffe == 0xfffe) { return EditCellMode::Hex; }
    if text.starts_with("<?xml") { return EditCellMode::Xml; }
    if !matches!(text.trim_start().as_bytes().first(),Some(b'-'|b'0'..=b'9')) && validate_json(text).is_ok() { return EditCellMode::Json; }
    // shortcut: RTL stays in Text until shared shaping and bidirectional caret layout exist.
    EditCellMode::Text
}

pub fn xml_document(text: &str) -> Result<Document<'_>, String> {
    // shortcut: bounded whole-cell XML; upgrade to streaming validation for larger cell editors.
    if text.len() > 16 * 1024 * 1024 { return Err("XML exceeds the 16 MiB editor limit".into()); }
    Document::parse_with_options(text, ParsingOptions { allow_dtd: true, nodes_limit: 250_000, entity_resolver: None })
        .map_err(|error| format!("Invalid XML: {error}"))
}

pub fn indent_xml(text: &str) -> Result<String, String> {
    format_xml(text,true)
}

pub fn compact_xml(text: &str) -> Result<String, String> {
    format_xml(text,false)
}

fn format_xml(text: &str, indent: bool) -> Result<String, String> {
    let document = xml_document(text)?;
    let mut styles = vec![(false, false, 0usize); document.descendants().count()];
    let mut replacements = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        let (in_mixed, inherited_space, depth) = node.parent_element()
            .map(|parent| { let (mixed, space, depth) = styles[parent.id().get_usize()]; (mixed, space, depth + 1) })
            .unwrap_or((false, false, 0));
        if depth > 128 { return Err("XML nesting exceeds the indentation limit".into()); }
        let range = node.range();
        // Preserve inline spaces, CDATA and entity expansions; normalize only inter-tag line indentation.
        let spacing = |child:roxmltree::Node<'_, '_>| {
            let range=child.range();let raw=&text[range.clone()];
            child.is_text() && raw.bytes().all(|byte|matches!(byte,b' '|b'\t'|b'\r'|b'\n'))
                && (raw.contains('\n') || raw.contains('\r')) && !text[..range.start].ends_with("<![CDATA[")
        };
        let mixed = in_mixed || !node.children().any(|child|!child.is_text())
            || node.children().any(|child| (child.is_text() && !spacing(child))
                || child.range().start < range.start || child.range().end > range.end);
        let preserve = match node.attribute((roxmltree::NS_XML_URI, "space")) {
            Some("preserve") => true,
            Some("default") => false,
            _ => inherited_space,
        };
        styles[node.id().get_usize()] = (mixed, preserve, depth);
        if mixed || preserve || !node.has_children() { continue; }
        for child in node.children() {
            if spacing(child) {replacements.push((child.range(),String::new()));}
            else if indent {let at=child.range().start;replacements.push((at..at,format!("\n{}","    ".repeat(depth+1))));}
        }
        if indent {if let Some(end) = text[range.clone()].rfind("</") {
            let at=range.start+end;replacements.push((at..at,format!("\n{}","    ".repeat(depth))));
        }}
    }
    replacements.sort_by_key(|(range, _)| range.start);
    let size = replacements.iter().try_fold(text.len(), |size, (range, added)| size.checked_sub(range.len())?.checked_add(added.len()))
        .filter(|size| *size <= 32 * 1024 * 1024).ok_or("Formatted XML exceeds the editor limit")?;
    let mut out = String::with_capacity(size);
    let mut previous = 0;
    for (range, added) in replacements {
        if range.start<previous {return Err("XML expansion overlaps a formatting range".into());}
        out.push_str(&text[previous..range.start]); out.push_str(&added); previous = range.end;
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
    fn json_formatting_uses_four_spaces_and_preserves_large_numbers_and_null() {
        let raw=r#"{ "integer":18446744073709551616,"fraction":0.12345678901234567890123,"exponent":1.23456789e+123,"text":"中文","null":null }"#;
        let pretty=format_json(raw,true).unwrap();let compact=format_json(&pretty,false).unwrap();
        assert!(pretty.contains("\n    \"integer\": 18446744073709551616"));
        for token in ["18446744073709551616","0.12345678901234567890123","1.23456789e+123","中文"] {assert!(pretty.contains(token));assert!(compact.contains(token));}
        assert!(!compact.contains('\n'));assert_eq!(format_json(&compact,true).unwrap(),pretty);
        assert_eq!(format_json(" null \n",true).unwrap()," null \n");assert_eq!(format_json(" null \n",false).unwrap()," null \n");
        assert!(format_json("{bad",true).unwrap_err().contains("line 1 column"));
        let object=r#"{"$serde_json::private::Number":"123"}"#;
        assert_eq!(format_json(object,false).unwrap(),object);
        for raw in [r#"{"duplicate":1,"duplicate":2,"overflow":1e9999}"#,r#"[{},[],"escaped \" quote \\ 中文",-0.000,1E+001]"#] {
            assert_eq!(format_json(&format_json(raw,true).unwrap(),false).unwrap(),raw);
        }
        for text in ["[1,]","true false","{\"a\":1} trailing","\"bad\nstring\"","[01]","NaN"] {assert!(validate_json(text).is_err(),"{text}");}
        let deep=format!("{}0{}","[".repeat(130),"]".repeat(130));assert!(format_json(&deep,true).unwrap_err().contains("nesting"));
        assert!(format_json(&" ".repeat(16*1024*1024+1),true).unwrap_err().contains("16 MiB"));
    }

    #[test]
    fn xml_compaction_normalizes_only_line_indentation_and_keeps_content_and_tokens() {
        let raw="\u{feff}<?xml version='1.0'?><!DOCTYPE r [<!ENTITY word '中文'>]><r a='&quot;'><?keep x?><!-- comment --><item>&word;</item><items><item/></items></r>";
        let pretty=indent_xml(raw).unwrap();assert_eq!(compact_xml(&pretty).unwrap(),raw);
        let irregular="<r>\n\t<a/>\r\n  <b><c/>\n\t</b>\n</r>";
        let pretty=indent_xml(irregular).unwrap();assert_eq!(pretty,"<r>\n    <a/>\n    <b>\n        <c/>\n    </b>\n</r>");
        assert_eq!(compact_xml(irregular).unwrap(),"<r><a/><b><c/></b></r>");assert_eq!(indent_xml(&pretty).unwrap(),pretty);
        for text in ["<p>before<b><i/></b>after</p>","<p> <b/> </p>","<r>\n  </r>",
            "<r xml:space='preserve'>\n<a><b/></a>\n</r>","<r><![CDATA[ \n ]]><b/></r>",
            "<r>&#10;<b/></r>","<!DOCTYPE r [<!ENTITY markup '<a><b/></a>'>]><r>&markup;</r>"] {
            assert_eq!(compact_xml(text).unwrap(),text,"{text}");assert_eq!(indent_xml(text).unwrap(),text,"{text}");
        }
        assert!(compact_xml("<r><bad>").is_err());
    }

    #[test]
    fn automatic_mode_distinguishes_printable_bytes_formats_numbers_and_noncharacters() {
        use crate::state::EditCellMode::*;
        use rusqlite::types::Value;
        for (value, expected) in [(Value::Null,Text),(Value::Integer(i64::MAX),Text),(Value::Real(1.25),Text),
            (Value::Text("中文\n\tvalue".into()),Text),(Value::Blob("中文".as_bytes().to_vec()),Text),
            (Value::Blob(vec![255,0,128]),Hex),(Value::Text("a\0b".into()),Hex),
            (Value::Text("\u{fdd0}".into()),Hex),(Value::Text("\u{10ffff}".into()),Hex),
            (Value::Text("\u{a0}word".into()),Text),(Value::Text("العربية".into()),Text),
            (Value::Text("42".into()),Text),(Value::Text("1e10".into()),Text),
            (Value::Text("true".into()),Json),(Value::Text("null".into()),Json),
            (Value::Text("\"string\"".into()),Json),(Value::Blob(b"{\"a\":1}".to_vec()),Json),
            (Value::Text("<?xml version='1.0'?><r/>".into()),Xml),(Value::Text("<r/>".into()),Text),
            (Value::Text("{broken".into()),Text),(Value::Text("\u{feff}text".into()),Text)] {
            assert_eq!(automatic_mode(&value),expected,"{value:?}");
        }
    }

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
