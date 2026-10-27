use crate::db::{self, CellValue};
use std::sync::atomic::{AtomicU64, Ordering};

/// The HTML stamp keeps typed cells tied to the clipboard that exported them.
pub struct CellClipboard {
    pub text: String,
    pub html: String,
    marker: String,
    cells: Vec<Vec<CellValue>>,
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn cell_text(cell: &CellValue) -> String {
    match cell {
        CellValue::Null => String::new(),
        CellValue::Integer(n) => n.to_string(),
        CellValue::Real(n) => n.to_string(),
        CellValue::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        CellValue::Blob(bytes) => format!("X'{}'", db::to_hex(bytes)),
    }
}

fn tsv_field(text: &str) -> String {
    if text.is_empty() || text.contains(['\t', '\r', '\n', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

impl CellClipboard {
    pub fn new(
        schema: &str,
        table: &str,
        columns: &[String],
        cells: Vec<Vec<CellValue>>,
        headers: bool,
        sql: bool,
    ) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let marker = format!(
            "<!--next-gui-e28:{}:{epoch}:{}-->",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let mut html = format!("<html><head>{marker}</head><body><table>");
        let mut lines = Vec::new();
        if headers && !sql {
            lines.push(
                columns
                    .iter()
                    .map(|c| tsv_field(c))
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
            html.push_str("<tr>");
            for col in columns {
                html.push_str(&format!("<th>{}</th>", html_escape(col)));
            }
            html.push_str("</tr>");
        }
        let insert = format!(
            "INSERT INTO {} ({}) VALUES",
            db::qualified_name(schema, table),
            columns
                .iter()
                .map(|c| db::quote_ident(c))
                .collect::<Vec<_>>()
                .join(", ")
        );
        for row in &cells {
            if sql {
                lines.push(format!(
                    "{insert} ({});",
                    row.iter()
                        .map(|c| db::sql_literal(c.as_ref()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            } else {
                lines.push(
                    row.iter()
                        .map(|c| tsv_field(&cell_text(c)))
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
            }
            html.push_str("<tr>");
            for cell in row {
                html.push_str(&format!(
                    "<td><pre>{}</pre></td>",
                    html_escape(&cell_text(cell))
                ));
            }
            html.push_str("</tr>");
        }
        html.push_str("</table></body></html>");
        // SQL copies deliberately paste as SQL text, even within this application.
        if sql {
            html.clear();
        }
        Self {
            text: lines.join("\n"),
            html,
            marker,
            cells,
        }
    }

    pub fn internal_cells(&self, html: &str) -> Option<Vec<Vec<CellValue>>> {
        (!self.html.is_empty() && html.contains(&self.marker)).then(|| self.cells.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_is_escaped_and_internal_cells_keep_all_sqlite_storage_types() {
        let cells = vec![vec![
            CellValue::Null,
            CellValue::Text(Vec::new()),
            CellValue::Text(b"NULL\t\n\"<&".to_vec()),
            CellValue::Integer(i64::MAX),
            CellValue::Real(1.25),
            CellValue::Blob(vec![0, 255]),
            CellValue::Text(vec![255]),
        ]];
        let columns = vec!["<header>".into(); 7];
        let copy = CellClipboard::new("main", "t", &columns, cells.clone(), true, false);
        assert!(copy.html.contains("&lt;header&gt;"));
        assert!(copy.html.contains("&quot;&lt;&amp;"));
        assert!(copy.text.contains("\"NULL\t\n\"\"<&\""));
        assert_eq!(copy.internal_cells(&copy.html), Some(cells.clone()));
        assert_eq!(copy.internal_cells("<html>external</html>"), None);
        let other = CellClipboard::new("main", "t", &columns, cells, false, false);
        assert_eq!(copy.internal_cells(&other.html), None);
    }

    #[test]
    fn sql_copy_quotes_identifiers_and_preserves_blob_null_and_invalid_text() {
        let copy = CellClipboard::new(
            "main",
            "odd\"table",
            &["a\"b".into(), "c".into(), "d".into()],
            vec![vec![
                CellValue::Null,
                CellValue::Blob(vec![0, 255]),
                CellValue::Text(vec![255]),
            ]],
            false,
            true,
        );
        assert_eq!(copy.text, "INSERT INTO \"main\".\"odd\"\"table\" (\"a\"\"b\", \"c\", \"d\") VALUES (NULL, X'00FF', CAST(X'FF' AS TEXT));");
        assert!(copy.html.is_empty());
        assert_eq!(copy.internal_cells(&copy.html), None);
    }
}
