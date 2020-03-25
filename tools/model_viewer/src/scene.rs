use opengb::scene::CvdModelEntity;
use opengb::scene::Mv3ModelEntity;
use opengb::scene::PolModelEntity;
use opengb::loaders::cvdloader::*;
use opengb::loaders::polloader::*;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene, SceneCallbacks};

pub struct ModelViewerScene {
    pub path: String,
}

impl SceneCallbacks for ModelViewerScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        scene
            .camera_mut()
            .transform_mut()
            .translate_local(&Vec3::new(0., 200., 400.));

        if self.path.to_lowercase().ends_with(".mv3") {
            let entity = CoreEntity::new(Mv3ModelEntity::new(&self.path));
            scene.add_entity(entity);
        } else if self.path.to_lowercase().ends_with(".pol") {
            let pol = pol_load_from_file(&self.path).unwrap();
            for mesh in &pol.meshes {
                for material in &mesh.material_info {
                    let entity =
                        CoreEntity::new(PolModelEntity::new(&mesh.vertices, material, &self.path));
                    scene.add_entity(entity)
                }
            }
        } else if self.path.to_lowercase().ends_with(".cvd") {
            let cvd = cvd_load_from_file(&self.path).unwrap();
            for (i, model) in cvd.models.iter().enumerate() {
                cvd_add_model_entity(&model, scene, &self.path, i as u32);
            }
        } else {
            panic!("Not supported file format");
        }
    }

    fn on_updating<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>, delta_sec: f32) {
        scene.camera_mut().transform_mut().rotate(
            &Vec3::new(0., 1., 0.),
            0.2 * delta_sec * std::f32::consts::PI,
        );
    }
}

fn cvd_add_model_entity<T: SceneCallbacks>(
    model_node: &CvdModelNode,
    scene: &mut CoreScene<T>,
    path: &str,
    id: u32,
) {
    if let Some(model) = &model_node.model{
        for material in &model.mesh.materials {
            if material.triangles.is_none() {
                continue;
            }

            println!("frame {}", model.mesh.frames.len());
            for v in &model.mesh.frames {
                let mut entity = CoreEntity::new(CvdModelEntity::new(
                    v,
                    material,
                    path,
                    id,
                ));
                entity
                    .transform_mut()
                    .translate_local(&model.position_keyframes[0].position);
                scene.add_entity(entity);

                break;
            }
        }
    }

    if let Some(children) = &model_node.children {
        for child in children {
            cvd_add_model_entity(child, scene, path, id);
        }
    }
}
