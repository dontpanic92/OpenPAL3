use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;

#[derive(Debug)]
pub struct TrCacheFile {
    unknown: u32,
    pub zpkg_key: [u8; 16],
    pub entries: Vec<TrCacheFileEntry>,
}

impl TrCacheFile {
    pub fn read(buffer: &[u8], key: &str) -> anyhow::Result<Self> {
        let mut key = Vec::from(key.as_bytes());
        key.push(0);

        let mut cipher = super::select_cipher(7).unwrap().setup(&key);
        let decrypted = cipher.decrypt(&buffer);
        let content = super::decompress(&decrypted)?;

        let mut reader = Cursor::new(content);
        let unknown = reader.read_u32_le()?;
        let zpkg_key = reader.read_u8_vec(0x10)?.try_into().unwrap();
        let mut entries = vec![];

        loop {
            let s = reader.read_i16::<LittleEndian>()?;
            if s < 0 {
                break;
            }

            reader.set_position(reader.position() - 2);
            entries.push(TrCacheFileEntry::read(&mut reader)?);
        }

        Ok(Self {
            unknown,
            zpkg_key,
            entries,
        })
    }
}

#[derive(Debug)]
pub struct TrCacheFileEntry {
    pub filename: String,
    pub offset: u64,
    pub packed_size: u64,
    unknown1: u64,
    pub unpacked_size: u64,
    pub cipher: u32,
    unknown4: u32,
    pub file_key: [u8; 16],
}

impl TrCacheFileEntry {
    pub fn read(reader: &mut dyn Read) -> anyhow::Result<Self> {
        let name_len = reader.read_u16_le()?;
        let filename = reader.read_string(name_len as usize)?;
        let offset = reader.read_u64::<LittleEndian>()?;
        let packed_size = reader.read_u64::<LittleEndian>()?;
        let unknown1 = reader.read_u64::<LittleEndian>()?;
        let unpacked_size = reader.read_u64::<LittleEndian>()?;
        let cipher = reader.read_u32::<LittleEndian>()?;
        let unknown4 = reader.read_u32::<LittleEndian>()?;
        let file_key = reader.read_u8_vec(0x10)?.try_into().unwrap();

        Ok(Self {
            filename,
            offset,
            packed_size,
            unknown1,
            unpacked_size,
            cipher,
            unknown4,
            file_key,
        })
    }
}
