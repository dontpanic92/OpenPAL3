use std::{
    io::{Read, Seek},
    sync::{Arc, Mutex},
};

use common::SeekRead;
use mini_fs::UserFile;

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
