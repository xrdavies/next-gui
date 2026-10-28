//! One parser for bounded previews, files and Unicode clipboard imports.

use crate::db::CsvImportOptions;

#[derive(Debug, Clone)]
pub struct Data {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub types: Vec<&'static str>,
}

struct Parser<'a> {
    options: &'a CsvImportOptions,
    records: Vec<Vec<String>>,
    record: Vec<String>,
    field: String,
    quoted: bool,
    closed: bool,
    skip_lf: bool,
    limit: usize,
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
                self.records.push(std::mem::take(&mut self.record));
                self.skip_lf = ch == '\r';
                if self.limit != 0 && self.records.len() >= self.limit {
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

    fn finish(mut self) -> Result<Data, String> {
        if self.quoted {
            return Err("Unterminated CSV quoted field".into());
        }
        if !self.field.is_empty() || !self.record.is_empty() || self.closed {
            self.field();
            self.records.push(std::mem::take(&mut self.record));
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
