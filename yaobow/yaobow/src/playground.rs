use fileformats::binrw::BinRead;

pub fn run_test() {
    let data =
        std::fs::read("F:\\SteamLibrary\\steamapps\\common\\SWDHC\\Map\\S5a_03_4.map").unwrap();
    let map = fileformats::swd5::map::Map::read(&mut std::io::Cursor::new(data)).unwrap();

    println!("{:?}", map);
}
