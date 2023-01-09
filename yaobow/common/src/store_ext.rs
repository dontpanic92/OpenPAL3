use encoding::{DecoderTrap, Encoding};
use std::{io, io::BufReader, io::Read, path::Path};

pub trait StoreExt2: mini_fs::StoreExt {
    fn read_to_end<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>>;
    fn read_to_end_from_gbk<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<String, Box<dyn std::error::Error>>;
}

impl StoreExt2 for mini_fs::MiniFs {
    fn read_to_end<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let file = <Self as mini_fs::StoreExt>::open(self, path)?;
        let mut bytes = Vec::new();
        let mut buf_reader = BufReader::new(file);
        buf_reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn read_to_end_from_gbk<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let data = self.read_to_end(path)?;
        Ok(encoding::all::GBK.decode(&data, DecoderTrap::Ignore)?)
    }
}
