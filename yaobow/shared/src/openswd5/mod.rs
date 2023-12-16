use encoding::{DecoderTrap, Encoding};
use fileformats::{binrw::BinReaderExt, c00::C00};

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

    unsafe {
        let state = lua50_32_sys::lua_open();

        lua50_32_sys::luaopen_base(state);
        lua50_32_sys::luaopen_table(state);
        lua50_32_sys::luaopen_io(state);
        lua50_32_sys::luaopen_string(state);
        lua50_32_sys::luaopen_math(state);
        lua50_32_sys::luaopen_debug(state);
        lua50_32_sys::luaopen_loadlib(state);

        let ret = lua50_32_sys::luaL_loadbuffer(
            state,
            out.as_ptr() as *const i8,
            out.len(),
            b"main\0".as_ptr() as *const i8,
        );

        println!("ret: {}", ret);

        let call_ret = lua50_32_sys::lcall(state, 0, 0);
        println!("call_ret: {}", call_ret);
        if call_ret > 0 {
            let s = lua50_32_sys::lua_tostring(state, -1);
            let str = std::ffi::CStr::from_ptr(s);
            println!("error: {}", str.to_str().unwrap());
        }

        lua50_32_sys::getglobal(state, b"initiatelua\0".as_ptr() as *const i8);

        let call_ret = lua50_32_sys::lcall(state, 0, 0);
        println!("call_ret2: {}", call_ret);

        if call_ret > 0 {
            let s = lua50_32_sys::lua_tostring(state, -1);
            let str = std::ffi::CStr::from_ptr(s);
            let str = encoding::all::BIG5_2003
                .decode(str.to_bytes(), DecoderTrap::Ignore)
                .unwrap();
            println!("error: {}", str);
        }

        lua50_32_sys::lua_close(state);
    }
}
