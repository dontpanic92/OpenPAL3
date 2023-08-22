use std::io::Cursor;

use fileformats::{binrw::BinRead, role_bin::RoleBinFile};

pub fn run_test() {
    let data = std::fs::read(
        //"F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 5\\Map\\kuangfengzhai\\kuangfengzhai_0_0.nod",
        "F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 5\\Config\\role_00.bin",
    )
    .unwrap();

    let file = RoleBinFile::read(&mut Cursor::new(data));
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
