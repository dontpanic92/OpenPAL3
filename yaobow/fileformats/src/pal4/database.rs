//! PAL4's self-describing `GAME_DB_FLAG` databases from `database.cpk`.
//! Types and enum references come from the file, not a hardcoded gameplay schema.

use anyhow::{Context, Result, bail, ensure};
use encoding::{DecoderTrap, Encoding};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Database {
    pub enums: Vec<EnumDefinition>,
    pub tables: Vec<Table>,
}

#[derive(Debug, Serialize)]
pub struct EnumDefinition {
    pub name: String,
    pub description: String,
    pub values: Vec<EnumValue>,
}

#[derive(Debug, Serialize)]
pub struct EnumValue {
    pub value: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Column {
    pub name: String,
    pub data_type: u32,
    pub enum_name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct Table {
    pub name: String,
    pub stored_name: String,
    pub description: String,
    /// Preserved header word; zero in the inspected databases.
    pub metadata: u32,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Value {
    Integer(i32),
    Text(String),
    Float(f32),
}

impl Database {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader { bytes, offset: 0 };
        ensure!(
            reader.string()? == "GAME_DB_FLAG",
            "invalid PAL4 database magic"
        );
        reader.marker(0x01321844)?;
        let enum_count = reader.count()?;
        let mut enums = Vec::new();
        for _ in 0..enum_count {
            let name = reader.string()?;
            reader.marker(0x0132188f)?;
            let description = reader.string()?;
            let value_count = reader.count()?;
            let mut values = Vec::new();
            for _ in 0..value_count {
                values.push(EnumValue {
                    value: reader.u32()? as i32,
                    name: reader.string()?,
                });
            }
            reader.marker(0x01321890)?;
            enums.push(EnumDefinition {
                name,
                description,
                values,
            });
        }
        let table_count = reader.count()?;
        let mut tables = Vec::new();
        for _ in 0..table_count {
            let name = reader.string()?;
            let table = read_table(&mut reader, name.clone())
                .with_context(|| format!("table {name} at {:#x}", reader.offset))?;
            tables.push(table);
        }
        ensure!(
            reader.offset == bytes.len(),
            "trailing PAL4 database bytes at {:#x}",
            reader.offset
        );
        Ok(Self { enums, tables })
    }
}

fn read_table(reader: &mut Reader<'_>, name: String) -> Result<Table> {
    reader.marker(0x01321841)?;
    let stored_name = reader.string()?;
    let description = reader.string()?;
    let metadata = reader.u32()?;
    let column_count = reader.count()?;
    let row_count = reader.count()?;
    let mut columns = Vec::new();
    for _ in 0..column_count {
        let column = Column {
            name: reader.string()?,
            data_type: reader.u32()?,
            enum_name: reader.string()?,
            description: reader.string()?,
        };
        ensure!(
            (1..=3).contains(&column.data_type),
            "unsupported type {} in column {} at {:#x}",
            column.data_type,
            column.name,
            reader.offset
        );
        columns.push(column);
    }

    let mut rows = Vec::new();
    for row_index in 0..row_count {
        reader.marker(0x0132183f)?;
        let mut row = Vec::new();
        for column in &columns {
            let value = match column.data_type {
                1 => Value::Integer(reader.u32()? as i32),
                2 => Value::Text(reader.string()?),
                3 => Value::Float(f32::from_bits(reader.u32()?)),
                data_type => bail!(
                    "unsupported type {data_type} in column {}, row {row_index} at {:#x}",
                    column.name,
                    reader.offset
                ),
            };
            row.push(value);
        }
        reader.marker(0x01321840)?;
        rows.push(row);
    }
    reader.marker(0x01321842)?;
    Ok(Table {
        name,
        stored_name,
        description,
        metadata,
        columns,
        rows,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8]> {
        let remaining = &self.bytes[self.offset..];
        let bytes = remaining.get(..len).with_context(|| {
            format!(
                "truncated PAL4 database at {:#x}, need {len} bytes",
                self.offset
            )
        })?;
        self.offset += len;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }

    fn count(&mut self) -> Result<usize> {
        let count = self.u32()? as usize;
        ensure!(
            count <= (self.bytes.len() - self.offset) / 4,
            "invalid count {count} at {:#x}",
            self.offset - 4
        );
        Ok(count)
    }

    fn string(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        encoding::all::GBK
            .decode(self.take(len)?, DecoderTrap::Strict)
            .map_err(|e| anyhow::anyhow!("invalid GBK database string: {e}"))
    }

    fn marker(&mut self, expected: u32) -> Result<()> {
        let offset = self.offset;
        let actual = self.u32()?;
        ensure!(
            actual == expected,
            "expected marker {expected:#010x}, got {actual:#010x} at {offset:#x}"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn string(bytes: &mut Vec<u8>, value: &[u8]) {
        word(bytes, value.len() as u32);
        bytes.extend_from_slice(value);
    }

    fn fixture(data_type: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        string(&mut bytes, b"GAME_DB_FLAG");
        word(&mut bytes, 0x01321844);
        word(&mut bytes, 1);
        string(&mut bytes, b"Kind");
        word(&mut bytes, 0x0132188f);
        string(&mut bytes, b"category");
        word(&mut bytes, 1);
        word(&mut bytes, 7);
        string(&mut bytes, b"example");
        word(&mut bytes, 0x01321890);
        word(&mut bytes, 1);
        string(&mut bytes, b"Sample");
        word(&mut bytes, 0x01321841);
        string(&mut bytes, b"Sample");
        string(&mut bytes, b"synthetic table");
        word(&mut bytes, 0);
        word(&mut bytes, 3);
        word(&mut bytes, 1);
        for (name, kind, enum_name) in [
            (b"ID".as_slice(), data_type, b"Kind".as_slice()),
            (b"NAME", 2, b""),
            (b"RATE", 3, b""),
        ] {
            string(&mut bytes, name);
            word(&mut bytes, kind);
            string(&mut bytes, enum_name);
            string(&mut bytes, b"");
        }
        word(&mut bytes, 0x0132183f);
        word(&mut bytes, (-7i32) as u32);
        string(&mut bytes, &[0xb2, 0xe2, 0xca, 0xd4]);
        word(&mut bytes, (-0.25f32).to_bits());
        word(&mut bytes, 0x01321840);
        word(&mut bytes, 0x01321842);
        bytes
    }

    #[test]
    fn reads_schema_enums_gbk_and_typed_rows() {
        let database = Database::read(&fixture(1)).unwrap();
        let enumeration = &database.enums[0];
        assert_eq!(enumeration.name, "Kind");
        assert_eq!(enumeration.values[0].value, 7);
        assert_eq!(enumeration.values[0].name, "example");
        let table = &database.tables[0];
        assert_eq!(table.name, "Sample");
        assert_eq!(table.columns[0].enum_name, "Kind");
        assert!(table.columns[1].enum_name.is_empty());
        let row = &table.rows[0];
        assert!(matches!(row[0], Value::Integer(-7)));
        assert!(matches!(&row[1], Value::Text(value) if value == "\u{6d4b}\u{8bd5}"));
        assert!(matches!(row[2], Value::Float(value) if value == -0.25));
    }

    #[test]
    fn rejects_truncated_corrupt_and_trailing_data() {
        let bytes = fixture(1);
        for end in 0..bytes.len() {
            assert!(Database::read(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut corrupt = bytes.clone();
        corrupt[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(Database::read(&corrupt).is_err());
        let mut corrupt = bytes.clone();
        corrupt[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Database::read(&corrupt).is_err());
        let mut corrupt = bytes.clone();
        corrupt[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Database::read(&corrupt).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(Database::read(&trailing).is_err());
    }

    #[test]
    fn rejects_unknown_column_types_instead_of_guessing() {
        for data_type in [0, 4, u32::MAX] {
            let error = Database::read(&fixture(data_type)).unwrap_err();
            assert!(format!("{error:#}").contains("unsupported type"));
        }
    }
}
