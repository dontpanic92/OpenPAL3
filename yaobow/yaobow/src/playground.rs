use std::io::Cursor;

use fileformats::{binrw::BinRead, evf::EvfFile};
pub fn run_test() {
    let data =
        std::fs::read("F:\\PAL4\\gamedata\\scenedata\\scenedata\\q01\\Q01\\Q01.evf").unwrap();
    let evf = EvfFile::read(&mut Cursor::new(data));

    println!("{:?}", evf);
}
