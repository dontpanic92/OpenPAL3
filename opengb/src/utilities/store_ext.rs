use std::{io, io::BufReader, io::Read, path::Path};

pub trait StoreExt2: mini_fs::StoreExt {
    fn read_to_end<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>>;
}

impl StoreExt2 for mini_fs::MiniFs {
    fn read_to_end<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let file = <Self as mini_fs::StoreExt>::open(self, path)?;
        let mut bytes = Vec::new();
        let mut buf_reader = BufReader::new(file);
        buf_reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
