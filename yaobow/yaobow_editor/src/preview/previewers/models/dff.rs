use std::{io::Read, path::Path, rc::Rc};

use crosscom::ComRc;
use fileformats::rwbs::read_dff;
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::{IArmatureComponent, IComponent, IEntity};
use shared::{
    loaders::{
        anm::load_anm,
        dff::{create_entity_from_dff_model, DffLoaderConfig},
        Pal4TextureResolver,
    },
    openpal3::asset_manager::AssetManager,
    openpal4::{
        actor::{Pal4ActorAnimationConfig, Pal4ActorAnimationController},
        comdef::IPal4ActorAnimationController,
    },
    GameType,
};

use crate::preview::previewers::{get_extension, jsonify};

use super::ModelLoader;

pub struct DffModelLoader {
    asset_mgr: Rc<AssetManager>,
    game_type: GameType,
}

impl DffModelLoader {
    pub fn new(asset_mgr: Rc<AssetManager>, game_type: GameType) -> Self {
        Self {
            asset_mgr,
            game_type,
        }
    }
}

impl ModelLoader for DffModelLoader {
    fn load_text(&self, vfs: &MiniFs, path: &Path) -> String {
        if get_extension(path).as_deref() == Some("dff") {
            let mut buf = vec![];
            _ = vfs.open(&path).unwrap().read_to_end(&mut buf);
            read_dff(&buf)
                .map(|f| jsonify(&f))
                .unwrap_or("Unsupported".to_string())
        } else {
            load_anm(vfs, path)
                .map(|f| jsonify(&f))
                .unwrap_or("Unsupported".to_string())
        }
    }

    fn is_supported(&self, path: &Path) -> bool {
        let extension = get_extension(path);
        let extension = extension.as_deref();
        extension == Some("dff") || extension == Some("anm")
    }

    fn load(&self, vfs: &MiniFs, path: &Path) -> ComRc<IEntity> {
        if get_extension(path).as_deref() == Some("dff") {
            create_entity_from_dff_model(
                &self.asset_mgr.component_factory(),
                vfs,
                path,
                "preview".to_string(),
                true,
                &DffLoaderConfig {
                    texture_resolver: &Pal4TextureResolver {},
                    keep_right_to_render_only: false,
                }, //self.game_type.dff_loader_config().unwrap(),
            )
        } else {
            let folder_path = path.parent().unwrap();
            let actor_name = folder_path.file_name().unwrap().to_str().unwrap();
            let dff_path = folder_path.join(format!("{}.dff", actor_name));
            let entity = create_entity_from_dff_model(
                &self.asset_mgr.component_factory(),
                vfs,
                dff_path,
                "preview".to_string(),
                true,
                &DffLoaderConfig {
                    texture_resolver: &Pal4TextureResolver {},
                    keep_right_to_render_only: false,
                },
            );

            let armature = entity
                .get_component(IArmatureComponent::uuid())
                .unwrap()
                .query_interface::<IArmatureComponent>()
                .unwrap();

            let controller = Pal4ActorAnimationController::create(armature);
            entity.add_component(
                IPal4ActorAnimationController::uuid(),
                controller.query_interface::<IComponent>().unwrap(),
            );

            let anm = load_anm(self.asset_mgr.vfs(), path).unwrap_or(vec![]);
            controller.play_animation(anm, vec![], Pal4ActorAnimationConfig::Looping);

            entity
        }
    }
}
