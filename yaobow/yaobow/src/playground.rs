pub fn run_test() {
    // shared::playground::test();
    let atp = fileformats::atp::AtpFile::read(
        &std::fs::read("F:\\SteamLibrary\\steamapps\\common\\SWDHC\\ACT\\00000010.atp").unwrap(),
    )
    .unwrap();

    // std::fs::write("f:\\test.atp", &atp.files[0].as_ref().unwrap()).unwrap();
    // println!("{:?}", atp);
}
