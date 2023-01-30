use std::{io::BufReader, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::mv3::read_mv3;
use mini_fs::{MiniFs, StoreExt};
use opengb::{
    asset_manager::AssetManager,
    scene::{
        create_animated_mesh_from_mv3, create_mv3_entity, RoleAnimationRepeatMode, RoleController,
    },
};
use radiance::comdef::IEntity;

use crate::preview::previewers::{get_extension, jsonify};

use super::ModelLoader;

pub struct Mv3ModelLoader {
    asset_mgr: Rc<AssetManager>,
}

impl Mv3ModelLoader {
    pub fn new(asset_mgr: Rc<AssetManager>) -> Self {
        Self { asset_mgr }
    }
}

impl ModelLoader for Mv3ModelLoader {
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String {
        read_mv3(&mut BufReader::new(vfs.open(&path).unwrap()))
            .map(|f| jsonify(&f))
            .unwrap_or("Unsupported".to_string())
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        extension.as_deref() == Some("mv3")
    }

    fn load(&self, _vfs: &MiniFs, path: &Path) -> ComRc<IEntity> {
        let e = create_mv3_entity(
            self.asset_mgr.clone(),
            "101",
            "preview",
            "preview".to_string(),
            true,
        )
        .unwrap();

        let anim = create_animated_mesh_from_mv3(
            e.clone(),
            &self.asset_mgr.component_factory(),
            self.asset_mgr.vfs(),
            path,
        );

        if let Ok(anim) = anim {
            let controller = RoleController::get_role_controller(e.clone()).unwrap();
            controller.get().play_anim_mesh(
                "preview".to_string(),
                anim,
                RoleAnimationRepeatMode::Repeat,
            );

            controller.get().set_active(true);
        }

        e
    }
}
