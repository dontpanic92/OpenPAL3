use crate::{create_reader, plain_fs::PlainFs};
use std::path::Path;

use super::fmb_archive::FmbArchive;

pub type FmbFs = PlainFs<FmbArchive>;

impl FmbFs {
    pub fn create<P: AsRef<Path>>(fmb_path: P) -> anyhow::Result<Self> {
        let reader = create_reader(fmb_path)?;
        let sfb_archive = FmbArchive::load(reader)?;
        Ok(Self::new(sfb_archive))
    }
}
