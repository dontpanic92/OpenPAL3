use std::{
    io::{Cursor, SeekFrom},
    path::Path,
};

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};
use common::read_ext::ReadExt;
use radiance::utils::SeekRead;

use crate::fs::{memory_file::MemoryFile, plain_fs::PlainArchive};

use super::{blowfish, tea::Tea, xtea::XTea};

#[derive(thiserror::Error, Debug)]
pub enum ZpkReadError {
    #[error("Unsupported zpk encryption type: {0}")]
    UnsupportedEncryptionType(u8),

    #[error("Unsupported zpk compression type: {0}")]
    UnsupportedCompressionType(u8),
}

pub struct ZpkArchive {
    reader: Box<dyn SeekRead>,
    header: ZpkHeader,
    entries: Vec<ZpkEntry>,
}

impl ZpkArchive {
    pub fn load(mut reader: Box<dyn SeekRead>) -> anyhow::Result<ZpkArchive> {
        let header = ZpkHeader::read(&mut reader)?;

        let entry_start = reader.read_u32_le()?;
        reader.seek(SeekFrom::Start(entry_start as u64))?;

        let mut entries = vec![];
        let mut xor_key = 0x6fd7;
        loop {
            let entry_size_enc = reader.read_u16::<BigEndian>()?;
            let chunk_size_enc = reader.read_u16::<LittleEndian>()?;
            let entry_size = entry_size_enc ^ xor_key;
            let chunk_size = entry_size_enc ^ chunk_size_enc;
            xor_key = entry_size_enc;

            if entry_size == 0 || chunk_size == 0 {
                break;
            }

            entries.append(&mut ZpkEntry::read(
                &mut reader,
                entry_size,
                chunk_size,
                header.key.clone(),
            )?);
        }

        Ok(Self {
            reader,
            header,
            entries,
        })
    }

    fn decrypt_data(&self, file: &ZpkEntry, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if file.encryption_type == 0 {
            Ok(data.to_vec())
        } else if file.encryption_type == 1 {
            let tea = Tea::new(&self.header.key);
            Ok(tea.decrypt(&data))
        } else if file.encryption_type == 2 {
            let xtea = XTea::new(&self.header.key);
            Ok(xtea.decrypt(&data))
        } else {
            Err(ZpkReadError::UnsupportedEncryptionType(
                file.encryption_type,
            ))?
        }
    }

    fn decompress_data(&self, file: &ZpkEntry, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        if file.compression_type == 0 {
            Ok(data.to_vec())
        } else if file.compression_type == 1 {
            unimplemented!("compression type 1 not implemented for zpk");
        } else if file.compression_type == 2 {
            let mut output = vec![];
            lzma_rs::lzma_decompress(&mut Cursor::new(&data[4..]), &mut output)?;
            Ok(output)
        } else {
            Err(ZpkReadError::UnsupportedCompressionType(
                file.compression_type,
            ))?
        }
    }
}

impl PlainArchive for ZpkArchive {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().to_str().unwrap();

        if let Some(file) = self
            .entries
            .iter()
            .find(|item| item.name == path.to_string())
        {
            self.reader.seek(SeekFrom::Start(file.offset as u64))?;

            let mut content = vec![];

            while content.len() < file.original_size as usize {
                let data_size = self.reader.read_u32::<BigEndian>()?;
                let chunk_size = self.reader.read_u32::<LittleEndian>()?;

                let data = self.reader.read_u8_vec(chunk_size as usize)?;
                let data = self.decrypt_data(file, &data)?;
                let data = self.decompress_data(file, &data)?;

                content.extend_from_slice(&data[0..data_size as usize]);
            }

            Ok(MemoryFile::new(Cursor::new(content)))
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))?
        }
    }

    fn files(&self) -> Vec<String> {
        self.entries.iter().map(|s| s.name.clone()).collect()
    }
}

#[derive(Debug)]
pub struct ZpkHeader {
    key: Vec<u8>,
}

impl ZpkHeader {
    pub fn read(reader: &mut dyn SeekRead) -> anyhow::Result<Self> {
        let _ = reader.seek(SeekFrom::End(-4))?;
        let key_start = reader.read_u32_le()?;
        let key_start = key_start ^ 0x6fd7;

        let _ = reader.seek(SeekFrom::Start(key_start as u64))?;
        let key_len = reader.read_u16_le()?;
        let key = reader.read_u8_vec(key_len as usize)?;

        Ok(Self { key })
    }
}

#[derive(Debug)]
pub struct ZpkEntry {
    pub name: String,
    pub encryption_type: u8,
    pub compression_type: u8,
    pub offset: u32,
    pub original_size: u32,
    pub packed_size: u32,
}

impl ZpkEntry {
    pub fn read(
        reader: &mut dyn SeekRead,
        entry_size: u16,
        chunk_size: u16,
        key: Vec<u8>,
    ) -> anyhow::Result<Vec<Self>> {
        let mut data = reader.read_u8_vec(chunk_size as usize)?;
        let mut output = vec![0; data.len()];

        let bf = blowfish::Blowfish::<BigEndian>::new_from_slice(&key).unwrap();

        for i in 0..(chunk_size as usize + 7) / 8 {
            let p = &mut data[i * 8..(i + 1) * 8];
            p.swap(0, 4);
            p.swap(1, 5);
            p.swap(2, 6);
            p.swap(3, 7);

            let mut o = &mut output[i * 8..(i + 1) * 8];

            bf.decrypt_block_b2b(
                GenericArray::from_slice(&p),
                GenericArray::from_mut_slice(&mut o),
            );
        }

        let mut entries = vec![];
        let mut cursor = Cursor::new(output);
        while cursor.position() < entry_size as u64 {
            entries.push(Self::read_one(&mut cursor)?);
        }

        Ok(entries)
    }

    fn read_one(reader: &mut Cursor<Vec<u8>>) -> anyhow::Result<Self> {
        let len = reader.read_u16_le()?;
        let name = reader.read_string(len as usize)?;
        let encryption_type = reader.read_u8()?;
        let compression_type = reader.read_u8()?;

        let offset = reader.read_u32_le()?;
        let original_size = reader.read_u32_le()?;
        let packed_size = reader.read_u32_le()?;
        Ok(Self {
            name,
            encryption_type,
            compression_type,
            offset,
            original_size,
            packed_size,
        })
    }
}
