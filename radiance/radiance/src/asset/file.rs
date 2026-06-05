//! `mini_fs::UserFile` implementations used by [`crate::asset`] stores.
//!
//! - [`MemoryFile`] wraps a `Cursor<Vec<u8>>` so a fully-decompressed
//!   entry can be returned from an archive store.
//! - [`StreamingFile`] adapts a shared, seekable backing reader into a
//!   bounded `Read+Seek` view, so an archive can hand out independent
//!   file handles that share one underlying file descriptor.
//!
//! These are intentional verbatim copies of the equivalents in
//! `packfs/src/{memory_file,streaming_file}.rs`. Keeping a local copy
//! lets `radiance` host its own asset formats (currently `ypk`)
//! without depending on `packfs`; `packfs` keeps its copies for the
//! game-specific formats it still owns (`cpk`, `pkg`, ...). The two
//! copies have no behavioural divergence.

use std::{
    io::{Cursor, Read, Seek},
    sync::{Arc, Mutex},
};

use mini_fs::UserFile;

use super::seek_traits::SeekRead;

pub struct MemoryFile {
    cursor: Cursor<Vec<u8>>,
}

impl MemoryFile {
    pub fn new(cursor: Cursor<Vec<u8>>) -> MemoryFile {
        MemoryFile { cursor }
    }

    pub fn content(&self) -> Vec<u8> {
        self.cursor.clone().into_inner()
    }
}

impl UserFile for MemoryFile {}

impl Read for MemoryFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Seek for MemoryFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(pos)
    }
}

pub struct StreamingFile {
    reader: Arc<Mutex<dyn SeekRead + Send + Sync>>,
    position: u64,
    start_position: u64,
    end_position: u64,
}

impl StreamingFile {
    pub fn new(
        reader: Arc<Mutex<dyn SeekRead + Send + Sync>>,
        start_position: u64,
        end_position: u64,
    ) -> StreamingFile {
        Self {
            reader,
            position: start_position,
            start_position,
            end_position,
        }
    }
}

impl UserFile for StreamingFile {}

impl Read for StreamingFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.end_position {
            return Ok(0);
        }

        let mut reader = self.reader.lock().unwrap();
        reader.seek(std::io::SeekFrom::Start(self.position))?;
        let read = reader.read(buf)?;
        let read = read.min((self.end_position - self.position) as usize);
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for StreamingFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let new_position = match pos {
            std::io::SeekFrom::Start(offset) => self.start_position + offset,
            std::io::SeekFrom::End(offset) => (self.end_position as i64 + offset) as u64,
            std::io::SeekFrom::Current(offset) => (self.position as i64 + offset) as u64,
        };

        if new_position < self.start_position {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek before start",
            ));
        }

        if new_position > self.end_position {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek after end",
            ));
        }

        self.position = new_position;
        Ok(self.position - self.start_position)
    }
}
