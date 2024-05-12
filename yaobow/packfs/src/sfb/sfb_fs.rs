use crate::{create_reader, plain_fs::PlainFs};
use std::path::Path;

use super::sfb_archive::SfbArchive;

pub type SfbFs = PlainFs<SfbArchive>;

impl SfbFs {
    pub fn create<P: AsRef<Path>>(sfb_path: P) -> anyhow::Result<SfbFs> {
        let reader = create_reader(sfb_path)?;
        let sfb_archive = SfbArchive::load(reader)?;
        Ok(Self::new(sfb_archive))
    }
}
