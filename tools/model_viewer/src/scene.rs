use opengb::scene::CvdModelEntity;
use opengb::scene::Mv3ModelEntity;
use opengb::scene::PolModelEntity;
use opengb::loaders::cvdloader::*;
use opengb::loaders::polloader::*;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene, SceneExtension};

pub struct ModelViewerScene {
    pub path: String,
}

impl SceneExtension<ModelViewerScene> for ModelViewerScene {
    fn on_loading(&mut self, scene: &mut CoreScene<ModelViewerScene>) {
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

    fn on_updating(&mut self, scene: &mut CoreScene<ModelViewerScene>, delta_sec: f32) {
        scene.camera_mut().transform_mut().rotate_axis_angle(
            &Vec3::new(0., 1., 0.),
            0.2 * delta_sec * std::f32::consts::PI,
        );
    }
}

fn cvd_add_model_entity(
    model_node: &CvdModelNode,
    scene: &mut CoreScene<ModelViewerScene>,
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
                    .translate_local(&model.position_keyframes.as_ref().unwrap().frames[0].position);
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
