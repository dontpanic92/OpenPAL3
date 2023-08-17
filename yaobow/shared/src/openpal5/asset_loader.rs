use std::{collections::HashMap, io::Cursor, rc::Rc};

use common::store_ext::StoreExt2;
use fileformats::{
    binrw::BinRead,
    nod::NodFile,
    role_bin::{AssetItem, RoleBinFile},
};
use mini_fs::MiniFs;
use radiance::rendering::ComponentFactory;

use crate::loaders::Pal5TextureResolver;

pub struct AssetLoader {
    vfs: MiniFs,
    component_factory: Rc<dyn ComponentFactory>,
    index: HashMap<u32, AssetItem>,
    texture_resolver: Pal5TextureResolver,
}

impl AssetLoader {
    pub fn new(component_factory: Rc<dyn ComponentFactory>, vfs: MiniFs) -> Rc<Self> {
        let index = load_index(&vfs);
        Rc::new(Self {
            component_factory,
            vfs,
            index,
            texture_resolver: Pal5TextureResolver {},
        })
    }

    pub fn load_map_nod(&self, map_name: &str) -> anyhow::Result<NodFile> {
        let path = format!("/{}/{}_0_0.nod", map_name, map_name);
        Ok(NodFile::read(&mut Cursor::new(
            self.vfs.read_to_end(&path)?,
        ))?)
    }
}

fn load_index(vfs: &MiniFs) -> HashMap<u32, AssetItem> {
    let index_files = [
        "/Config/role_00.bin",
        "/Config/role_01.bin",
        "/Config/role_02.bin",
        "/Config/role_03.bin",
        "/Config/role_04.bin",
        "/Config/role_05.bin",
    ];

    let mut index = HashMap::new();
    for path in index_files.iter() {
        load_index_single(vfs, path, &mut index).unwrap();
    }

    index
}

fn load_index_single(
    vfs: &MiniFs,
    path: &str,
    index: &mut HashMap<u32, AssetItem>,
) -> anyhow::Result<()> {
    let role_bin = RoleBinFile::read(&mut Cursor::new(vfs.read_to_end(path)?))?;
    for item in role_bin.items {
        index.insert(item.id, item);
    }

    Ok(())
}
