pub mod asset_loader;
pub mod comdef;
pub mod director;
pub mod scripting;

use std::{io::Cursor, rc::Rc};

use common::store_ext::StoreExt2;
use fileformats::{
    binrw::{BinRead, BinReaderExt},
    c00::C00,
};
use mini_fs::MiniFs;

use crate::{fs::init_virtual_fs, scripting::lua50_32::Lua5032Vm, GameType};

use self::scripting::create_lua_vm;

pub fn test() {
    let game = GameType::SWDHC;
    let vfs = init_virtual_fs("F:\\SteamLibrary\\steamapps\\common\\SWDHC", None);
    let asset_loader = asset_loader::AssetLoader::new(vfs, game);
    // let script = asset_loader.load_main_script().unwrap();
    // let vm = Lua5032Vm::new(script, "initiatelua").unwrap();
    let vm = create_lua_vm(&asset_loader).unwrap();

    vm.execute().unwrap();
    println!("4");
}
