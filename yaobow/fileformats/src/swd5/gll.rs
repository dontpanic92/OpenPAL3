//! `.GLL` container files used by SWDHC (and other Xuan-Yuan Sword titles).
//!
//! The container is a simple sparse, index-addressed blob table:
//!
//! ```text
//! offset  0: "GLL022" followed by 10 zero bytes            (16 bytes)
//! offset 16: u32 offset table, terminated by 0xFFFFFFFF
//!            offsets are relative to byte 16 (the start of the table itself)
//! ```
//!
//! Record `i` spans `[offsets[i], offsets[i + 1])`, and the final record runs
//! to the end of the file. Empty (zero-length) records are common: the tables
//! are sparse and indexed by in-game id, so gaps are expected.
//!
//! The per-file record payload layout differs from one `.GLL` to another and is
//! not decoded here — this type only cracks the container.

use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt};

const MAGIC: &[u8; 6] = b"GLL022";
const HEADER_SIZE: usize = 16;
const TERMINATOR: u32 = 0xFFFF_FFFF;

#[derive(Debug, Clone)]
pub struct GllFile {
    /// Byte ranges of each record, relative to the start of the whole file.
    records: Vec<(usize, usize)>,
    data: Vec<u8>,
}

impl GllFile {
    pub fn read(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < HEADER_SIZE || &data[..MAGIC.len()] != MAGIC {
            anyhow::bail!("Not a GLL022 file");
        }

        let mut cursor = Cursor::new(&data[HEADER_SIZE..]);
        let mut offsets = vec![];
        loop {
            let offset = cursor.read_u32::<LittleEndian>()?;
            if offset == TERMINATOR {
                break;
            }

            // Offsets are relative to the start of the offset table.
            offsets.push(HEADER_SIZE + offset as usize);
        }

        let table_end = HEADER_SIZE + cursor.position() as usize;
        if let Some(&first) = offsets.first()
            && first != table_end
        {
            anyhow::bail!(
                "Malformed GLL: first record starts at {first} but the offset table ends at {table_end}"
            );
        }

        let records = offsets
            .iter()
            .enumerate()
            .map(|(i, &start)| {
                let end = offsets.get(i + 1).copied().unwrap_or(data.len());
                (start.min(data.len()), end.clamp(start, data.len()))
            })
            .collect();

        Ok(Self {
            records,
            data: data.to_vec(),
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the raw payload of record `index`, or `None` when out of range.
    /// Sparse entries are returned as empty slices.
    pub fn record(&self, index: usize) -> Option<&[u8]> {
        self.records
            .get(index)
            .map(|&(start, end)| &self.data[start..end])
    }

    pub fn records(&self) -> impl Iterator<Item = &[u8]> {
        (0..self.len()).map(|i| self.record(i).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(records: &[&[u8]]) -> Vec<u8> {
        let mut data = MAGIC.to_vec();
        data.extend_from_slice(&[0u8; 10]);

        let table_size = (records.len() + 1) * 4;
        let mut offset = table_size as u32;
        for record in records {
            data.extend_from_slice(&offset.to_le_bytes());
            offset += record.len() as u32;
        }
        data.extend_from_slice(&TERMINATOR.to_le_bytes());
        for record in records {
            data.extend_from_slice(record);
        }

        data
    }

    #[test]
    fn reads_records_including_sparse_ones() {
        let data = build(&[b"hello", b"", b"world!"]);
        let gll = GllFile::read(&data).unwrap();

        assert_eq!(gll.len(), 3);
        assert_eq!(gll.record(0).unwrap(), b"hello");
        assert_eq!(gll.record(1).unwrap(), b"");
        assert_eq!(gll.record(2).unwrap(), b"world!");
        assert!(gll.record(3).is_none());
    }

    #[test]
    fn rejects_non_gll_data() {
        assert!(GllFile::read(b"not a gll file at all").is_err());
    }
}
