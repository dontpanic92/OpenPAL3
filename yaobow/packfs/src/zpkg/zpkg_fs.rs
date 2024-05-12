use std::path::{Path, PathBuf};

use crate::{create_reader, plain_fs::PlainFs};

use super::zpkg_archive::ZpkgArchive;

pub type ZpkgFs = PlainFs<ZpkgArchive>;

impl ZpkgFs {
    pub fn create<P: AsRef<Path>>(zpkg_path: P) -> anyhow::Result<ZpkgFs> {
        let cache_path = map_cache_path(zpkg_path.as_ref())?;
        let cache_content = std::fs::read(cache_path)?;

        let reader = create_reader(zpkg_path)?;
        let archive = ZpkgArchive::load(reader, &cache_content)?;
        Ok(Self::new(archive))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ZpkgReadError {
    #[error("Unsupported zpkg file name: {0}")]
    UnknownZpkgFileName(PathBuf),

    #[error("Gujian2 folder structure incoorect: {0}")]
    UnknownFolderStructure(PathBuf),
}

fn map_cache_path(zpkg_path: &Path) -> anyhow::Result<PathBuf> {
    let filename = zpkg_path
        .file_name()
        .ok_or(ZpkgReadError::UnknownZpkgFileName(zpkg_path.to_owned()))?
        .to_ascii_lowercase()
        .to_string_lossy()
        .to_string();
    let folder = zpkg_path
        .parent()
        .ok_or(ZpkgReadError::UnknownFolderStructure(zpkg_path.to_owned()))?;
    match filename.as_str() {
        "base.zpkg" => Ok(folder.join("Bin/TRGameCache.dll")),
        "creatures.zpkg" => Ok(folder.join("Bin/TRCharCache.dll")),
        "models.zpkg" => Ok(folder.join("Bin/TRWorldCache.dll")),
        "gamedata.zpkg" => Ok(folder.join("Bin/TRLogicalCache.dll")),
        "contents1.zpkg" => Ok(folder.join("Bin/TRCaveCache.dll")),
        "contents2.zpkg" => Ok(folder.join("Bin/TRAnimCache.dll")),
        _ => Err(ZpkgReadError::UnknownZpkgFileName(zpkg_path.to_owned()))?,
    }
}
