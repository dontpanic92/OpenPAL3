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
    aabb_triggers: Vec<SceAabbTrigger>,
    item_triggers: Vec<SceItemTrigger>,
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
            aabb_triggers: vec![],
            item_triggers: vec![],
        }
    }

    pub fn name(&self) -> &str {
        &self.cpk_name
    }

    pub fn sub_name(&self) -> &str {
        &self.scn_name
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

    pub fn test_nav_trigger(&self, coord: &Vec3) -> Option<u32> {
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

    pub fn test_aabb_trigger(&self, coord: &Vec3) -> Option<u32> {
        const R: f32 = 50.;
        for trigger in &self.aabb_triggers {
            log::debug!(
                "Testing Aabb {:?} {:?} {:?}",
                &trigger.aabb_coord2,
                &trigger.aabb_coord1,
                &coord
            );
            if Self::test_sphere_aabb(coord, R, &trigger.aabb_coord1, &trigger.aabb_coord2) {
                return Some(trigger.sce_proc_id);
            }
        }

        None
    }

    pub fn test_item_trigger(&self, coord: &Vec3) -> Option<u32> {
        const D: f32 = 100.;
        for trigger in &self.item_triggers {
            log::debug!(
                "Testing item trigger {:?} {:?} {}",
                &trigger.coord,
                &coord,
                Vec3::sub(coord, &trigger.coord).norm2(),
            );
            if Vec3::sub(coord, &trigger.coord).norm2() < D * D {
                return Some(trigger.sce_proc_id);
            }
        }

        None
    }

    pub fn test_role_trigger(&self, coord: &Vec3) -> Option<u32> {
        const D: f32 = 100.;
        for role in &self.scn_file.roles {
            let role_position = Vec3::new(role.position_x, role.position_y, role.position_z);
            log::debug!(
                "Testing role trigger {:?} {:?} {}",
                &role_position,
                &coord,
                Vec3::sub(coord, &role_position).norm2(),
            );
            if Vec3::sub(coord, &role_position).norm2() < D * D {
                return Some(role.sce_proc_id);
            }
        }

        None
    }

    pub fn get_height(&self, nav_coord: (f32, f32)) -> f32 {
        let x =
            (nav_coord.0.ceil() as usize).clamp(0, self.nav.nav_file.maps[0].width as usize - 1);
        let y =
            (nav_coord.1.ceil() as usize).clamp(0, self.nav.nav_file.maps[0].height as usize - 1);
        self.nav.nav_file.maps[0].map[y][x].height
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

    pub fn get_object<'a>(self: &'a mut CoreScene<Self>, id: i32) -> Option<&'a dyn Entity> {
        self.entities()
            .iter()
            .find(|e| e.name() == format!("OBJECT_{}", id))
            .map(|e| *e)
    }

    pub fn get_root_object_mut<'a>(
        self: &'a mut CoreScene<Self>,
        id: i32,
    ) -> Option<&'a mut dyn Entity> {
        self.root_entities_mut()
            .iter_mut()
            .find(|e| e.name() == format!("OBJECT_{}", id))
            .map(|e| &mut **e)
    }

    pub fn get_role_entity<'a>(
        self: &'a mut CoreScene<Self>,
        id: i32,
    ) -> &'a CoreEntity<RoleEntity> {
        let pos = self
            .entities()
            .iter()
            .position(|e| e.name() == format!("ROLE_{}", id))
            .unwrap();
        self.entities()
            .get(pos)
            .unwrap()
            .downcast_ref::<CoreEntity<RoleEntity>>()
            .unwrap()
    }

    pub fn get_role_entity_mut<'a>(
        self: &'a mut CoreScene<Self>,
        id: i32,
    ) -> &'a mut CoreEntity<RoleEntity> {
        let pos = self
            .root_entities_mut()
            .iter()
            .position(|e| e.name() == format!("ROLE_{}", id))
            .unwrap();
        self.root_entities_mut()
            .get_mut(pos)
            .unwrap()
            .as_mut()
            .downcast_mut::<CoreEntity<RoleEntity>>()
            .unwrap()
    }

    fn test_sphere_aabb(s: &Vec3, r: f32, aabb1: &Vec3, aabb2: &Vec3) -> bool {
        macro_rules! dist_sqr {
            ($s: expr, $min: expr, $max: expr) => {
                if $s < $min {
                    ($min - $s) * ($min - $s)
                } else if $s > $max {
                    ($s - $max) * ($s - $max)
                } else {
                    0.
                }
            };
        }

        let x = vec![aabb1.x.min(aabb2.x), aabb1.x.max(aabb2.x)];
        let y = vec![aabb1.y.min(aabb2.y), aabb1.y.max(aabb2.y)];
        let z = vec![aabb1.z.min(aabb2.z), aabb1.z.max(aabb2.z)];

        let dist =
            dist_sqr!(s.x, x[0], x[1]) + dist_sqr!(s.y, y[0], y[1]) + dist_sqr!(s.z, z[0], z[1]);

        log::debug!("Testing Aabb {} {}", dist, r * r);
        dist < r * r
    }

    fn load_objects(self: &mut CoreScene<ScnScene>) {
        let ground_pol_name = self.scn_file.scn_base_name.clone() + ".pol";
        let mut entities: Vec<Box<dyn Entity>> = vec![];

        let mut scn_object = self
            .asset_mgr
            .load_scn_pol(
                &self.cpk_name,
                &self.scn_file.scn_base_name,
                &ground_pol_name,
                std::u16::MAX,
            )
            .unwrap();
        Self::apply_position_rotation(&mut scn_object, &Vec3::new(0., 0., 0.), 0.);
        entities.push(Box::new(scn_object));

        let _self = self.extension_mut();
        for obj in &_self.scn_file.nodes {
            let mut entity: Option<Box<dyn Entity>> = None;
            if obj.nav_trigger_coord_min.0 != 0
                || obj.nav_trigger_coord_min.1 != 0
                || obj.nav_trigger_coord_max.0 != 0
                || obj.nav_trigger_coord_max.1 != 0
            {
                _self.nav_triggers.push(SceNavTrigger {
                    nav_coord_max: obj.nav_trigger_coord_max,
                    nav_coord_min: obj.nav_trigger_coord_min,
                    sce_proc_id: obj.sce_proc_id,
                });
            }

            if obj.node_type == 16 {
                _self.item_triggers.push(SceItemTrigger {
                    coord: obj.position,
                    sce_proc_id: obj.sce_proc_id,
                });
            } else if obj.node_type == 20 {
                _self.aabb_triggers.push(SceAabbTrigger {
                    aabb_coord2: obj.aabb_trigger_coord2,
                    aabb_coord1: obj.aabb_trigger_coord1,
                    sce_proc_id: obj.sce_proc_id,
                });
            }

            if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
                if obj.name.as_bytes()[0] as char == '_' {
                    if let Some(p) = _self.asset_mgr.load_scn_pol(
                        &_self.cpk_name,
                        &_self.scn_name,
                        &obj.name,
                        obj.index,
                    ) {
                        entity = Some(Box::new(p));
                    } else if let Some(c) = _self.asset_mgr.load_scn_cvd(
                        &_self.cpk_name,
                        &_self.scn_name,
                        &obj.name,
                        obj.index,
                    ) {
                        entity = Some(Box::new(c));
                    } else {
                        log::error!("Cannot load object: {}", obj.name);
                    }
                } else if obj.name.to_lowercase().ends_with(".pol") {
                    entity = Some(Box::new(
                        _self
                            .asset_mgr
                            .load_object_item_pol(&obj.name, obj.index)
                            .unwrap(),
                    ));
                } else if obj.name.to_lowercase().ends_with(".cvd") {
                    entity = Some(Box::new(
                        _self
                            .asset_mgr
                            .load_object_item_cvd(&obj.name, obj.index)
                            .unwrap(),
                    ));
                } else if obj.name.as_bytes()[0] as char == '+' {
                    // Unknown
                    continue;
                } else {
                    entity = Some(Box::new(
                        _self
                            .asset_mgr
                            .load_object_item_pol(&obj.name, obj.index)
                            .unwrap(),
                    ));
                }
            }

            if let Some(mut p) = entity {
                Self::apply_position_rotation(p.as_mut(), &obj.position, obj.rotation.to_radians());
                entities.push(p);
            }
        }

        for entity in entities {
            self.add_entity(entity);
        }
    }

    fn apply_position_rotation(entity: &mut dyn Entity, position: &Vec3, rotation: f32) {
        entity
            .transform_mut()
            .set_position(position)
            .rotate_axis_angle_local(&Vec3::UP, rotation);
    }

    fn map_role_id(role_id: i32) -> i32 {
        match role_id {
            -1 => 101,
            0 => 101,
            1 => 104,
            5 => 109,
            x => x,
        }
    }

    fn load_roles(self: &mut CoreScene<ScnScene>) {
        for i in &[-1, 0, 1, 5] {
            let entity_name = format!("ROLE_{}", i);
            let model_name = Self::map_role_id(*i).to_string();
            let role_entity = self.asset_mgr.load_role(&model_name, "C01").unwrap();
            let entity = CoreEntity::new(role_entity, entity_name);
            self.add_entity(Box::new(entity));
        }

        let mut entities = vec![];
        for role in &self.scn_file.roles {
            if let Some(role_entity) = self.asset_mgr.load_role(&role.name, &role.action_name) {
                let mut entity = CoreEntity::new(role_entity, format!("ROLE_{}", role.index));
                entity
                    .transform_mut()
                    .set_position(&Vec3::new(
                        role.position_x,
                        role.position_y,
                        role.position_z,
                    ))
                    // HACK
                    .rotate_axis_angle_local(&Vec3::UP, std::f32::consts::PI);

                if role.sce_proc_id != 0 {
                    entity.set_active(true);
                }

                entities.push(entity);
            }
        }

        for e in entities {
            self.add_entity(Box::new(e));
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

pub struct SceAabbTrigger {
    aabb_coord1: Vec3,
    aabb_coord2: Vec3,
    sce_proc_id: u32,
}

pub struct SceItemTrigger {
    coord: Vec3,
    sce_proc_id: u32,
}
