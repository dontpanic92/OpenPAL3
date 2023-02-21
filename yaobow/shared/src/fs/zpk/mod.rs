use crate::fs::plain_fs::PlainArchive;

use self::zpk_archive::{ZpkArchive, ZpkHeader};

mod blowfish;
mod consts;
mod tea;
mod xtea;
mod zpk_archive;
pub mod zpk_fs;

/**
 * References:
 *      https://github.com/xurubin/GuJianUnpack
 */

pub fn zpk_test() -> anyhow::Result<()> {
    let file = std::fs::File::open("F:\\SteamLibrary\\steamapps\\common\\Gujian\\Data\\Music.zpk")?;
    let mem = unsafe { memmap::MmapOptions::new().map(&file)? };
    let cursor = std::io::Cursor::new(mem);
    let mut archive = ZpkArchive::load(cursor)?;
    let f = archive.open("p71.ogg")?;

    println!("good");
    std::fs::write("f:\\p71.ogg", f.content()).unwrap();

    println!("{:?}", archive);
    Ok(())
}
