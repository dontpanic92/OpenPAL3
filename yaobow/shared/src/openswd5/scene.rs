use crosscom::ComRc;
use radiance::comdef::IScene;

use super::asset_loader::AssetLoader;

pub struct Swd5Scene {
    pub scene: ComRc<IScene>,
}

impl Swd5Scene {
    pub fn load(asset_loader: &AssetLoader, map_id: i32) -> anyhow::Result<Self> {
        let fld = asset_loader.load_fld(map_id)?;
        let map = asset_loader.load_map(fld.map_file.to_string())?;
        let scene = asset_loader.load_scene_dff(&map.model_chunk.model_file.to_string())?;
        scene.camera().borrow_mut().set_fov43(60_f32.to_radians());

        Ok(Self { scene })
    }
}
