use encoding::{DecoderTrap, Encoding};
use mini_fs::{File, StoreExt};
use std::{io, io::BufReader, io::Read, path::Path};

pub trait StoreExt2: mini_fs::StoreExt {
    fn read_to_end<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<u8>>;

    fn read_to_end_from_gbk<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<String, Box<dyn std::error::Error>>;

    fn open_with_fallback<P: AsRef<Path>>(
        &self,
        path: P,
        fallback_ext: &[&str],
    ) -> io::Result<File>;

    fn try_open_files<P: AsRef<Path>>(&self, path: &[P]) -> io::Result<File>;
}

impl StoreExt2 for mini_fs::MiniFs {
    fn open_with_fallback<P: AsRef<Path>>(
        &self,
        path: P,
        fallback_ext: &[&str],
    ) -> io::Result<File> {
        let res = self.open(path.as_ref());
        if res.is_err() {
            for ext in fallback_ext {
                let new_path = path.as_ref().with_extension(ext);
                let res = self.open(new_path);
                if res.is_ok() {
                    return res;
                }
            }
        }

        res
    }

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

    fn try_open_files<P: AsRef<Path>>(&self, paths: &[P]) -> io::Result<File> {
        for p in paths {
            let res: Result<File, io::Error> = self.open(p);
            if res.is_ok() {
                return res;
            }
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "File not found"))
    }
}
