use mini_fs::{Entries, Store};
use std::{
    cell::RefCell,
    path::Path,
    sync::{Arc, Mutex},
};

use super::YpkArchive;

pub struct YpkFs {
    archive: RefCell<YpkArchive>,
}

impl YpkFs {
    pub fn new<P: AsRef<Path>>(ypk_path: P) -> anyhow::Result<YpkFs> {
        let file = std::fs::File::open(ypk_path.as_ref())?;
        let archive = RefCell::new(YpkArchive::load(Arc::new(Mutex::new(file)))?);
        Ok(YpkFs { archive })
    }
}

impl Store for YpkFs {
    type File = mini_fs::File;

    fn open_path(&self, path: &Path) -> std::io::Result<Self::File> {
        self.archive.borrow_mut().open(path.to_str().unwrap())
    }

    fn entries_path(&self, _: &Path) -> std::io::Result<Entries> {
        unimplemented!();
    }
}
