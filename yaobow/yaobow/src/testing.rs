use std::io::Cursor;

use fileformats::{amf::AmfFile, binrw::BinRead};

pub fn run_test() {
    let data = std::fs::read(
        //"F:\SteamLibrary\\steamapps\\common\\Chinese Paladin 5\\Map\\kuangfengzhai\\kuangfengzhai_0_0.nod",
        "F:\\PAL4\\gamedata\\PALActor\\101\\C07.amf",
    )
    .unwrap();

    let file = AmfFile::read(&mut Cursor::new(data));
    println!("file: {:?}", file);
    /*let models: Vec<String> = file
        .unwrap()
        .nodes
        .iter()
        .map(|m| {
            let x: Vec<u8> = m
                .name
                .iter()
                .take_while(|x| **x != 0)
                .cloned()
                .collect();
            String::from_utf8_lossy(&x).to_string()
        })
        .collect();

    println!("models: {:?}", models);*/
}
