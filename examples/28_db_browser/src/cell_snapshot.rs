use crate::db::CellValue;
use std::borrow::Cow;
use std::io::{BufRead, Seek, SeekFrom, Write};
use std::ops::RangeInclusive;
use std::sync::Arc;

const INLINE_BYTES: usize = 8 * 1024 * 1024;

pub(crate) mod real_bits {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(value.to_bits())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        u64::deserialize(deserializer).map(f64::from_bits)
    }
}

/// Typed preview rows, spilling large cells to disk instead of retaining whole pages.
#[derive(Debug, Clone, Default)]
pub struct CellSnapshot {
    identity: Arc<()>,
    rows: Vec<Vec<CellValue>>,
    bytes: usize,
    file: Option<Arc<tempfile::NamedTempFile>>,
    offsets: Vec<u64>,
    columns: Vec<usize>,
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
    pub fn push(&mut self, cells: Vec<CellValue>) -> Result<(), String> {
        if self.is_empty() {
            self.columns = (0..cells.len()).collect();
        }
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
                self.offsets.push(write_row(file.as_file_mut(), row)?);
            }
            self.rows.clear();
            self.bytes = 0;
            self.file = Some(Arc::new(file));
        }
        if let Some(file) = &mut self.file {
            let file = Arc::get_mut(file).ok_or("Cannot append to a shared cell snapshot")?;
            self.offsets.push(write_row(file.as_file_mut(), &cells)?);
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

    /// Read a projected row once, including snapshots spilled to disk.
    pub fn row(&self, index: usize) -> Result<Cow<'_, [CellValue]>, String> {
        let cells = self.read_row(index)?;
        if self.columns.iter().copied().eq(0..cells.len()) {
            Ok(cells)
        } else {
            self.columns.iter().map(|&column| cells.get(column).cloned().ok_or_else(|| "No preview cell".into()))
                .collect::<Result<Vec<_>, String>>().map(Cow::Owned)
        }
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
            serde_json::from_str::<Vec<CellValue>>(&line)
                .map(Cow::Owned)
                .map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_preview_has_no_cells_to_project() {
        let mut snapshot = CellSnapshot::default();
        snapshot.remove_column(0);
        assert!(snapshot.is_empty());
        assert!(snapshot.cell(0, 0).is_err());
        assert!(snapshot.rectangle(0..=0, 0..=0).is_err());
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
