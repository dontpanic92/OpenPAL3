use crate::asset_manager::AssetManager;
use crate::loaders::{cvd_loader::*, nav_loader::NavFile, pol_loader::*, scn_loader::*};
use crate::scene::CvdModelEntity;
use crate::scene::PolModelEntity;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, SceneExtension};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

pub struct ScnScene {
    asset_mgr: Rc<AssetManager>,
    cpk_name: String,
    scn_name: String,
    scn_file: ScnFile,
    nav_file: NavFile,
}

impl SceneExtension<ScnScene> for ScnScene {
    fn on_loading(&mut self, scene: &mut CoreScene<ScnScene>) {
        self.load_objects(scene);
        self.load_roles(scene);
    }

    fn on_updating(&mut self, scene: &mut CoreScene<ScnScene>, delta_sec: f32) {}
}

impl ScnScene {
    pub fn new(
        asset_mgr: &Rc<AssetManager>,
        cpk_name: &str,
        scn_name: &str,
        scn_file: ScnFile,
        nav_file: NavFile,
    ) -> Self {
        Self {
            asset_mgr: asset_mgr.clone(),
            cpk_name: cpk_name.to_string(),
            scn_name: scn_name.to_string(),
            scn_file,
            nav_file,
        }
    }

    pub fn nav_origin(&self) -> &Vec3 {
        &self.nav_file.unknown1[0].origin
    }

    fn load_objects(&self, scene: &mut CoreScene<ScnScene>) {
        let ground_pol_name = self.scn_file.scn_base_name.clone() + ".pol";
        let mut cvd_objects = vec![];
        let mut pol_objects = self.asset_mgr.load_scn_pol(&self.cpk_name, &self.scn_name, &ground_pol_name);

        for obj in &scn_file.nodes {
            let mut pol = vec![];
            let mut cvd = vec![];
            if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
                if obj.name.as_bytes()[0] as char == '_' {
                    pol.append(self.asset_mgr.load_scn_pol(&self.cpk_name, &self.scn_name, &obj.name));
                } else if obj.name.ends_with(".pol") {
                    pol.append(self.asset_mgr.load_object_item_pol(&obj.name));
                } else if obj.name.ends_with(".cvd") {
                    cvd.append(self.asset_mgr.load_object_item_cvd(&obj.name));
                } else if obj.name.as_bytes()[0] as char == '+' {
                    // Unknown
                    continue;
                } else {
                    pol.append(self.asset_mgr.load_object_item_pol(&obj.name));
                }
            }

            pol.iter_mut().for_each(|e| Self::apply_position_rotation(e, &obj.position, obj.rotation.to_radian()));
            pol_objects.append(pol);
            cvd_objects.append(cvd);
        }

        pol_objects.sort_by_key(|e| e.has_alpha());
        for entity in pol_objects {
            scene.add_entity(entity);
        }

        for entity in cvd_objects {
            scene.add_entity(entity);
        }
    }

    fn apply_position_rotation(entity: &mut dyn Entity, position: &Vec3, rotation: f32) {
        entity.transform_mut()
            .set_position(position)
            .rotate_axis_angle_local(&Vec3::UP, rotation);
    }

    fn load_roles(&self, scene: &mut CoreScene<ScnScene>) {
        for i in 101..111 {
            let role_name = i.to_string();
            let entity_name = i.to_string();
            let role_entity = self.asset_mgr.load_role(&role_name, "C01");
            let entity = CoreEntity::new(role_entity, &entity_name);
            scene.add_entity(entity);
        }

        for role in &self.scn_file.roles {
            let role_entity = self.asset_mgr.load_role(&role.name, &role.action_name);
            let mut entity = CoreEntity::new(role_entity, &role.index.to_string());
            entity
                .transform_mut()
                .set_position(&Vec3::new(
                    role.position_x,
                    role.position_y,
                    role.position_z,
                ))
                // HACK
                .rotate_axis_angle_local(&Vec3::UP, std::f32::consts::PI);
            scene.add_entity(entity);
        }
    }
}

pub fn load_scene<T: SceneExtension<T>>(
    asset_mgr: &Rc<AssetManager>,
    scene: &mut CoreScene<T>,
    path: &str,
    scn_file: &ScnFile,
    load_objects: bool,
) {
    let ground = asset_mgr.load_scn_pol(cpk_name, scn_name, pol_name);

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
    if model_path.to_lowercase().ends_with(".mv3") {
        panic!("????");
    } else if model_path.to_lowercase().ends_with(".pol") {
        let pol = pol_load_from_file(&model_path).unwrap();
        let mut i = 0;
        let mut entities = vec![];
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

                entities.push(entity);
                i += 1;
            }
        }

        entities.sort_by_key(|e| e.has_alpha());
        for entity in entities {
            scene.add_entity(entity);
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
                let transform = entity
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
                    transform.rotate_quaternion_local(q);
                }

                if let Some(s) = model
                    .scale_keyframes
                    .as_ref()
                    .and_then(|frame| frame.frames.get(0))
                {
                    let q2 = s.quaternion;
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
