pub fn test() {
    let data =
        std::fs::read("F:\\SteamLibrary\\steamapps\\common\\SWDHC\\Texture\\Texture_20080310.tbl")
            .unwrap();
    let data: Vec<u8> = data.into_iter().skip(8).collect();

    std::fs::write("f:\\tbl.input.bin", &data).unwrap();

    let output = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&data, 0x21c).unwrap();

    std::fs::write("f:\\tbl.bin", output).unwrap();
}
