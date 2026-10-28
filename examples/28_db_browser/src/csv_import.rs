//! One parser for bounded previews, files and Unicode clipboard imports.

use crate::db::CsvImportOptions;

#[derive(Debug, Clone)]
pub struct Data {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub types: Vec<&'static str>,
}

type RecordSink<'a> = dyn FnMut(&[String]) -> Result<(), String> + 'a;

struct Parser<'a> {
    options: &'a CsvImportOptions,
    records: Vec<Vec<String>>,
    record: Vec<String>,
    field: String,
    quoted: bool,
    closed: bool,
    skip_lf: bool,
    limit: usize,
    count: usize,
    sink: Option<&'a mut RecordSink<'a>>,
}

impl<'a> Parser<'a> {
    fn new(options: &'a CsvImportOptions, limit: usize) -> Result<Self, String> {
        if ['\0', '\r', '\n'].contains(&options.delimiter)
            || options
                .quote
                .is_some_and(|ch| ['\0', '\r', '\n'].contains(&ch) || ch == options.delimiter)
        {
            return Err("CSV delimiter and quote must be distinct single characters, excluding NUL and line breaks".into());
        }
        Ok(Self {
            options,
            records: Vec::new(),
            record: Vec::new(),
            field: String::new(),
            quoted: false,
            closed: false,
            skip_lf: false,
            limit,
            count: 0,
            sink: None,
        })
    }

    fn field(&mut self) {
        let field = std::mem::take(&mut self.field);
        self.record.push(if self.options.trim_fields {
            field
                .trim_matches(|ch: char| ch.is_ascii_whitespace())
                .into()
        } else {
            field
        });
        self.closed = false;
    }

    fn consume(&mut self, text: &str) -> Result<bool, String> {
        for ch in text.chars() {
            if self.skip_lf {
                self.skip_lf = false;
                if ch == '\n' {
                    continue;
                }
            }
            if self.quoted {
                if Some(ch) == self.options.quote {
                    self.quoted = false;
                    self.closed = true;
                } else {
                    self.field.push(ch);
                }
            } else if self.closed && Some(ch) == self.options.quote {
                self.field.push(ch);
                self.quoted = true;
                self.closed = false;
            } else if ch == self.options.delimiter {
                self.field();
            } else if ch == '\r' || ch == '\n' {
                self.field();
                self.record()?;
                self.skip_lf = ch == '\r';
                if self.limit != 0 && self.count >= self.limit {
                    return Ok(false);
                }
            } else if self.closed {
                return Err("Unexpected text after CSV closing quote".into());
            } else if Some(ch) == self.options.quote && self.field.is_empty() {
                self.quoted = true;
            } else {
                self.field.push(ch);
            }
        }
        Ok(true)
    }

    fn record(&mut self) -> Result<(), String> {
        let row = std::mem::take(&mut self.record);
        self.count += 1;
        if let Some(sink) = &mut self.sink {
            sink(&row)?;
            if self.records.len() < 20 {
                self.records.push(row);
            }
        } else {
            self.records.push(row);
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Data, String> {
        if self.quoted {
            return Err("Unterminated CSV quoted field".into());
        }
        if !self.field.is_empty() || !self.record.is_empty() || self.closed {
            self.field();
            self.record()?;
        }
        if self.records.is_empty() {
            return Err("empty CSV".into());
        }
        let mut columns = if self.options.has_header {
            self.records.remove(0)
        } else {
            vec![String::new(); self.records[0].len()]
        };
        for (index, column) in columns.iter_mut().enumerate() {
            if column.is_empty() {
                *column = format!("field{}", index + 1);
            }
        }
        if columns.iter().enumerate().any(|(index, name)| {
            columns[..index]
                .iter()
                .any(|prior| prior.eq_ignore_ascii_case(name))
        }) || self.records.iter().any(|row| row.len() != columns.len())
        {
            return Err(
                "CSV needs unique headers and the same number of fields in every row".into(),
            );
        }
        let sample = 20 - usize::from(self.options.has_header);
        let types = (0..columns.len())
            .map(|column| {
                if !self.options.detect_types {
                    return "";
                }
                let mut kind = "";
                for row in self.records.iter().take(sample) {
                    let value = row[column].trim();
                    let next = if value.parse::<i64>().is_ok() {
                        "INTEGER"
                    } else if value.parse::<f64>().is_ok_and(f64::is_finite) {
                        "REAL"
                    } else {
                        "TEXT"
                    };
                    kind = match (kind, next) {
                        ("TEXT", _) | (_, "TEXT") => "TEXT",
                        ("REAL", _) | (_, "REAL") => "REAL",
                        _ => "INTEGER",
                    };
                }
                kind
            })
            .collect();
        Ok(Data {
            columns,
            rows: self.records,
            types,
        })
    }
}

pub fn read(
    reader: impl std::io::Read,
    options: &CsvImportOptions,
    limit: usize,
) -> Result<Data, String> {
    let mut parser = Parser::new(options, limit)?;
    crate::csv_encoding::decode_reader(reader, &options.encoding, |text| parser.consume(text))?;
    parser.finish()
}

pub fn text(text: &str, options: &CsvImportOptions, limit: usize) -> Result<Data, String> {
    let mut parser = Parser::new(options, limit)?;
    parser.consume(text.trim_start_matches('\u{feff}'))?;
    parser.finish()
}

/// Validated private snapshot, retaining only the original type/preview sample in memory.
#[derive(Debug)]
pub struct Prepared {
    pub preview: Data,
    pub row_count: u64,
    header: bool,
    file: std::fs::File,
}

struct SnapshotReader<'a> {
    file: &'a std::fs::File,
    offset: u64,
}

impl std::io::Read for SnapshotReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        #[cfg(unix)]
        let result = std::os::unix::fs::FileExt::read_at(self.file, buffer, self.offset);
        #[cfg(windows)]
        let result = std::os::windows::fs::FileExt::seek_read(self.file, buffer, self.offset);
        #[cfg(not(any(unix, windows)))]
        let result = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "CSV snapshots require positional file reads",
        ));
        let count = result?;
        self.offset += count as u64;
        Ok(count)
    }
}

impl Prepared {
    pub fn rows(&self) -> Result<impl Iterator<Item = Result<Vec<String>, String>> + '_, String> {
        use std::io::BufRead;
        let mut lines = std::io::BufReader::new(SnapshotReader {
            file: &self.file,
            offset: 0,
        })
        .lines();
        if self.header {
            let line = lines
                .next()
                .ok_or("CSV snapshot has no header")?
                .map_err(|error| error.to_string())?;
            serde_json::from_str::<Vec<String>>(&line).map_err(|error| error.to_string())?;
        }
        let mut remaining = self.row_count;
        let mut ended = false;
        Ok(std::iter::from_fn(move || {
            if ended {
                return None;
            }
            match lines.next() {
                Some(line) if remaining > 0 => {
                    remaining -= 1;
                    Some(line.map_err(|error| error.to_string()).and_then(|line| {
                        serde_json::from_str(&line).map_err(|error| error.to_string())
                    }))
                }
                Some(_) => {
                    ended = true;
                    Some(Err("CSV snapshot contains unexpected extra rows".into()))
                }
                None if remaining > 0 => {
                    ended = true;
                    Some(Err(
                        "CSV snapshot ended before all validated rows were read".into(),
                    ))
                }
                None => {
                    ended = true;
                    None
                }
            }
        }))
    }
}

pub fn prepare(reader: impl std::io::Read, options: &CsvImportOptions) -> Result<Prepared, String> {
    use std::io::Write;
    // Anonymous files are removed by the OS when handles close, including process crashes.
    let mut file = tempfile::tempfile().map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = file.metadata().map_err(|error| error.to_string())?.permissions();
        permissions.set_mode(0o600);
        file.set_permissions(permissions).map_err(|error| error.to_string())?;
    }
    let mut writer = std::io::BufWriter::new(&mut file);
    let mut width = None;
    let mut count = 0;
    let mut sink = |row: &[String]| {
        if *width.get_or_insert(row.len()) != row.len() {
            return Err(
                "CSV needs unique headers and the same number of fields in every row".into(),
            );
        }
        serde_json::to_writer(&mut writer, row).map_err(|error| error.to_string())?;
        writer.write_all(b"\n").map_err(|error| error.to_string())?;
        count += 1;
        Ok(())
    };
    let mut parser = Parser::new(options, 0)?;
    parser.sink = Some(&mut sink);
    crate::csv_encoding::decode_reader(reader, &options.encoding, |text| parser.consume(text))?;
    let preview = parser.finish()?;
    writer.flush().map_err(|error| error.to_string())?;
    drop(writer);
    Ok(Prepared {
        preview,
        row_count: count - usize::from(options.has_header) as u64,
        header: options.has_header,
        file,
    })
}

#[cfg(test)]
pub fn prepare_text(text: &str, options: &CsvImportOptions) -> Result<Prepared, String> {
    let unicode = CsvImportOptions {
        encoding: "UTF-8".into(),
        ..options.clone()
    };
    prepare(text.trim_start_matches('\u{feff}').as_bytes(), &unicode)
}

pub fn character(value: &str, codepoint: bool) -> Option<char> {
    if codepoint {
        let value = value.trim();
        let unit = if let Some(hex) = value
            .strip_prefix("U+")
            .or_else(|| value.strip_prefix("u+"))
            .or_else(|| value.strip_prefix("0x"))
        {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            value.parse().ok()?
        };
        char::from_u32(unit)
    } else {
        let mut chars = value.chars();
        let ch = chars.next()?;
        chars.next().is_none().then_some(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    struct Fragmented<'a>(&'a [u8]);
    impl Read for Fragmented<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let len = out.len().min(1);
            self.0.read(&mut out[..len])
        }
    }

    #[test]
    fn csv_stream_snapshot_keeps_only_sample_reopens_exact_rows_and_imports_large_files() {
        use std::io::{Seek, SeekFrom, Write};
        let mut source = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = std::io::BufWriter::new(source.as_file_mut());
            writeln!(writer, "id,note").unwrap();
            for index in 0..50_000 {
                writeln!(writer, "{index},\"日本\nrecord {index}\"").unwrap();
            }
            writer.flush().unwrap();
        }
        source.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
        let options = CsvImportOptions::default();
        let snapshot = prepare(source.as_file_mut(), &options).unwrap();
        assert_eq!(snapshot.row_count, 50_000);
        assert_eq!(snapshot.preview.rows.len(), 19);
        assert_eq!(snapshot.preview.rows[18], ["18", "日本\nrecord 18"]);
        assert_eq!(snapshot.preview.types, ["INTEGER", "TEXT"]);
        source.as_file_mut().set_len(0).unwrap();
        let mut seen = 0;
        for row in snapshot.rows().unwrap() {
            let row = row.unwrap();
            assert_eq!(row, [seen.to_string(), format!("日本\nrecord {seen}")]);
            seen += 1;
        }
        assert_eq!(seen, 50_000);
        assert_eq!(snapshot.rows().unwrap().next().unwrap().unwrap()[0], "0");
        let mut first = snapshot.rows().unwrap();
        let mut second = snapshot.rows().unwrap();
        for index in 0..3 {
            assert_eq!(first.next().unwrap().unwrap()[0], index.to_string());
            assert_eq!(second.next().unwrap().unwrap()[0], index.to_string());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            assert_eq!(snapshot.file.metadata().unwrap().nlink(),0);
            assert_eq!(
                snapshot.file.metadata().unwrap().permissions().mode() & 0o077,
                0
            );
        }
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE keeper(v);INSERT INTO keeper VALUES('saved');BEGIN;INSERT INTO keeper VALUES('pending')").unwrap();
        assert_eq!(
            crate::db::import_csv_rows(
                &conn,
                "streamed",
                &snapshot.preview.columns,
                &snapshot.preview.types,
                snapshot.rows().unwrap(),
                &options
            )
            .unwrap(),
            50_000
        );
        assert_eq!(
            conn.query_row("SELECT count(*),sum(id) FROM streamed", [], |row| Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?
            )))
            .unwrap(),
            (50_000, 1_249_975_000)
        );
        assert_eq!(crate::db::count_rows(&conn, "keeper").unwrap(), 2);
        assert!(!conn.is_autocommit());
        conn.execute_batch("ROLLBACK").unwrap();
        assert!(crate::db::table_columns(&conn, "streamed")
            .unwrap()
            .is_empty());
        let no_header = prepare_text(
            "1;日本\n2;other\n",
            &CsvImportOptions {
                has_header: false,
                delimiter: ';',
                ..options.clone()
            },
        )
        .unwrap();
        assert_eq!(no_header.row_count, 2);
        assert_eq!(no_header.preview.columns, ["field1", "field2"]);
        assert_eq!(
            no_header
                .rows()
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [["1", "日本"], ["2", "other"]]
        );
        let empty_table = prepare_text("v\n", &options).unwrap();
        assert_eq!(empty_table.row_count, 0);
        assert!(empty_table.rows().unwrap().next().is_none());
    }

    #[test]
    fn csv_stream_rejects_invalid_tail_and_rolls_back_snapshot_read_failures() {
        use std::io::{BufRead, Seek, SeekFrom, Write};
        let options = CsvImportOptions::default();
        let prefix = format!(
            "v\n{}",
            (0..25)
                .map(|index| format!("{index}\n"))
                .collect::<String>()
        );
        for suffix in [b"wrong,width\n".as_slice(), b"\"unclosed", b"\xff"] {
            let mut bytes = prefix.as_bytes().to_vec();
            bytes.extend(suffix);
            assert!(read(Fragmented(&bytes), &options, 20).is_ok());
            assert!(prepare(Fragmented(&bytes), &options).is_err());
        }
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE keeper(v);INSERT INTO keeper VALUES('saved');CREATE TABLE existing(v INTEGER);INSERT INTO existing VALUES(42);BEGIN;INSERT INTO keeper VALUES('pending')").unwrap();
        for truncate in [false, true] {
            let mut snapshot = prepare_text(&prefix, &options).unwrap();
            if truncate {
                let mut reader = std::io::BufReader::new(snapshot.file.try_clone().unwrap());
                reader.seek(SeekFrom::Start(0)).unwrap();
                for _ in 0..6 {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                }
                snapshot
                    .file
                    .set_len(reader.stream_position().unwrap())
                    .unwrap();
            } else {
                snapshot.file.seek(SeekFrom::End(0)).unwrap();
                snapshot.file.write_all(b"not JSON\n").unwrap();
            }
            for table in ["fresh", "existing"] {
                assert!(crate::db::import_csv_rows(
                    &conn,
                    table,
                    &snapshot.preview.columns,
                    &snapshot.preview.types,
                    snapshot.rows().unwrap(),
                    &options
                )
                .is_err());
                assert_eq!(crate::db::count_rows(&conn, "existing").unwrap(), 1);
                assert_eq!(crate::db::count_rows(&conn, "keeper").unwrap(), 2);
                assert!(!conn.is_autocommit());
                assert!(crate::db::table_columns(&conn, "fresh").unwrap().is_empty());
            }
        }
        let unicode = prepare_text(
            "\u{feff}v\n日本\n",
            &CsvImportOptions {
                encoding: "unknown-codec".into(),
                ..options
            },
        )
        .unwrap();
        assert_eq!(unicode.rows().unwrap().next().unwrap().unwrap(), ["日本"]);
    }

    #[test]
    fn chunk_boundaries_preserve_boms_quotes_crlf_unicode_and_trim() {
        let input = ",amount,note\r\n 日本 ,1,\" line 1\r\n\"\"quoted\"\" \"\r\nlast,2.5,end\r\n";
        let options = CsvImportOptions {
            trim_fields: true,
            ..CsvImportOptions::default()
        };
        let expected = text(input, &options, 0).unwrap();
        assert_eq!(expected.columns, ["field1", "amount", "note"]);
        assert_eq!(expected.types, ["TEXT", "REAL", "TEXT"]);
        assert_eq!(expected.rows[0], ["日本", "1", "line 1\r\n\"quoted\""]);
        let mut cases = vec![("UTF-8", input.as_bytes().to_vec())];
        for (label, little, wide) in [
            ("UTF-16LE", true, false),
            ("UTF-16BE", false, false),
            ("UTF-32LE", true, true),
            ("UTF-32BE", false, true),
        ] {
            let mut raw = Vec::new();
            if wide {
                for ch in input.chars() {
                    raw.extend(if little {
                        (ch as u32).to_le_bytes()
                    } else {
                        (ch as u32).to_be_bytes()
                    });
                }
            } else {
                for unit in input.encode_utf16() {
                    raw.extend(if little {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
            }
            cases.push((label, raw));
        }
        for (encoding, raw) in cases {
            let parsed = read(
                Fragmented(&raw),
                &CsvImportOptions {
                    encoding: encoding.into(),
                    ..options.clone()
                },
                0,
            )
            .unwrap();
            assert_eq!(parsed.rows, expected.rows, "{encoding}");
            assert_eq!(parsed.types, expected.types);
            let snapshot = prepare(
                Fragmented(&raw),
                &CsvImportOptions {
                    encoding: encoding.into(),
                    ..options.clone()
                },
            )
            .unwrap();
            assert_eq!(snapshot.preview.types, expected.types);
            assert_eq!(
                snapshot
                    .rows()
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap(),
                expected.rows
            );
        }
        let custom = CsvImportOptions {
            delimiter: '§',
            quote: Some('「'),
            ..options.clone()
        };
        assert_eq!(
            text("a§b\n1§「x§y「\n", &custom, 0).unwrap().rows[0],
            ["1", "x§y"]
        );
        assert!(text("a,b\n1,\"x\"oops", &options, 0).is_err());
        assert!(text("a,b\n1", &options, 0).is_err());
    }

    #[test]
    fn preview_stops_before_invalid_tail_and_types_promote_without_losing_real_affinity() {
        let mut bytes = b"n\n".to_vec();
        for _ in 0..30 {
            bytes.extend(b"2.5\n");
        }
        bytes.extend([0xff]);
        let options = CsvImportOptions::default();
        let preview = read(Fragmented(&bytes), &options, 20).unwrap();
        assert_eq!(preview.rows.len(), 19);
        assert_eq!(preview.types, ["REAL"]);
        assert!(read(Fragmented(&bytes), &options, 0).is_err());
        assert_eq!(text("n\n1\n2.5\n3\n", &options, 0).unwrap().types, ["REAL"]);
        assert_eq!(text("n\n1\nword\n", &options, 0).unwrap().types, ["TEXT"]);
        assert_eq!(
            text(
                "n\n1\n",
                &CsvImportOptions {
                    detect_types: false,
                    ..options.clone()
                },
                0
            )
            .unwrap()
            .types,
            [""]
        );
        for delimiter in ['\0', '\n', '"'] {
            assert!(text(
                "a",
                &CsvImportOptions {
                    delimiter,
                    ..options.clone()
                },
                0
            )
            .is_err());
        }
    }
}
