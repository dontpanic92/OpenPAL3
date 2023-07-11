use std::{cell::RefCell, rc::Rc};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use mini_fs::MiniFs;
use radiance::{comdef::IScene, rendering::ComponentFactory, scene::CoreScene};

use crate::{
    loaders::{bsp::create_entity_from_bsp_model, Pal4TextureResolver},
    scripting::angelscript::ScriptModule,
};

pub struct AssetLoader {
    vfs: MiniFs,
    component_factory: Rc<dyn ComponentFactory>,
    texture_resolver: Pal4TextureResolver,
}

impl AssetLoader {
    pub fn new(component_factory: Rc<dyn ComponentFactory>, vfs: MiniFs) -> Rc<Self> {
        Rc::new(Self {
            component_factory,
            vfs,
            texture_resolver: Pal4TextureResolver {},
        })
    }

    pub fn load_script_module(&self, scene: &str) -> anyhow::Result<Rc<RefCell<ScriptModule>>> {
        let content = self
            .vfs
            .read_to_end(&format!("/gamedata/script/{}.csb", scene))?;
        Ok(Rc::new(RefCell::new(
            ScriptModule::read_from_buffer(&content).unwrap(),
        )))
    }

    pub fn load_scene(&self, scene_name: &str, block_name: &str) -> anyhow::Result<ComRc<IScene>> {
        let path = format!(
            "/gamedata/PALWorld/{}/{}/{}.bsp",
            scene_name, block_name, block_name,
        );

        let scene = CoreScene::create();
        let entity = create_entity_from_bsp_model(
            &self.component_factory,
            &self.vfs,
            path,
            "world".to_string(),
            &self.texture_resolver,
        );

        scene.add_entity(entity);
        Ok(scene)
    }
}
