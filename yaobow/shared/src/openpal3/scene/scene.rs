use crate::openpal3::asset_manager::AssetManager;
use crate::openpal3::comdef::{IRoleController, IScnSceneComponentImpl};
use crate::openpal3::loaders::nav_loader::{NavFile, NavMapPoint};
use crate::openpal3::loaders::scn_loader::ScnFile;
use crate::openpal3::scene::RoleController;
use crate::ComObject_ScnSceneComponent;
use crosscom::ComRc;
use radiance::comdef::{IComponentImpl, IEntity, IScene};
use radiance::math::Vec3;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub struct ScnScene {
    scene: ComRc<IScene>,
    asset_mgr: Rc<AssetManager>,
    cpk_name: String,
    scn_name: String,
    scn_file: ScnFile,
    nav: Nav,
    nav_triggers: RefCell<Vec<SceNavTrigger>>,
    aabb_triggers: RefCell<Vec<SceAabbTrigger>>,
    item_triggers: RefCell<Vec<SceItemTrigger>>,
    ladder_triggers: RefCell<Vec<LadderTrigger>>,
}

ComObject_ScnSceneComponent!(super::ScnScene);

impl IComponentImpl for ScnScene {
    fn on_loading(&self) {
        self.load_objects();
        self.load_roles();
    }

    fn on_updating(&self, _delta_sec: f32) {}
}

impl IScnSceneComponentImpl for ScnScene {
    fn get(&self) -> &'static ScnScene {
        unsafe { &*(self as *const _) }
    }
}

impl ScnScene {
    pub fn new(
        scene: ComRc<IScene>,
        asset_mgr: &Rc<AssetManager>,
        cpk_name: &str,
        scn_name: &str,
        scn_file: ScnFile,
        nav_file: NavFile,
    ) -> Self {
        Self {
            scene,
            asset_mgr: asset_mgr.clone(),
            cpk_name: cpk_name.to_string(),
            scn_name: scn_name.to_string(),
            scn_file,
            nav: Nav::new(nav_file),
            nav_triggers: RefCell::new(vec![]),
            aabb_triggers: RefCell::new(vec![]),
            item_triggers: RefCell::new(vec![]),
            ladder_triggers: RefCell::new(vec![]),
        }
    }

    pub fn name(&self) -> &str {
        &self.cpk_name
    }

    pub fn sub_name(&self) -> &str {
        &self.scn_name
    }

    pub fn nav(&self) -> &Nav {
        &self.nav
    }

    pub fn nav_min_coord(&self, layer: usize) -> Vec3 {
        self.nav.nav_file.maps[layer].min_coord
    }

    pub fn nav_block_size(&self, layer: usize) -> (f32, f32) {
        self.nav.block_sizes[layer]
    }

    pub fn get_distance_to_border_by_scene_coord(&self, layer: usize, coord: &Vec3) -> f32 {
        let nav_coord = self.scene_coord_to_nav_coord(layer, coord);
        if nav_coord.0.floor() as u32 >= self.nav.nav_file.maps[layer].width
            || nav_coord.1.floor() as u32 >= self.nav.nav_file.maps[layer].height
        {
            return 0.;
        }

        let nav_coord_floor = (
            (nav_coord.0.floor() as usize)
                .clamp(0, self.nav.nav_file.maps[layer].width as usize - 1),
            (nav_coord.1.floor() as usize)
                .clamp(0, self.nav.nav_file.maps[layer].height as usize - 1),
        );

        let nav_coord_ceil = (
            (nav_coord.0.ceil() as usize)
                .clamp(0, self.nav.nav_file.maps[layer].width as usize - 1),
            (nav_coord.1.ceil() as usize)
                .clamp(0, self.nav.nav_file.maps[layer].height as usize - 1),
        );
        let distance_floor =
            &self.nav.nav_file.maps[layer].map[nav_coord_floor.1][nav_coord_floor.0];
        let _distance_ceil = &self.nav.nav_file.maps[layer].map[nav_coord_ceil.1][nav_coord_ceil.0];
        /*std::cmp::min(
            distance_floor.distance_to_border,
            distance_ceil.distance_to_border,
        ) as f32*/
        distance_floor.distance_to_border as f32
    }

    pub fn test_nav_trigger(&self, layer: usize, coord: &Vec3) -> Option<u32> {
        let nav_coord = self.scene_coord_to_nav_coord(layer, coord);
        let nav_coord = (nav_coord.0 as i32, nav_coord.1 as i32);

        for trigger in &*self.nav_triggers.borrow() {
            if Self::test_coord_in_bound(nav_coord, (trigger.nav_coord_min, trigger.nav_coord_max))
            {
                if trigger.node_type == 14
                    || (trigger.node_type == 0 && trigger.layer as usize == layer)
                {
                    return Some(trigger.sce_proc_id);
                }
            }
        }

        None
    }

    fn test_coord_in_bound(coord: (i32, i32), boundary: ((i32, i32), (i32, i32))) -> bool {
        coord.0 >= boundary.0 .0
            && coord.1 >= boundary.0 .1
            && coord.0 <= boundary.1 .0
            && coord.1 <= boundary.1 .1
    }

    pub fn test_nav_layer_trigger(&self, layer: usize, coord: &Vec3) -> bool {
        if let Some(layer_triggers) = &self.nav.nav_file.maps[layer].layer_triggers {
            let nav_coord = self
                .nav
                .round_nav_coord(layer, self.scene_coord_to_nav_coord(layer, coord));
            for trigger in layer_triggers {
                if Self::test_coord_in_bound(
                    nav_coord,
                    (trigger.nav_coord_min, trigger.nav_coord_max),
                )
                /*|| Self::test_coord_in_bound(
                    nav_coord.1,
                    (trigger.nav_coord_min, trigger.nav_coord_max),
                ) */
                {
                    return true;
                }
            }
        }

        false
    }

    pub fn test_aabb_trigger(&self, coord: &Vec3) -> Option<u32> {
        const R: f32 = 50.;
        for trigger in &*self.aabb_triggers.borrow() {
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
        for trigger in &*self.item_triggers.borrow() {
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

    pub fn test_role_trigger(&self, coord: &Vec3, exclude_role: i32) -> Option<u32> {
        const D: f32 = 100.;
        for role in self.scene.entities() {
            if role.name() == format!("ROLE_{}", exclude_role) {
                continue;
            }

            if !role.visible() {
                continue;
            }

            let role_position = role.transform().borrow().position();
            if Vec3::sub(coord, &role_position).norm2() < D * D {
                let role_model = RoleController::get_role_controller(role);
                if role_model.is_none() {
                    continue;
                }

                return Some(role_model.unwrap().get().proc_id() as u32);
            }
        }

        None
    }

    pub fn test_ladder(&self, layer: usize, coord: &Vec3) -> Option<LadderTestResult> {
        println!("testing ladder layer {} coord {:?}", layer, coord);
        const D: f32 = 100.;
        let nav_coord = self
            .nav
            .round_nav_coord(layer, self.scene_coord_to_nav_coord(layer, coord));
        for ladder in &*self.ladder_triggers.borrow() {
            let new_layer = if ladder.switch_layer {
                (layer + 1) % 2
            } else {
                layer
            };

            let mut ladder_position = ladder.position;
            ladder_position.y = coord.y;
            if ladder.sce_proc_id != 0 {
                if Vec3::sub(coord, &ladder_position).norm2() < D * D {
                    return Some(LadderTestResult::SceProc(ladder.sce_proc_id as u32));
                }
            }

            if self
                .nav
                .check_connectivity(layer, nav_coord, ladder.nav_coord1)
            {
                return Some(LadderTestResult::NewPosition((
                    ladder.switch_layer,
                    self.nav_coord_to_scene_coord(
                        new_layer,
                        ladder.nav_coord2.0 as f32,
                        ladder.nav_coord2.1 as f32,
                    ),
                )));
            } else if self
                .nav
                .check_connectivity(layer, nav_coord, ladder.nav_coord2)
            {
                return Some(LadderTestResult::NewPosition((
                    ladder.switch_layer,
                    self.nav_coord_to_scene_coord(
                        new_layer,
                        ladder.nav_coord1.0 as f32,
                        ladder.nav_coord1.1 as f32,
                    ),
                )));
            }
        }

        None
    }

    pub fn get_height(&self, layer: usize, nav_coord: (f32, f32)) -> f32 {
        let x = (nav_coord.0.floor() as usize)
            .clamp(0, self.nav.nav_file.maps[layer].width as usize - 1);
        let y = (nav_coord.1.floor() as usize)
            .clamp(0, self.nav.nav_file.maps[layer].height as usize - 1);
        self.nav.nav_file.maps[layer].map[y][x].height
    }

    pub fn scene_coord_to_nav_coord(&self, layer: usize, coord: &Vec3) -> (f32, f32) {
        let min_coord = self.nav_min_coord(layer);
        let block_size = self.nav_block_size(layer);
        (
            (coord.x - min_coord.x) / block_size.0,
            (coord.z - min_coord.z) / block_size.1,
        )
    }

    pub fn nav_coord_to_scene_coord(&self, layer: usize, nav_x: f32, nav_z: f32) -> Vec3 {
        let min_coord = self.nav_min_coord(layer);
        let block_size = self.nav_block_size(layer);
        Vec3::new(
            nav_x * block_size.0 + min_coord.x,
            self.get_height(layer, (nav_x, nav_z)),
            nav_z * block_size.1 + min_coord.z,
        )
    }

    pub fn get_object(&self, id: i32) -> Option<ComRc<IEntity>> {
        self.scene
            .entities()
            .iter()
            .find(|e| e.name() == format!("OBJECT_{}", id))
            .map(|e| e.clone())
    }

    pub fn get_root_object(&self, id: i32) -> Option<ComRc<IEntity>> {
        self.scene
            .root_entities()
            .iter()
            .find(|e| e.name() == format!("OBJECT_{}", id))
            .cloned()
    }

    pub fn get_role_entity(&self, id: i32) -> Option<ComRc<IEntity>> {
        let pos = self
            .scene
            .entities()
            .iter()
            .position(|e| e.name() == format!("ROLE_{}", id));

        if let Some(pos) = pos {
            self.scene.entities().get(pos).cloned()
        } else {
            None
        }
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

        dist < r * r
    }

    fn load_objects(&self) {
        let ground_pol_name = self.scn_file.scn_base_name.clone() + ".pol";
        let mut entities: Vec<ComRc<IEntity>> = vec![];

        let scn_object = self.asset_mgr.load_scn_pol(
            &self.cpk_name,
            &self.scn_file.scn_base_name,
            &ground_pol_name,
            self.scn_file.is_night,
            std::u16::MAX,
        );

        if let Some(scn_object) = scn_object {
            Self::apply_position_rotation(scn_object.clone(), &Vec3::new(0., 0., 0.), 0.);
            entities.push(scn_object);
        }

        for obj in &self.scn_file.nodes {
            let mut entity: Option<ComRc<IEntity>> = None;
            if obj.nav_trigger_coord_min.0 != 0
                || obj.nav_trigger_coord_min.1 != 0
                || obj.nav_trigger_coord_max.0 != 0
                || obj.nav_trigger_coord_max.1 != 0
            {
                self.nav_triggers.borrow_mut().push(SceNavTrigger {
                    nav_coord_max: obj.nav_trigger_coord_max,
                    nav_coord_min: obj.nav_trigger_coord_min,
                    node_type: obj.node_type,
                    layer: obj.nav_layer,
                    sce_proc_id: obj.sce_proc_id,
                });
            }

            let visible = obj.node_type != 17 && obj.node_type != 25;
            if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
                if obj.name.as_bytes()[0] as char == '_' {
                    if let Some(p) = self.asset_mgr.load_scn_pol(
                        &self.cpk_name,
                        &self.scn_file.scn_base_name,
                        &obj.name,
                        self.scn_file.is_night,
                        obj.index,
                    ) {
                        entity = Some(p);
                    } else if let Some(c) = self.asset_mgr.load_scn_cvd(
                        &self.cpk_name,
                        &self.scn_file.scn_base_name,
                        &obj.name,
                        self.scn_file.is_night,
                        obj.index,
                    ) {
                        entity = Some(c);
                    } else {
                        log::error!("Cannot load object: {}", obj.name);
                    }
                } else if obj.name.to_lowercase().ends_with(".pol") {
                    entity = Some(
                        self.asset_mgr
                            .load_object_item_pol(&obj.name, obj.index, visible)
                            .unwrap(),
                    );
                } else if obj.name.to_lowercase().ends_with(".cvd") {
                    entity = Some(
                        self.asset_mgr
                            .load_object_item_cvd(&obj.name, obj.index, visible)
                            .unwrap(),
                    );
                } else if obj.name.as_bytes()[0] as char == '+' {
                    // Unknown
                    continue;
                } else {
                    entity = Some(
                        self.asset_mgr
                            .load_object_item_pol(&obj.name, obj.index, visible)
                            .unwrap(),
                    );
                }
            }

            match obj.node_type {
                ScnNodeTypes::LADDER | ScnNodeTypes::LADDER2 => {
                    self.ladder_triggers.borrow_mut().push(LadderTrigger {
                        position: obj.position,
                        nav_coord1: obj.ladder_nav_coord1,
                        nav_coord2: obj.ladder_nav_coord2,
                        switch_layer: obj.ladder_switch_layer != obj.nav_layer as i32
                            || obj.node_type == ScnNodeTypes::LADDER2, // ?? should be obj.ladder_target_layer?
                        sce_proc_id: obj.sce_proc_id,
                    })
                }
                ScnNodeTypes::ITEM_TRIGGER
                | ScnNodeTypes::ITEM_TRIGGER2
                | ScnNodeTypes::ITEM_TRIGGER3
                | ScnNodeTypes::TRIGGER_SOURCE => {
                    self.item_triggers.borrow_mut().push(SceItemTrigger {
                        coord: obj.position,
                        sce_proc_id: obj.sce_proc_id,
                    });
                }
                ScnNodeTypes::TRIGGER_TARGET => {}
                ScnNodeTypes::AABB_TRIGGER => {
                    self.aabb_triggers.borrow_mut().push(SceAabbTrigger {
                        aabb_coord2: obj.aabb_trigger_coord2,
                        aabb_coord1: obj.aabb_trigger_coord1,
                        sce_proc_id: obj.sce_proc_id,
                    });
                }
                _ => {}
            }

            if let Some(p) = entity {
                Self::apply_position_rotation(p.clone(), &obj.position, obj.rotation.to_radians());
                entities.push(p);
            }
        }

        for entity in entities {
            self.scene.add_entity(entity);
        }
    }

    fn apply_position_rotation(entity: ComRc<IEntity>, position: &Vec3, rotation: f32) {
        entity
            .transform()
            .borrow_mut()
            .set_position(position)
            .rotate_axis_angle_local(&Vec3::UP, rotation);
    }

    fn map_role_id(role_id: i32) -> i32 {
        match role_id {
            0 => 101,
            1 => 104,
            2 => 105,
            3 => 107,
            4 => 102,
            5 => 109,
            // 11 => 110,
            x => x,
        }
    }

    fn load_roles(&self) {
        for i in &[0, 1, 2, 3, 4, 5] {
            let entity_name = format!("ROLE_{}", i);
            let model_name = Self::map_role_id(*i).to_string();
            let entity = self
                .asset_mgr
                .load_role(&model_name, "C01", entity_name, false)
                .unwrap();
            self.scene.add_entity(entity);
        }

        let mut entities = vec![];
        for role in &self.scn_file.roles {
            if let Some(entity) = self.asset_mgr.load_role(
                &role.name,
                &role.action_name,
                format!("ROLE_{}", role.index),
                false,
            ) {
                let entity = entity;
                let nav_coord = self.scene_coord_to_nav_coord(
                    0,
                    &Vec3::new(role.position_x, role.position_y, role.position_z),
                );
                let height = self.get_height(0, nav_coord);
                entity
                    .transform()
                    .borrow_mut()
                    .set_position(&Vec3::new(role.position_x, height, role.position_z))
                    // HACK
                    .rotate_axis_angle_local(&Vec3::UP, std::f32::consts::PI);

                let role_controller = entity.get_component(IRoleController::uuid()).unwrap();
                let role_controller = role_controller
                    .query_interface::<IRoleController>()
                    .unwrap();
                if role.sce_proc_id != 0 {
                    role_controller.get().set_active(true);
                    role_controller.get().set_proc_id(role.sce_proc_id as i32);
                }

                entities.push(entity);
            }
        }

        for e in entities {
            self.scene.add_entity(e);
        }
    }
}

struct ScnNodeTypes;
impl ScnNodeTypes {
    pub const LADDER: u16 = 15;
    pub const ITEM_TRIGGER2: u16 = 11;
    pub const ITEM_TRIGGER: u16 = 16;
    pub const AABB_TRIGGER: u16 = 20;
    pub const TRIGGER_TARGET: u16 = 22;
    pub const TRIGGER_SOURCE: u16 = 23;
    pub const ITEM_TRIGGER3: u16 = 33;

    // Elevators
    pub const LADDER2: u16 = 40;
}

pub struct Nav {
    nav_file: NavFile,
    block_sizes: Vec<(f32, f32)>,
}

impl Nav {
    pub fn new(nav_file: NavFile) -> Self {
        let mut block_sizes = vec![];
        for i in 0..nav_file.maps.len() {
            let area = Vec3::sub(&nav_file.maps[i].max_coord, &nav_file.maps[i].min_coord);
            let width = nav_file.maps[i].width + 1;
            let height = nav_file.maps[i].height + 1;
            block_sizes.push((area.x / width as f32, area.z / height as f32))
        }
        Self {
            nav_file,
            block_sizes,
        }
    }

    pub fn round_nav_coord(&self, layer: usize, nav_coord: (f32, f32)) -> (i32, i32) {
        let nav_coord_floor = (
            (nav_coord.0.floor() as i32).clamp(0, self.nav_file.maps[layer].width as i32 - 1),
            (nav_coord.1.floor() as i32).clamp(0, self.nav_file.maps[layer].height as i32 - 1),
        );

        nav_coord_floor
    }

    pub fn layer_count(&self) -> usize {
        self.nav_file.maps.len()
    }

    pub fn get_map_size(&self, layer: usize) -> (usize, usize) {
        (
            self.nav_file.maps[layer].width as usize,
            self.nav_file.maps[layer].height as usize,
        )
    }

    pub fn get(&self, layer: usize, nav_coord_x: i32, nav_coord_z: i32) -> Option<NavMapPoint> {
        if nav_coord_x < 0
            || nav_coord_z < 0
            || nav_coord_x as u32 >= self.nav_file.maps[layer].width
            || nav_coord_z as u32 >= self.nav_file.maps[layer].height
        {
            None
        } else {
            Some(self.nav_file.maps[layer].map[nav_coord_z as usize][nav_coord_x as usize])
        }
    }

    pub fn check_connectivity(
        &self,
        layer: usize,
        nav_coord_src: (i32, i32),
        nav_coord_dest: (i32, i32),
    ) -> bool {
        let mut visited = HashSet::new();
        self.check_connectivity_internal(layer, nav_coord_src, nav_coord_dest, &mut visited, 6)
    }

    pub fn print_map(&self) {
        for layer in 0..self.nav_file.maps.len() {
            for j in 0..self.nav_file.maps[layer].height {
                for i in 0..self.nav_file.maps[layer].width {
                    print!(
                        "{} ",
                        self.nav_file.maps[layer].map[j as usize][i as usize].distance_to_border
                    );
                }

                println!();
            }
            println!("==========");
        }
    }

    fn check_connectivity_internal(
        &self,
        layer: usize,
        nav_coord_src: (i32, i32),
        nav_coord_dest: (i32, i32),
        visited: &mut HashSet<(i32, i32)>,
        remaining_steps: i32,
    ) -> bool {
        if nav_coord_src == nav_coord_dest {
            return true;
        }

        if remaining_steps < 0 {
            return false;
        }

        visited.insert(nav_coord_src);

        // Obviously we can optimize this
        let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for d in &directions {
            let target_coord = (nav_coord_src.0 + d.0, nav_coord_src.1 + d.1);
            if visited.contains(&target_coord) {
                continue;
            }

            if let Some(point) = self.get(layer, target_coord.0, target_coord.1) {
                if point.distance_to_border != 0
                    && self.check_connectivity_internal(
                        layer,
                        target_coord,
                        nav_coord_dest,
                        visited,
                        remaining_steps - 1,
                    )
                {
                    return true;
                }
            }
        }

        visited.remove(&nav_coord_src);
        false
    }
}

pub struct SceNavTrigger {
    nav_coord_min: (i32, i32),
    nav_coord_max: (i32, i32),
    node_type: u16,
    layer: u16,
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

pub struct LadderTrigger {
    position: Vec3,
    nav_coord1: (i32, i32),
    nav_coord2: (i32, i32),
    switch_layer: bool,
    sce_proc_id: u32,
}

#[allow(dead_code)]
pub struct TriggerTarget {}

pub enum LadderTestResult {
    SceProc(u32),
    NewPosition((bool, Vec3)),
}
