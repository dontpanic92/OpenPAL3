use super::mv3entity::Mv3ModelEntity;
use super::polentity::PolModelEntity;
use opengb::loaders::polloader::*;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, SceneCallbacks};

pub struct ModelViewerScene {
    pub path: String,
}

impl SceneCallbacks for ModelViewerScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        if self.path.to_lowercase().ends_with(".mv3") {
            let mut entity = CoreEntity::new(Mv3ModelEntity::new(&self.path));
            entity
                .transform_mut()
                .translate(&Vec3::new(0., -40., -100.));
            scene.add_entity(entity);
        } else if self.path.to_lowercase().ends_with(".pol") {
            let pol = pol_load_from_file(&self.path).unwrap();
            for mesh in &pol.meshes {
                for material in &mesh.material_info {
                    let mut entity =
                        CoreEntity::new(PolModelEntity::new(&mesh.vertices, material, &self.path));
                    entity
                        .transform_mut()
                        .translate(&Vec3::new(0., -400., -1000.));
                    scene.add_entity(entity)
                }
            }
        } else {
            panic!("Not supported file format");
        }
    }
}
