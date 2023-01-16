use std::io::{Cursor, Read, Seek};

use mini_fs::UserFile;

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
