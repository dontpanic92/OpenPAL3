use crate::asset_manager::AssetManager;
use crate::loaders::{nav_loader::NavFile, scn_loader::*};
use radiance::scene::{CoreEntity, CoreScene, Entity, SceneExtension};
use radiance::{math::Vec3, scene::Scene};
use std::rc::Rc;

use super::RoleEntity;

pub struct ScnScene {
    asset_mgr: Rc<AssetManager>,
    cpk_name: String,
    scn_name: String,
    scn_file: ScnFile,
    nav: Nav,
    nav_triggers: Vec<SceNavTrigger>,
}

impl SceneExtension for ScnScene {
    fn on_loading(self: &mut CoreScene<ScnScene>) {
        self.load_objects();
        self.load_roles();
    }

    fn on_updating(self: &mut CoreScene<ScnScene>, delta_sec: f32) {}
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
            nav: Nav::new(nav_file),
            nav_triggers: vec![],
        }
    }

    pub fn nav_min_coord(&self) -> Vec3 {
        self.nav.nav_file.maps[0].min_coord
    }

    pub fn nav_block_size(&self) -> (f32, f32) {
        (self.nav.block_size_x, self.nav.block_size_z)
    }

    pub fn get_distance_to_border_by_scene_coord(&self, coord: &Vec3) -> f32 {
        let nav_coord = self.scene_coord_to_nav_coord(coord);
        let nav_coord_floor = (
            (nav_coord.0.floor() as usize).clamp(0, self.nav.nav_file.maps[0].width as usize - 1),
            (nav_coord.1.floor() as usize).clamp(0, self.nav.nav_file.maps[0].height as usize - 1),
        );

        let nav_coord_ceil = (
            (nav_coord.0.ceil() as usize).clamp(0, self.nav.nav_file.maps[0].width as usize - 1),
            (nav_coord.1.ceil() as usize).clamp(0, self.nav.nav_file.maps[0].height as usize - 1),
        );
        let distance_floor = &self.nav.nav_file.maps[0].map[nav_coord_floor.1][nav_coord_floor.0];
        let distance_ceil = &self.nav.nav_file.maps[0].map[nav_coord_ceil.1][nav_coord_ceil.0];
        std::cmp::min(
            distance_floor.distance_to_border,
            distance_ceil.distance_to_border,
        ) as f32
    }

    pub fn try_trigger_sce_proc(&self, coord: &Vec3) -> Option<u32> {
        let nav_coord = self.scene_coord_to_nav_coord(coord);
        for trigger in &self.nav_triggers {
            if nav_coord.0 >= trigger.nav_coord_min.0 as f32
                && nav_coord.1 >= trigger.nav_coord_min.1 as f32
                && nav_coord.0 <= trigger.nav_coord_max.0 as f32
                && nav_coord.1 <= trigger.nav_coord_max.1 as f32
            {
                return Some(trigger.sce_proc_id);
            }
        }

        None
    }

    pub fn scene_coord_to_nav_coord(&self, coord: &Vec3) -> (f32, f32) {
        let min_coord = self.nav_min_coord();
        (
            (coord.x - min_coord.x) / self.nav.block_size_x,
            (coord.z - min_coord.z) / self.nav.block_size_z,
        )
    }

    pub fn nav_coord_to_scene_coord(&self, nav_x: f32, nav_z: f32) -> Vec3 {
        let min_coord = self.nav_min_coord();
        let block_size = self.nav_block_size();
        Vec3::new(
            nav_x * block_size.0 + min_coord.x,
            min_coord.y,
            nav_z * block_size.1 + min_coord.z,
        )
    }

    pub fn get_role_entity<'a>(
        self: &'a mut CoreScene<Self>,
        name: &str,
    ) -> &'a CoreEntity<RoleEntity> {
        let pos = self
            .entities()
            .iter()
            .position(|e| e.name() == name)
            .unwrap();
        self.entities()
            .get(pos)
            .unwrap()
            .as_ref()
            .downcast_ref::<CoreEntity<RoleEntity>>()
            .unwrap()
    }

    pub fn get_role_entity_mut<'a>(
        self: &'a mut CoreScene<Self>,
        name: &str,
    ) -> &'a mut CoreEntity<RoleEntity> {
        let pos = self
            .entities_mut()
            .iter()
            .position(|e| e.name() == name)
            .unwrap();
        self.entities_mut()
            .get_mut(pos)
            .unwrap()
            .as_mut()
            .downcast_mut::<CoreEntity<RoleEntity>>()
            .unwrap()
    }

    fn load_objects(self: &mut CoreScene<ScnScene>) {
        let ground_pol_name = self.scn_file.scn_base_name.clone() + ".pol";
        let mut cvd_objects = vec![];
        let mut pol_objects = self.asset_mgr.load_scn_pol(
            &self.cpk_name,
            &self.scn_file.scn_base_name,
            &ground_pol_name,
        );

        let _self = self.extension_mut();
        for obj in &_self.scn_file.nodes {
            let mut pol = vec![];
            let mut cvd = vec![];
            if obj.node_type == 0 {
                _self.nav_triggers.push(SceNavTrigger {
                    nav_coord_max: obj.nav_coord_max,
                    nav_coord_min: obj.nav_coord_min,
                    sce_proc_id: obj.sce_proc_id,
                });
            } else if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
                if obj.name.as_bytes()[0] as char == '_' {
                    pol.append(&mut _self.asset_mgr.load_scn_pol(
                        &_self.cpk_name,
                        &_self.scn_name,
                        &obj.name,
                    ));
                } else if obj.name.ends_with(".pol") {
                    pol.append(&mut _self.asset_mgr.load_object_item_pol(&obj.name));
                } else if obj.name.ends_with(".cvd") {
                    cvd.append(&mut _self.asset_mgr.load_object_item_cvd(
                        &obj.name,
                        &obj.position,
                        obj.rotation.to_radians(),
                    ));
                } else if obj.name.as_bytes()[0] as char == '+' {
                    // Unknown
                    continue;
                } else {
                    pol.append(&mut _self.asset_mgr.load_object_item_pol(&obj.name));
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
            self.add_entity(entity);
        }

        for entity in cvd_objects {
            self.add_entity(entity);
        }
    }

    fn apply_position_rotation(entity: &mut dyn Entity, position: &Vec3, rotation: f32) {
        entity
            .transform_mut()
            .set_position(position)
            .rotate_axis_angle_local(&Vec3::UP, rotation);
    }

    fn load_roles(self: &mut CoreScene<ScnScene>) {
        for i in 101..111 {
            let role_name = i.to_string();
            let entity_name = i.to_string();
            let role_entity = self.asset_mgr.load_role(&role_name, "C01");
            let entity = CoreEntity::new(role_entity, &entity_name);
            self.add_entity(entity);
        }

        let mut entities = vec![];
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
            entities.push(entity);
        }

        for e in entities {
            self.add_entity(e);
        }
    }
}

pub struct Nav {
    nav_file: NavFile,
    block_size_x: f32,
    block_size_z: f32,
}

impl Nav {
    pub fn new(nav_file: NavFile) -> Self {
        let area = Vec3::sub(&nav_file.maps[0].max_coord, &nav_file.maps[0].min_coord);
        let width = nav_file.maps[0].width;
        let height = nav_file.maps[0].height;
        Self {
            nav_file,
            block_size_x: area.x / width as f32,
            block_size_z: area.z / height as f32,
        }
    }
}

pub struct SceNavTrigger {
    nav_coord_min: (u32, u32),
    nav_coord_max: (u32, u32),
    sce_proc_id: u32,
}
