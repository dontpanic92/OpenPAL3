use std::io::Cursor;

use binrw::BinRead;
use fileformats::{binrw::BinReaderExt, c00::C00};

use crate::scripting::lua50::chunk::LuaChunk;

pub fn test() {
    let data =
        std::fs::read("F:\\SteamLibrary\\steamapps\\common\\SWD5\\Script\\0000.C01").unwrap();

    let file =
        std::fs::File::open("F:\\SteamLibrary\\steamapps\\common\\SWD5\\Script\\0000.C01").unwrap();

    let mut reader = std::io::BufReader::new(file);
    let c00: C00 = reader.read_le().unwrap();

    let lzo: minilzo_rs::LZO = minilzo_rs::LZO::init().unwrap();
    let out = lzo
        .decompress(&c00.data, c00.header.original_size as usize)
        .unwrap();

    let chunk = LuaChunk::read(&mut Cursor::new(out)).unwrap();
    println!("{:?}", chunk);

    /*let lua = Lua::new();
    lua.context(|ctx| {
        ctx.load(&output_bytes).exec().unwrap();
    })*/
}
