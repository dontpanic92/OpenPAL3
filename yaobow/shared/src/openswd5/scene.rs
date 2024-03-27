use crosscom::ComRc;
use radiance::{comdef::IScene, math::Vec3};

use super::asset_loader::AssetLoader;

pub struct Swd5Scene {
    pub scene: ComRc<IScene>,
    pub camera_look_at: Vec3,
    pub camera_position: Vec3,
}

impl Swd5Scene {
    pub fn load(asset_loader: &AssetLoader, map_id: i32) -> anyhow::Result<Self> {
        let fld = asset_loader.load_fld(map_id)?;
        let map = asset_loader.load_map(fld.map_file.to_string())?;
        let scene = asset_loader.load_scene_dff(&map.model_chunk.model_file.to_string())?;
        scene.camera().borrow_mut().set_fov43(60_f32.to_radians());

        Ok(Self {
            scene,
            camera_look_at: Vec3::new(0., 0., 0.),
            camera_position: Vec3::new(0., 0., 0.),
        })
    }

    pub fn set_camera_delta(&mut self, dx: f32, dy: f32, dz: f32) {
        self.scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&self.camera_look_at)
            .look_at(&Vec3::new(
                self.camera_look_at.x,
                self.camera_look_at.y,
                self.camera_look_at.z + 1.,
            ))
            .rotate_axis_angle_local(&Vec3::UP, -dx.to_radians())
            .rotate_axis_angle_local(&Vec3::EAST, -dy.to_radians())
            .translate_local(&Vec3::new(0., 0., dz));

        self.camera_position = self.scene.camera().borrow().transform().position();
    }

    pub fn set_camera_lookat(&mut self, x: f32, y: f32, z: f32) {
        self.camera_look_at = Vec3::new(x, y, z);
        self.scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .look_at(&self.camera_look_at);
    }

    pub fn set_camera_pos(&mut self, x: f32, y: f32, z: f32) {
        self.camera_position = Vec3::new(x, y, z);
        self.scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&self.camera_position)
            .look_at(&self.camera_look_at);
    }
}
