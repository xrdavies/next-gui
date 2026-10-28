use crate::db::CellValue;
use std::borrow::Cow;
use std::io::{BufRead, Seek, SeekFrom, Write};
use std::ops::RangeInclusive;
use std::sync::Arc;

const INLINE_BYTES: usize = 8 * 1024 * 1024;

pub struct SnapshotWriter {
    output: std::io::BufWriter<tempfile::NamedTempFile>,
    position: u64,
    offsets: Vec<u64>,
    width: usize,
}
impl SnapshotWriter {
    pub fn new(width: usize) -> Result<Self, String> {
        Ok(Self { output: std::io::BufWriter::new(tempfile::NamedTempFile::new().map_err(|e| e.to_string())?), position: 0, offsets: Vec::new(), width })
    }
    pub fn push(&mut self, cells: &[CellValue]) -> Result<(), String> {
        if cells.len() != self.width { return Err("Typed snapshot row width does not match its columns".into()); }
        self.offsets.try_reserve(1).map_err(|e| e.to_string())?;
        self.offsets.push(self.position);
        serde_json::to_writer(&mut *self, cells).map_err(|e| e.to_string())?;
        self.write_all(b"\n").map_err(|e| e.to_string())
    }
    pub fn finish(mut self) -> Result<CellSnapshot, String> {
        self.flush().map_err(|e| e.to_string())?;
        let file = self.output.into_inner().map_err(|e| e.to_string())?;
        Ok(CellSnapshot { file: Some(Arc::new(file)), offsets: Arc::new(self.offsets), columns: (0..self.width).collect(), width: self.width, ..Default::default() })
    }
}
impl Write for SnapshotWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.output.write(bytes)?;
        self.position = self.position.checked_add(count as u64).ok_or_else(|| std::io::Error::other("Typed snapshot file offset overflow"))?;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> { self.output.flush() }
}

pub(crate) mod real_bits {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(value.to_bits())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        u64::deserialize(deserializer).map(f64::from_bits)
    }
}

/// Typed rows, spilling large cells and indexing immutable full results on disk.
#[derive(Debug, Clone, Default)]
pub struct CellSnapshot {
    identity: Arc<()>,
    rows: Vec<Vec<CellValue>>,
    bytes: usize,
    file: Option<Arc<tempfile::NamedTempFile>>,
    offsets: Arc<Vec<u64>>,
    columns: Vec<usize>,
    width: usize,
}

pub struct Rows<'a> {
    source: RowSource<'a>,
    columns: &'a [usize],
    width: usize,
}
enum RowSource<'a> {
    Memory(std::slice::Iter<'a, Vec<CellValue>>),
    File { lines: std::io::Lines<std::io::BufReader<std::fs::File>>, remaining: usize },
}
impl<'a> Iterator for Rows<'a> {
    type Item = Result<Cow<'a, [CellValue]>, String>;
    fn next(&mut self) -> Option<Self::Item> {
        let row = match &mut self.source {
            RowSource::Memory(rows) => Ok(Cow::Borrowed(rows.next()?.as_slice())),
            RowSource::File { lines, remaining } => {
                if *remaining == 0 { return None; }
                *remaining -= 1;
                lines.next().ok_or_else(|| "Typed snapshot ended before its indexed row".to_string())
                    .and_then(|line| line.map_err(|e| e.to_string()))
                    .and_then(|line| parse_row(&line, self.width)).map(Cow::Owned)
            }
        };
        Some(row.and_then(|row| project_row(row, self.columns)))
    }
}

fn parse_row(line: &str, width: usize) -> Result<Vec<CellValue>, String> {
    let row: Vec<CellValue> = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if row.len() != width { return Err("Typed snapshot row width does not match its columns".into()); }
    Ok(row)
}

fn project_row<'a>(cells: Cow<'a, [CellValue]>, columns: &[usize]) -> Result<Cow<'a, [CellValue]>, String> {
    if columns.iter().copied().eq(0..cells.len()) { Ok(cells) } else {
        columns.iter().map(|&column| cells.get(column).cloned().ok_or_else(|| "No preview cell".into()))
            .collect::<Result<Vec<_>, String>>().map(Cow::Owned)
    }
}

fn write_row(file: &mut std::fs::File, cells: &[CellValue]) -> Result<u64, String> {
    let offset = file.stream_position().map_err(|e| e.to_string())?;
    let mut output = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut output, cells).map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    Ok(offset)
}

impl CellSnapshot {
    pub fn file(&self) -> Option<Arc<tempfile::NamedTempFile>> { self.file.clone() }
    pub fn push(&mut self, cells: Vec<CellValue>) -> Result<(), String> {
        if self.is_empty() {
            self.columns = (0..cells.len()).collect();
            self.width = cells.len();
        }
        if cells.len() != self.width { return Err("Typed snapshot row width does not match its columns".into()); }
        let bytes = cells
            .iter()
            .map(|cell| match cell {
                CellValue::Text(bytes) | CellValue::Blob(bytes) => bytes.len(),
                _ => std::mem::size_of::<CellValue>(),
            })
            .sum::<usize>();
        if self.file.is_none() && self.bytes.saturating_add(bytes) > INLINE_BYTES {
            let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
            for row in &self.rows {
                Arc::make_mut(&mut self.offsets).push(write_row(file.as_file_mut(), row)?);
            }
            self.rows.clear();
            self.bytes = 0;
            self.file = Some(Arc::new(file));
        }
        if let Some(file) = &mut self.file {
            let file = Arc::get_mut(file).ok_or("Cannot append to a shared cell snapshot")?;
            Arc::make_mut(&mut self.offsets).push(write_row(file.as_file_mut(), &cells)?);
        } else {
            self.bytes = self.bytes.saturating_add(bytes);
            self.rows.push(cells);
        }
        self.identity = Arc::new(());
        Ok(())
    }

    pub fn len(&self) -> usize {
        if self.file.is_some() {
            self.offsets.len()
        } else {
            self.rows.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn remove_column(&mut self, column: usize) {
        if !self.is_empty() {
            self.columns.remove(column);
            self.identity = Arc::new(());
        }
    }

    pub fn identity(&self) -> Arc<()> {
        self.identity.clone()
    }

    /// Index an immutable typed result without copying its cell payloads.
    pub fn from_file(file: Arc<tempfile::NamedTempFile>, width: usize, mut check: impl FnMut(usize) -> Result<(), String>) -> Result<Self, String> {
        let mut input = std::io::BufReader::new(file.reopen().map_err(|e| e.to_string())?);
        let mut offsets = Vec::new();
        let mut offset = 0u64;
        loop {
            check(offsets.len())?;
            let bytes = input.skip_until(b'\n').map_err(|e| e.to_string())?;
            if bytes == 0 { break; }
            offsets.try_reserve(1).map_err(|e| e.to_string())?;
            offsets.push(offset);
            offset = offset.checked_add(bytes as u64).ok_or("Typed snapshot file offset overflow")?;
        }
        Ok(Self { file: Some(file), offsets: Arc::new(offsets), columns: (0..width).collect(), width, ..Default::default() })
    }

    pub fn rows(&self) -> Result<Rows<'_>, String> {
        let source = if let Some(file) = &self.file {
            RowSource::File { lines: std::io::BufReader::new(file.reopen().map_err(|e| e.to_string())?).lines(), remaining: self.len() }
        } else { RowSource::Memory(self.rows.iter()) };
        Ok(Rows { source, columns: &self.columns, width: self.width })
    }

    /// Read a projected row once, including snapshots spilled to disk.
    pub fn row(&self, index: usize) -> Result<Cow<'_, [CellValue]>, String> {
        let cells = self.read_row(index)?;
        project_row(cells, &self.columns)
    }

    fn read_row(&self, index: usize) -> Result<Cow<'_, [CellValue]>, String> {
        if let Some(file) = &self.file {
            let offset = *self.offsets.get(index).ok_or("No preview row")?;
            let mut input = std::fs::File::open(file.path()).map_err(|e| e.to_string())?;
            input
                .seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            let mut line = String::new();
            std::io::BufReader::new(input)
                .read_line(&mut line)
                .map_err(|e| e.to_string())?;
            parse_row(&line, self.width)
                .map(Cow::Owned)
        } else {
            self.rows
                .get(index)
                .map(|row| Cow::Borrowed(row.as_slice()))
                .ok_or_else(|| "No preview row".into())
        }
    }

    pub fn cell(&self, row: usize, column: usize) -> Result<CellValue, String> {
        let column = *self.columns.get(column).ok_or("No preview column")?;
        if self.file.is_none() {
            return self
                .rows
                .get(row)
                .and_then(|r| r.get(column))
                .cloned()
                .ok_or_else(|| "No preview cell".into());
        }
        self.read_row(row)?
            .get(column)
            .cloned()
            .ok_or_else(|| "No preview cell".into())
    }

    pub fn rectangle(
        &self,
        rows: RangeInclusive<usize>,
        cols: RangeInclusive<usize>,
    ) -> Result<Vec<Vec<CellValue>>, String> {
        if rows.is_empty()
            || cols.is_empty()
            || *rows.end() >= self.len()
            || *cols.end() >= self.columns.len()
        {
            return Err("Selection no longer fits the preview".into());
        }
        rows.map(|row| {
            let cells = self.read_row(row)?;
            cols.clone()
                .map(|col| {
                    cells
                        .get(self.columns[col])
                        .cloned()
                        .ok_or_else(|| "No preview cell".into())
                })
                .collect()
        })
        .collect()
    }
}

impl next_gui_ui::TableDataSource for CellSnapshot {
    fn len(&self) -> usize { self.len() }
    fn row(&self, index: usize) -> Result<Vec<String>, String> {
        Ok(self.row(index)?.iter().map(|cell| crate::db::cell_to_string(cell.as_ref())).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executed_rows_are_indexed_once_and_keep_raw_values_and_shared_offsets() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let reads = Arc::new(AtomicUsize::new(0)); let calls = reads.clone();
        conn.create_scalar_function("observed", 1, rusqlite::functions::FunctionFlags::SQLITE_UTF8, move |context| {
            calls.fetch_add(1, Ordering::Relaxed); context.get::<i64>(0)
        }).unwrap();
        let result = crate::db::execute_sql(&conn, "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<3000) SELECT observed(x),CASE WHEN x%2=0 THEN NULL ELSE CAST(x'FF00' AS TEXT) END,zeroblob(x%23),9223372036854775807 FROM n").unwrap();
        assert_eq!(reads.load(Ordering::Relaxed), 3000);
        assert_eq!(result.values.len(), 500);
        let values = result.all_values.unwrap();
        assert_eq!(values.len(), 3000);
        assert!(Arc::ptr_eq(&values.file().unwrap(), &result.export_rows.unwrap()));
        let clone = values.as_ref().clone();
        assert!(Arc::ptr_eq(&values.offsets, &clone.offsets));
        for row in [0, 499, 500, 1002, 2999] {
            let x = row as i64 + 1;
            let expected = vec![CellValue::Integer(x), if x%2==0 { CellValue::Null } else { CellValue::Text(vec![255,0]) }, CellValue::Blob(vec![0; x as usize%23]), CellValue::Integer(i64::MAX)];
            assert_eq!(clone.row(row).unwrap().as_ref(), expected);
        }
        assert_eq!(reads.load(Ordering::Relaxed), 3000);
        let mut output = SnapshotWriter::new(1).unwrap();
        assert!(output.push(&[]).is_err());
        output.push(&[CellValue::Integer(9)]).unwrap();
        assert_eq!(output.finish().unwrap().cell(0,0).unwrap(), CellValue::Integer(9));
        assert_eq!(SnapshotWriter::new(0).unwrap().finish().unwrap().len(), 0);
    }

    #[test]
    fn empty_preview_has_no_cells_to_project() {
        let mut snapshot = CellSnapshot::default();
        snapshot.remove_column(0);
        assert!(snapshot.is_empty());
        assert!(snapshot.cell(0, 0).is_err());
        assert!(snapshot.rectangle(0..=0, 0..=0).is_err());
    }

    #[test]
    fn full_file_rows_stream_project_and_cancel_without_copying_the_payload() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let result = crate::db::execute_sql(&conn, "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1003) SELECT x, CASE WHEN x=2 THEN NULL ELSE x*0.5 END, x'ff00' FROM n").unwrap();
        assert_eq!(result.values.len(), 500);
        let file = result.export_rows.as_ref().unwrap().clone();
        let mut count = 0;
        let mut full = CellSnapshot::from_file(file.clone(), 3, |rows| { count = rows; Ok(()) }).unwrap();
        assert_eq!((full.len(), count), (1003, 1003));
        assert!(full.rows.is_empty());
        assert!(Arc::ptr_eq(full.file.as_ref().unwrap(), &file));
        full.remove_column(0);
        let streamed: Vec<_> = full.rows().unwrap().map(|row| row.unwrap().into_owned()).collect();
        assert_eq!(streamed.len(), 1003);
        assert_eq!(streamed[1], [CellValue::Null, CellValue::Blob(vec![255, 0])]);
        assert_eq!(streamed[1002], full.row(1002).unwrap().as_ref());
        assert!(CellSnapshot::from_file(file, 3, |rows| if rows == 2 { Err("cancelled".into()) } else { Ok(()) }).unwrap_err().contains("cancelled"));
        let mut invalid = tempfile::NamedTempFile::new().unwrap();
        writeln!(invalid, "[]").unwrap();
        let invalid = CellSnapshot::from_file(Arc::new(invalid), 1, |_| Ok(())).unwrap();
        assert!(invalid.row(0).is_err());
        assert!(invalid.rows().unwrap().next().unwrap().is_err());
        let mut memory = CellSnapshot::default();
        memory.push(vec![CellValue::Integer(7)]).unwrap();
        assert!(memory.push(vec![]).is_err());
        assert_eq!(memory.rows().unwrap().next().unwrap().unwrap().as_ref(), [CellValue::Integer(7)]);
    }

    #[test]
    fn large_preview_spills_and_clone_keeps_bytes_and_column_projection() {
        let mut snapshot = CellSnapshot::default();
        snapshot
            .push(vec![
                CellValue::Integer(1),
                CellValue::Text(vec![255]),
                CellValue::Real(-0.0),
            ])
            .unwrap();
        let blob = vec![7; INLINE_BYTES + 1];
        snapshot
            .push(vec![
                CellValue::Integer(2),
                CellValue::Blob(blob.clone()),
                CellValue::Real(f64::INFINITY),
            ])
            .unwrap();
        snapshot
            .push(vec![
                CellValue::Integer(3),
                CellValue::Null,
                CellValue::Real(f64::NEG_INFINITY),
            ])
            .unwrap();
        assert!(snapshot.file.is_some());
        assert!(snapshot.rows.is_empty());
        assert_eq!(snapshot.bytes, 0);
        assert_eq!(snapshot.len(), 3);
        snapshot.remove_column(0);
        let cloned = snapshot.clone();
        assert!(Arc::ptr_eq(&snapshot.identity(), &cloned.identity()));
        assert_eq!(cloned.row(2).unwrap().as_ref(), &[CellValue::Null, CellValue::Real(f64::NEG_INFINITY)]);
        drop(snapshot);
        assert_eq!(cloned.cell(0, 0).unwrap(), CellValue::Text(vec![255]));
        assert_eq!(cloned.cell(1, 0).unwrap(), CellValue::Blob(blob));
        assert_eq!(cloned.cell(1, 1).unwrap(), CellValue::Real(f64::INFINITY));
        assert_eq!(
            cloned.cell(2, 1).unwrap(),
            CellValue::Real(f64::NEG_INFINITY)
        );
        assert!(matches!(cloned.cell(0, 1).unwrap(), CellValue::Real(n) if n.is_sign_negative()));
        assert_eq!(
            cloned.rectangle(2..=2, 0..=0).unwrap(),
            vec![vec![CellValue::Null]]
        );
        assert!(cloned.cell(3, 0).is_err());
    }
}
