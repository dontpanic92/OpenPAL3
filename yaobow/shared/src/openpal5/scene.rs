use crosscom::ComRc;
use radiance::{
    comdef::IScene,
    math::{Quaternion, Vec3},
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
                let file_path = asset.file_path.to_string();

                // Asset Type?
                if file_path.ends_with(".dff") {
                    println!("{}", file_path);
                    let model = asset_loader.load_model(&file_path)?;
                    model
                        .transform()
                        .borrow_mut()
                        // .rotate_axis_angle_local(&Vec3::UP, node.rotation[0])
                        // .rotate_axis_angle_local(&Vec3::BACK, node.rotation[1])
                        // .rotate_axis_angle_local(&Vec3::EAST, node.rotation[2])
                        .set_position(&Vec3::new(
                            node.position[0],
                            node.position[1],
                            node.position[2],
                        ));
                    scene.add_entity(model);
                }
            }
        }

        Ok(Self { scene })
    }
}
