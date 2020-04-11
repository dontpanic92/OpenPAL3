use opengb::loaders::cvd_loader::*;
use opengb::loaders::pol_loader::*;
use opengb::loaders::scn_loader::*;
use opengb::scene::load_scene;
use opengb::scene::CvdModelEntity;
use opengb::scene::Mv3ModelEntity;
use opengb::scene::PolModelEntity;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene, SceneExtension};
use std::path::PathBuf;

pub struct ScnScene {
    path: String,
    scn_file: ScnFile,
}

impl SceneExtension<ScnScene> for ScnScene {
    fn on_loading(&mut self, scene: &mut CoreScene<ScnScene>) {
        scene
            .camera_mut()
            .transform_mut()
            .translate_local(&Vec3::new(0., 400., 1000.));

        load_scene(scene, &self.path, &self.scn_file, true);
    }

    fn on_updating(&mut self, scene: &mut CoreScene<ScnScene>, delta_sec: f32) {
        scene.camera_mut().transform_mut().rotate_axis_angle(
            &Vec3::new(0., 1., 0.),
            0.2 * delta_sec * std::f32::consts::PI,
        );
    }
}

impl ScnScene {
    pub fn new(path: String) -> Self {
        let scn_file = scn_load_from_file(&path);
        println!("{:?}", scn_file);
        Self { path, scn_file }
    }
}
