use std::{collections::HashMap, io::Cursor, rc::Rc};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use fileformats::{
    binrw::BinRead,
    nod::NodFile,
    role_bin::{AssetItem, RoleBinFile},
};
use mini_fs::MiniFs;
use radiance::{comdef::IEntity, rendering::ComponentFactory};

use crate::loaders::{
    dff::{create_entity_from_dff_model, DffLoaderConfig},
    Pal5TextureResolver,
};

pub struct AssetLoader {
    vfs: Rc<MiniFs>,
    component_factory: Rc<dyn ComponentFactory>,
    pub index: HashMap<u32, AssetItem>,
    texture_resolver: Pal5TextureResolver,
}

impl AssetLoader {
    pub fn new(component_factory: Rc<dyn ComponentFactory>, vfs: Rc<MiniFs>) -> Rc<Self> {
        let index = load_index(&vfs);
        Rc::new(Self {
            component_factory,
            vfs,
            index,
            texture_resolver: Pal5TextureResolver {},
        })
    }

    pub fn load_map_nod(&self, map_name: &str) -> anyhow::Result<NodFile> {
        let path = format!("/Map/{}/{}_0_0.nod", map_name, map_name);
        Ok(NodFile::read(&mut Cursor::new(
            self.vfs.read_to_end(&path)?,
        ))?)
    }

    pub fn load_model(&self, model_path: &str) -> anyhow::Result<ComRc<IEntity>> {
        let model_path = format!("/Model/{}", model_path);
        let entity = create_entity_from_dff_model(
            &self.component_factory,
            &self.vfs,
            model_path.clone(),
            model_path,
            true,
            &DffLoaderConfig {
                texture_resolver: &self.texture_resolver,
                keep_right_to_render_only: false,
            },
        );
        Ok(entity)
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
        match load_index_single(vfs, path, &mut index) {
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to load index file {}: {}", path, e);
            }
        }
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
