use opengb::loaders::cvdloader::*;
use opengb::loaders::polloader::*;
use opengb::loaders::scnloader::*;
use opengb::scene::CvdModelEntity;
use opengb::scene::Mv3ModelEntity;
use opengb::scene::PolModelEntity;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene, SceneCallbacks};
use std::path::PathBuf;

pub struct ScnScene {
    path: String,
    scn_file: ScnFile,
}

impl SceneCallbacks for ScnScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        scene
            .camera_mut()
            .transform_mut()
            .translate_local(&Vec3::new(0., 400., 1000.));

        let scn_path = PathBuf::from(&self.path);
        let scn_private_folder = scn_path.parent().unwrap().join(&self.scn_file.scn_name);
        let object_path = scn_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("basedata")
            .join("object");
        let item_path = scn_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("basedata")
            .join("item");
        let pol_name = self.scn_file.scn_name.clone() + ".pol";
        let pol_path = scn_private_folder.join(pol_name);
        println!("{:?}", pol_path);
        ScnScene::load_model(pol_path.to_str().unwrap(), scene, &Vec3::new(0., 0., 0.));

        for obj in &self.scn_file.nodes {
            if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
                println!("nodetype {} name {}", obj.node_type, &obj.name);
                let obj_path;
                if obj.name.as_bytes()[0] as char == '_' {
                    obj_path = scn_private_folder.join(&obj.name);
                } else if obj.name.contains('.') {
                    obj_path = object_path.join(&obj.name);
                } else if obj.name.as_bytes()[0] as char == '+' {
                    // Unknown
                    continue;
                } else {
                    obj_path = item_path.join(&obj.name).join(obj.name.to_owned() + ".pol");
                }

                ScnScene::load_model(obj_path.to_str().unwrap(), scene, &obj.position);
            }
        }
    }

    fn on_updating<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>, delta_sec: f32) {
        scene.camera_mut().transform_mut().rotate(
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

    fn load_model<T: SceneCallbacks>(model_path: &str, scene: &mut CoreScene<T>, position: &Vec3) {
        println!("{}", model_path);
        if model_path.to_lowercase().ends_with(".mv3") {
            let mut entity = CoreEntity::new(Mv3ModelEntity::new(&model_path));
            entity.transform_mut().set_position(position);
            scene.add_entity(entity);
        } else if model_path.to_lowercase().ends_with(".pol") {
            let pol = pol_load_from_file(&model_path).unwrap();
            for mesh in &pol.meshes {
                for material in &mesh.material_info {
                    let mut entity =
                        CoreEntity::new(PolModelEntity::new(&mesh.vertices, material, &model_path));
                    entity.transform_mut().set_position(position);
                    scene.add_entity(entity)
                }
            }
        } else if model_path.to_lowercase().ends_with(".cvd") {
            let cvd = cvd_load_from_file(&model_path).unwrap();
            for (i, model) in cvd.models.iter().enumerate() {
                cvd_add_model_entity(&model, scene, &model_path, i as u32, position);
            }
        } else {
            panic!("Not supported file format");
        }
    }
}

fn cvd_add_model_entity<T: SceneCallbacks>(
    model_node: &CvdModelNode,
    scene: &mut CoreScene<T>,
    path: &str,
    id: u32,
    position: &Vec3,
) {
    if let Some(model) = &model_node.model {
        for material in &model.mesh.materials {
            if material.triangles.is_none() {
                continue;
            }

            println!("frame {}", model.mesh.frames.len());
            for v in &model.mesh.frames {
                let mut entity = CoreEntity::new(CvdModelEntity::new(v, material, path, id));
                entity
                    .transform_mut()
                    .set_position(position)
                    .translate_local(&model.position_keyframes[0].position);
                scene.add_entity(entity);

                break;
            }
        }
    }

    if let Some(children) = &model_node.children {
        for child in children {
            cvd_add_model_entity(child, scene, path, id, &Vec3::new_zeros());
        }
    }
}
