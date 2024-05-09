use fileformats::binrw::BinRead;

pub fn run_test() {
    let data =
        std::fs::read("F:\\PAL4\\gamedata\\scenedata\\scenedata\\q01\\N01\\GameObjs.gob").unwrap();
    let mut cursor = std::io::Cursor::new(data);
    let gob = fileformats::pal4::gob::GobFile::read(&mut cursor).unwrap();
    println!("cursor position: {}", cursor.position());

    println!("{}", serde_json::to_string_pretty(&gob).unwrap());
}
