use crate::asset_manager::AssetManager;
use crate::loaders::{nav_loader::NavFile, scn_loader::*};
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, CoreScene, Entity, SceneExtension};
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
        let mut pol_objects =
            self.asset_mgr
                .load_scn_pol(&self.cpk_name, &self.scn_file.scn_base_name, &ground_pol_name);

        for obj in &self.scn_file.nodes {
            let mut pol = vec![];
            let mut cvd = vec![];
            if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
                if obj.name.as_bytes()[0] as char == '_' {
                    pol.append(&mut self.asset_mgr.load_scn_pol(
                        &self.cpk_name,
                        &self.scn_name,
                        &obj.name,
                    ));
                } else if obj.name.ends_with(".pol") {
                    pol.append(&mut self.asset_mgr.load_object_item_pol(&obj.name));
                } else if obj.name.ends_with(".cvd") {
                    cvd.append(&mut self.asset_mgr.load_object_item_cvd(
                        &obj.name,
                        &obj.position,
                        obj.rotation.to_radians(),
                    ));
                } else if obj.name.as_bytes()[0] as char == '+' {
                    // Unknown
                    continue;
                } else {
                    pol.append(&mut self.asset_mgr.load_object_item_pol(&obj.name));
                }
            }

            pol.iter_mut().for_each(|e| {
                Self::apply_position_rotation(e, &obj.position, obj.rotation.to_radians())
            });
            pol_objects.append(&mut pol);
            cvd_objects.append(&mut cvd);
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
        entity
            .transform_mut()
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
