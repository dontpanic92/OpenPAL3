use crosscom::ComRc;
use radiance::{
    comdef::IScene,
    scene::{CoreEntity, CoreScene},
};

use super::asset_loader::AssetLoader;

pub struct Pal5Scene {
    pub scene: ComRc<IScene>,
}

impl Pal5Scene {
    pub fn new_empty() -> Self {
        Self {
            scene: CoreScene::create(),
        }
    }

    pub fn load(asset_loader: &AssetLoader, scene_name: &str) -> anyhow::Result<Self> {
        let scene = CoreScene::create();
        scene.camera().borrow_mut().set_fov43(45_f32.to_radians());

        let nod = asset_loader.load_map_nod(scene_name)?;

        for node in &nod.nodes {
            let asset = asset_loader.index.get(&node.asset_id);
            if let Some(asset) = asset {
                println!("{:?}", asset);
                let file_path = asset.file_path.to_string();

                // Asset Type?
                if file_path.ends_with(".dff") {
                    let model = asset_loader.load_model(&file_path)?;
                    scene.add_entity(model);
                }
            }
        }

        Ok(Self { scene })
    }
}
