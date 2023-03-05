use std::{io::Cursor, path::Path};

use common::read_ext::ReadExt;

use crate::fs::{memory_file::MemoryFile, plain_fs::PlainArchive};

use super::tr_cache::TrCacheFile;

#[derive(Debug)]
pub struct ZpkgArchive<T: AsRef<[u8]>> {
    cursor: Cursor<T>,
    tr_cache: TrCacheFile,
}

impl<T: AsRef<[u8]>> ZpkgArchive<T> {
    pub fn load(cursor: Cursor<T>, cache_content: &[u8]) -> anyhow::Result<ZpkgArchive<T>> {
        let tr_cache = TrCacheFile::read(cache_content, "Gef9d(y2f^q0e9%fni2$sd8$0u")?;

        Ok(Self { cursor, tr_cache })
    }

    fn decrypt_data(data: &[u8], cipher_id: u32, key1: &[u8], key2: &[u8]) -> Vec<u8> {
        let mut key = [0u8; 16];
        for i in 0..4 {
            key[i * 4 + 0] = key1[i * 4 + 0] ^ key2[i * 4 + 0];
            key[i * 4 + 1] = key1[i * 4 + 1] ^ key2[i * 4 + 1];
            key[i * 4 + 2] = key1[i * 4 + 2] ^ key2[i * 4 + 2];
            key[i * 4 + 3] = key1[i * 4 + 3] ^ key2[i * 4 + 3];
        }

        let c = super::select_cipher(cipher_id);
        if let Some(c) = c {
            let mut ci = c.setup(&key);
            let output = ci.decrypt(data);

            output
        } else {
            data.to_vec()
        }
    }
}

impl<T: AsRef<[u8]>> PlainArchive for ZpkgArchive<T> {
    fn open<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<MemoryFile> {
        let path = path.as_ref().to_str().unwrap();

        if let Some(file) = self
            .tr_cache
            .entries
            .iter()
            .find(|item| item.filename == path.to_string())
        {
            self.cursor.set_position(file.offset);

            let data = self.cursor.read_u8_vec(file.packed_size as usize)?;
            let data =
                Self::decrypt_data(&data, file.cipher, &file.file_key, &self.tr_cache.zpkg_key);

            // std::fs::write("f:\\zpkg_decrypt.bin", &data).unwrap();

            let data = if file.packed_size == file.unpacked_size {
                data
            } else {
                super::decompress(&data)?
            };

            // std::fs::write("f:\\zpkg_decompressed.bin", &data).unwrap();

            Ok(MemoryFile::new(Cursor::new(data)))
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))?
        }
    }

    fn files(&self) -> Vec<String> {
        self.tr_cache
            .entries
            .iter()
            .map(|s| s.filename.clone())
            .collect()
    }
}
