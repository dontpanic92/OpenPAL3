use crate::loaders::cvdloader::*;
use crate::loaders::polloader::*;
use crate::loaders::scnloader::*;
use crate::scene::CvdModelEntity;
use crate::scene::Mv3ModelEntity;
use crate::scene::PolModelEntity;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene, SceneExtension};
use std::path::{Path, PathBuf};

pub struct ScnScene {
    path: String,
    scn_file: ScnFile,
}

impl SceneExtension<ScnScene> for ScnScene {
    fn on_loading(&mut self, scene: &mut CoreScene<ScnScene>) {
        scene
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(308.31, 229.44, 468.61))
            .rotate_axis_angle_local(&Vec3::UP, 33.24_f32.to_radians())
            .rotate_axis_angle_local(&Vec3::new(1., 0., 0.), -19.48_f32.to_radians());

        load_scene(scene, &self.path, &self.scn_file, false);
    }

    fn on_updating(&mut self, scene: &mut CoreScene<ScnScene>, delta_sec: f32) {}
}

impl ScnScene {
    pub fn new(path: &Path) -> Self {
        let scn_file = scn_load_from_file(path);
        println!("{:?}", scn_file);
        Self {
            path: path.to_str().unwrap().to_owned(),
            scn_file,
        }
    }
}

pub fn load_scene<T: SceneExtension<T>>(
    scene: &mut CoreScene<T>,
    path: &str,
    scn_file: &ScnFile,
    load_objects: bool,
) {
    let scn_path = PathBuf::from(&path);
    let scn_private_folder = scn_path.parent().unwrap().join(&scn_file.scn_base_name);
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
    let pol_name = scn_file.scn_base_name.clone() + ".pol";
    let pol_path = scn_private_folder.join(pol_name);
    println!("{:?}", pol_path);
    load_model(
        pol_path.to_str().unwrap(),
        "_ground",
        scene,
        &Vec3::new(0., 0., 0.),
        0.,
    );

    if !load_objects {
        return;
    }

    let mut i = 0;
    for obj in &scn_file.nodes {
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

            load_model(
                obj_path.to_str().unwrap(),
                &format!("object_{}", i),
                scene,
                &obj.position,
                obj.rotation.to_radians(),
            );

            i += 1;
        }
    }
}

fn load_model<T: SceneExtension<T>>(
    model_path: &str,
    name: &str,
    scene: &mut CoreScene<T>,
    position: &Vec3,
    rotation: f32,
) {
    println!("{}", model_path);
    if model_path.to_lowercase().ends_with(".mv3") {
        let mut entity = CoreEntity::new(Mv3ModelEntity::new_from_file(&model_path), name);
        entity
            .transform_mut()
            .set_position(position)
            .rotate_axis_angle_local(&Vec3::UP, rotation);
        scene.add_entity(entity);
    } else if model_path.to_lowercase().ends_with(".pol") {
        let pol = pol_load_from_file(&model_path).unwrap();
        let mut i = 0;
        for mesh in &pol.meshes {
            for material in &mesh.material_info {
                let mut entity = CoreEntity::new(
                    PolModelEntity::new(&mesh.vertices, material, &model_path),
                    &format!("{}_{}", name, i),
                );
                entity
                    .transform_mut()
                    .set_position(position)
                    .rotate_axis_angle_local(&Vec3::UP, rotation);
                scene.add_entity(entity);

                i += 1;
            }
        }
    } else if model_path.to_lowercase().ends_with(".cvd") {
        let cvd = cvd_load_from_file(&model_path).unwrap();
        for (i, model) in cvd.models.iter().enumerate() {
            cvd_add_model_entity(
                &model,
                name,
                scene,
                &model_path,
                i as u32,
                position,
                rotation,
            );
        }
    } else {
        panic!("Not supported file format");
    }
}

fn cvd_add_model_entity<T: SceneExtension<T>>(
    model_node: &CvdModelNode,
    name: &str,
    scene: &mut CoreScene<T>,
    path: &str,
    id: u32,
    position: &Vec3,
    rotation: f32,
) {
    if let Some(model) = &model_node.model {
        for material in &model.mesh.materials {
            if material.triangles.is_none() {
                continue;
            }

            for v in &model.mesh.frames {
                let mut entity = CoreEntity::new(
                    CvdModelEntity::new(v, material, path, id),
                    &format!("{}_{}", name, id),
                );
                let mut transform = entity
                    .transform_mut()
                    .set_position(position)
                    .rotate_axis_angle_local(&Vec3::UP, rotation);

                if let Some(p) = model
                    .position_keyframes
                    .as_ref()
                    .and_then(|frame| frame.frames.get(0))
                    .and_then(|f| Some(&f.position))
                {
                    transform.translate_local(p);
                }

                transform.scale_local(&Vec3::new(
                    model.scale_factor,
                    model.scale_factor,
                    model.scale_factor,
                ));

                if let Some(q) = model
                    .rotation_keyframes
                    .as_ref()
                    .and_then(|frame| frame.frames.get(0))
                    .and_then(|f| Some(&f.quaternion))
                {
                    let mut q2 = *q;
                    q2.w = -q2.w;
                    transform.rotate_quaternion_local(&q2);
                }

                if let Some(s) = model
                    .scale_keyframes
                    .as_ref()
                    .and_then(|frame| frame.frames.get(0))
                {
                    let mut q2 = s.quaternion;
                    q2.w = -q2.w;

                    let mut q3 = q2;
                    q3.inverse();

                    transform
                        .rotate_quaternion_local(&q2)
                        .scale_local(&s.scale)
                        .rotate_quaternion_local(&q3);
                }

                scene.add_entity(entity);
                break;
            }
        }
    }

    if let Some(children) = &model_node.children {
        for child in children {
            cvd_add_model_entity(child, name, scene, path, id, &Vec3::new_zeros(), rotation);
        }
    }
}
