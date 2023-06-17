use std::{cell::RefCell, rc::Rc};

use common::store_ext::StoreExt2;
use mini_fs::MiniFs;

use crate::scripting::angelscript::ScriptModule;

pub struct AssetLoader {
    vfs: MiniFs,
}

impl AssetLoader {
    pub fn new(vfs: MiniFs) -> Rc<Self> {
        Rc::new(Self { vfs })
    }

    pub fn load_script_module(&self, scene: &str) -> anyhow::Result<Rc<RefCell<ScriptModule>>> {
        let content = self
            .vfs
            .read_to_end(&format!("/gamedata/script/{}.csb", scene))?;
        Ok(Rc::new(RefCell::new(
            ScriptModule::read_from_buffer(&content).unwrap(),
        )))
    }
}
