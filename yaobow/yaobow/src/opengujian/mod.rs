use std::io::Cursor;

use fileformats::{binrw::BinRead, nif::NifModel, npc::NpcInfoFile};

pub fn test_nif(data: &[u8]) {
    let model = NifModel::read(&mut Cursor::new(data));

    // println!("{}", UnknownBlock::read(&mut cursor));
    println!("{:?}", model);
}

pub fn run_opengujian() {
    // zpk::zpk_test().unwrap();
    // packfs::zpkg::zpkg_test();
    // let data = std::fs::read("F:\\gujian_extracted\\101\\101.nif").unwrap();
    // test_nif(&data);
    let data =
        std::fs::read("F:\\PAL4\\gamedata\\scenedata\\scenedata\\q01\\N01\\npcInfo.npc").unwrap();
    let cam = NpcInfoFile::read(&mut Cursor::new(data));
    println!("cam: {:?}", cam);
}
