use super::{
    cvd_entity,
    mechanism::{Activation, Mechanism, SwitchInteraction, SwitchPhase},
};
use crate::openpal3::asset_manager::AssetManager;
use crate::openpal3::comdef::IRoleController;
use crate::openpal3::loaders::nav_loader::{NavFile, NavMapPoint};
use crate::openpal3::loaders::scn_loader::ScnFile;
use crate::openpal3::scene::{RoleAnimationRepeatMode, RoleController, RoleState};
use crate::openpal3::states::persistent_state::PersistentState;
use crosscom::ComRc;
use radiance::comdef::{
    IComponentImpl, IEntity, IEntityExt, IScene, ISceneExt, IStaticMeshComponent,
};
use radiance::components::mesh::StaticMeshComponent;
use radiance::math::Vec3;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
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
    mechanisms: RefCell<Vec<Mechanism>>,
    mechanism_procs: RefCell<Vec<u32>>,
    mechanism_player: RefCell<Option<(ComRc<IEntity>, usize)>>,
}

ComObject_ScnSceneComponent!(super::ScnScene);

impl IComponentImpl for ScnScene {
    fn on_loading(&self) {
        self.load_objects();
        self.load_roles();
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
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
        let mechanisms = if asset_mgr.game() == crate::GameType::PAL3 {
            scn_file
                .nodes
                .iter()
                .filter_map(Mechanism::from_node)
                .collect()
        } else {
            vec![]
        };
        Self {
            scene,
            asset_mgr: asset_mgr.clone(),
            cpk_name: cpk_name.to_string(),
            scn_name: scn_name.to_string(),
            scn_file,
            nav: Nav::new(nav_file, asset_mgr.game() == crate::GameType::PAL3),
            nav_triggers: RefCell::new(vec![]),
            aabb_triggers: RefCell::new(vec![]),
            item_triggers: RefCell::new(vec![]),
            ladder_triggers: RefCell::new(vec![]),
            mechanisms: RefCell::new(mechanisms),
            mechanism_procs: RefCell::new(vec![]),
            mechanism_player: RefCell::new(None),
        }
    }

    pub fn snapshot_mechanisms(&self, state: &mut PersistentState) {
        state.set_mechanism_states(
            self.name(),
            self.sub_name(),
            self.mechanisms
                .borrow()
                .iter()
                .map(|m| (m.id, m.saved_state()))
                .collect(),
        );
    }

    pub fn restore_mechanisms(&self, state: &PersistentState) {
        self.mechanism_procs.borrow_mut().clear();
        self.nav.enabled_groups.borrow_mut().clear();
        self.nav.floors.borrow_mut().clear();
        let saved = state.mechanism_states(self.name(), self.sub_name());
        for mechanism in self.mechanisms.borrow_mut().iter_mut() {
            if let Some(state) = saved.and_then(|s| s.get(&mechanism.id)) {
                mechanism.restore(state);
            } else if mechanism.kind == 22 {
                if let Some(node) = self.scn_file.nodes.iter().find(|n| n.index == mechanism.id) {
                    let link = &node.b14a;
                    if link.len() >= 33 && link[0] != 0 && link[32] != 255 {
                        let end = link[..32].iter().position(|b| *b == 0).unwrap_or(32);
                        let sub_scene = String::from_utf8_lossy(&link[..end]);
                        if state
                            .mechanism_states(self.name(), &sub_scene)
                            .and_then(|states| states.get(&(link[32] as u16)))
                            .is_some_and(|s| s.switched != 0)
                        {
                            mechanism.restore_raised();
                        }
                    }
                }
            }
            if mechanism.kind == 31 && !mechanism.active && self.name().eq_ignore_ascii_case("m09")
            {
                self.nav.enabled_groups.borrow_mut().insert(2);
            }
            self.apply_mechanism_pose(mechanism, mechanism.pose_at_end());
        }
        self.sync_platform_objects();
    }

    fn apply_mechanism_pose(&self, mechanism: &Mechanism, end: bool) {
        if mechanism.kind == 31
            && self.name().eq_ignore_ascii_case("m09")
            && mechanism.state.native.as_ref().is_some_and(|s| s.completed)
        {
            self.nav.enabled_groups.borrow_mut().insert(2);
        }
        if let Some(entity) = self.get_root_object(mechanism.id as i32) {
            if let Some(native) = &mechanism.state.native {
                let transform = entity.transform();
                let mut transform = transform.borrow_mut();
                let mut position = mechanism.position;
                position.y = mechanism.state.height;
                transform
                    .set_position(&position)
                    .clear_rotation()
                    .rotate_axis_angle_local(&Vec3::UP, native.rotation.to_radians());
                entity.set_visible(native.active);
                drop(transform);
                if mechanism.kind == 7 {
                    self.update_mechanism_floor(mechanism, &entity, native.rotation);
                }
            } else if mechanism.kind == 22 {
                let transform = entity.transform();
                let mut transform = transform.borrow_mut();
                let mut position = transform.position();
                position.y = mechanism.state.height;
                transform.set_position(&position);
                drop(transform);
                let rotation = self
                    .scn_file
                    .nodes
                    .iter()
                    .find(|n| n.index == mechanism.id)
                    .map(|n| n.rotation)
                    .unwrap_or(0.);
                self.update_mechanism_floor(mechanism, &entity, rotation);
            } else if mechanism
                .state
                .interaction
                .as_ref()
                .is_none_or(|i| matches!(i.phase, SwitchPhase::Animating))
            {
                let time = mechanism.animation_time().unwrap_or_else(|| {
                    if end {
                        cvd_entity::animation_duration(&entity)
                    } else {
                        0.
                    }
                });
                cvd_entity::set_animation_time(&entity, time);
            }
        }
    }

    fn update_mechanism_floor(
        &self,
        mechanism: &Mechanism,
        entity: &ComRc<IEntity>,
        rotation: f32,
    ) {
        let mut floors = self.nav.floors.borrow_mut();
        let position = entity.transform().borrow().position();
        if let Some(floor) = floors.get_mut(&mechanism.id) {
            floor.position = position;
            floor.layer = mechanism.layer as usize;
            floor.rotation = rotation.to_radians();
            floor.enabled = mechanism.provides_floor();
        } else if let Some((min, max)) = Self::mechanism_bounds(entity) {
            if let Some(block) = self.nav.block_sizes.get(mechanism.layer as usize) {
                let center = Vec3::scalar_mul(0.5, &Vec3::add(&min, &max));
                let mut size = Vec3::sub(&max, &min);
                // Native 0x42a0d3/0x42a12a: long/cross-axis footprint adjustments.
                let (extend, inset) = if mechanism.kind == 7 {
                    (4.5, 2.5)
                } else {
                    (1.5, 1.5)
                };
                if size.x > size.z {
                    size.x += extend * block.0;
                    size.z -= inset * block.1;
                } else {
                    size.x -= inset * block.0;
                    size.z += extend * block.1;
                }
                let half = Vec3::scalar_mul(0.5, &size);
                floors.insert(
                    mechanism.id,
                    MechanismFloor {
                        layer: mechanism.layer as usize,
                        position,
                        rotation: rotation.to_radians(),
                        min: Vec3::sub(&center, &half),
                        max: Vec3::add(&center, &half),
                        enabled: mechanism.provides_floor(),
                    },
                );
            }
        }
    }

    fn sync_platform_objects(&self) {
        let mut mechanisms = self.mechanisms.borrow_mut();
        let carried: Vec<_> = mechanisms
            .iter()
            .filter(|m| m.kind == 22)
            .filter_map(|m| {
                let node = self.scn_file.nodes.iter().find(|n| n.index == m.id)?;
                let id = node.ladder_nav_coord2.0 as u16;
                (id != m.id).then_some((id, m.state.height, m.layer))
            })
            .collect();
        for (id, height, layer) in carried {
            if let Some(entity) = self.get_root_object(id as i32) {
                let mut position = entity.transform().borrow().position();
                position.y = height;
                entity.transform().borrow_mut().set_position(&position);
                if let Some(mechanism) = mechanisms.iter_mut().find(|m| m.id == id) {
                    mechanism.layer = layer;
                    mechanism.state.layer = Some(layer);
                }
            }
        }
    }

    fn mechanism_bounds(entity: &ComRc<IEntity>) -> Option<(Vec3, Vec3)> {
        let component = entity.get_component(IStaticMeshComponent::uuid())?;
        let mesh = component.query_interface::<IStaticMeshComponent>()?;
        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut found = false;
        for geometry in mesh.inner::<StaticMeshComponent>().get_geometries().iter() {
            if let Some((a, b)) = geometry.vertices.aabb_min_max() {
                min = Vec3::new(min.x.min(a[0]), min.y.min(a[1]), min.z.min(a[2]));
                max = Vec3::new(max.x.max(b[0]), max.y.max(b[1]), max.z.max(b[2]));
                found = true;
            }
        }
        found.then_some((min, max))
    }

    pub fn set_mechanism_active(&self, id: u16, active: bool) {
        if let Some(mechanism) = self.mechanisms.borrow_mut().iter_mut().find(|m| m.id == id) {
            mechanism.set_active(active);
            self.apply_mechanism_pose(mechanism, mechanism.pose_at_end());
        }
    }

    pub fn trigger_mechanism(&self, id: u16) {
        let mut mechanisms = self.mechanisms.borrow_mut();
        if let Some(mechanism) = mechanisms.iter_mut().find(|m| m.id == id) {
            if mechanism.activate(true) == Activation::Started {
                if let Some(proc_id) = mechanism.activation_proc() {
                    self.mechanism_procs.borrow_mut().push(proc_id);
                }
                self.apply_mechanism_pose(mechanism, false);
            }
        } else {
            log::warn!(
                "Unsupported PAL3 native Trigger({}) in {}/{}",
                id,
                self.name(),
                self.sub_name()
            );
        }
    }

    pub fn set_mechanism_player(&self, player: Option<(ComRc<IEntity>, usize)>) {
        *self.mechanism_player.borrow_mut() = player;
    }

    pub fn mechanism_input_locked(&self) -> bool {
        self.mechanisms
            .borrow()
            .iter()
            .any(|m| m.state.interaction.is_some())
    }

    fn capture_platform_rider(&self, mechanism: &mut Mechanism) {
        let Some(platform) = &mut mechanism.state.platform else {
            return;
        };
        if !platform.boarding_pending {
            return;
        }
        platform.boarding_pending = false;
        let player = self.mechanism_player.borrow();
        let Some((player, layer)) = player.as_ref() else {
            return;
        };
        if self
            .nav
            .floor_at(*layer, &player.transform().borrow().position())
            == Some(mechanism.id)
        {
            platform.rider = Some(player.name());
        }
    }

    fn mechanism_role(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.scene.entities().into_iter().find(|e| e.name() == name)
    }

    pub fn take_mechanism_procs(&self) -> Vec<u32> {
        std::mem::take(&mut *self.mechanism_procs.borrow_mut())
    }

    pub fn switch_state(&self, id: u16) -> u8 {
        self.mechanisms
            .borrow()
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.state.switched)
            .or_else(|| {
                self.scn_file
                    .nodes
                    .iter()
                    .find(|n| n.index == id)
                    .map(|n| n.switch_state)
            })
            .unwrap_or(0)
    }

    /// Outer Option marks a consumed interaction, even when no script is called.
    pub fn interact_mechanism(&self, layer: usize, position: &Vec3) -> Option<Option<u32>> {
        if self.mechanism_input_locked() {
            return Some(None);
        }
        for mechanism in self.mechanisms.borrow_mut().iter_mut() {
            let visible = self
                .get_root_object(mechanism.id as i32)
                .map(|e| e.visible())
                .unwrap_or(mechanism.active);
            if mechanism.kind != 23
                || !visible
                || mechanism.layer as usize != layer
                || Vec3::sub(
                    position,
                    &self
                        .get_root_object(mechanism.id as i32)
                        .map(|e| e.transform().borrow().position())
                        .unwrap_or(mechanism.position),
                )
                .norm2()
                    >= 200. * 200.
            {
                continue;
            }
            mechanism.active = visible;
            match mechanism.activate(false) {
                Activation::Ignored => continue,
                Activation::Script(proc_id) => return Some(Some(proc_id)),
                Activation::Started => {
                    self.apply_mechanism_pose(mechanism, false);
                    return Some(None);
                }
                Activation::Interact => {
                    let player = self.mechanism_player.borrow();
                    let Some((player, _)) = player.as_ref() else {
                        continue;
                    };
                    let entity = self.get_root_object(mechanism.id as i32)?;
                    let to = entity.transform().borrow().position();
                    let radius = cvd_entity::model_bounds(&entity)
                        .or_else(|| Self::mechanism_bounds(&entity))
                        .map(|(min, max)| {
                            let (min, max) = radiance::math::transform_aabb(
                                [min.x, min.y, min.z],
                                [max.x, max.y, max.z],
                                entity.transform().borrow().matrix(),
                            );
                            (max[0] - min[0]).max(max[2] - min[2]) * 0.5
                        })
                        .unwrap_or(0.);
                    let direction = Vec3::sub(&to, position);
                    let distance = direction.norm();
                    let target = if distance > 0. {
                        Vec3::add(
                            position,
                            &Vec3::scalar_mul((distance - radius) / distance, &direction),
                        )
                    } else {
                        *position
                    };
                    // The native walk-to is horizontal; Y comes from navigation.
                    mechanism.state.interaction = Some(SwitchInteraction {
                        player: player.name(),
                        target: Vec3::new(target.x, position.y, target.z),
                        phase: SwitchPhase::Approach,
                    });
                    let remaining = Vec3::sub(&target, position);
                    if remaining.x.hypot(remaining.z) < 24. {
                        mechanism.begin_switch_action();
                    }
                    return Some(None);
                }
            }
        }
        None
    }

    pub fn update_mechanisms(&self, delta_sec: f32) -> Vec<u32> {
        let mut procs = self.take_mechanism_procs();
        let mut links = vec![];
        for mechanism in self.mechanisms.borrow_mut().iter_mut() {
            if mechanism.kind == 22 || mechanism.state.native.is_some() {
                self.apply_mechanism_pose(mechanism, mechanism.pose_at_end());
            }
            self.capture_platform_rider(mechanism);
            self.update_switch_interaction(mechanism, delta_sec.max(0.));
            let duration = self
                .get_root_object(mechanism.id as i32)
                .map(|e| cvd_entity::animation_duration(&e))
                .unwrap_or(0.);
            let animating = mechanism.animation_time().is_some();
            let completed = mechanism.update(delta_sec, duration);
            if mechanism.kind == 22 || mechanism.state.native.is_some() || animating {
                self.apply_mechanism_pose(mechanism, completed);
            }
            if completed {
                if let Some(platform) = &mut mechanism.state.platform {
                    if let Some(name) = platform.rider.take() {
                        if let Some(player) = self.mechanism_role(&name) {
                            Self::carry_role(&player, mechanism.state.height, mechanism.layer);
                        }
                    }
                    platform.boarding_pending = false;
                }
                if let Some(interaction) = mechanism.state.interaction.take() {
                    if let Some(player) = self.mechanism_role(&interaction.player) {
                        if let Some(controller) = RoleController::get_role_controller(player) {
                            controller.inner::<RoleController>().idle();
                        }
                    }
                }
                if let Some(proc_id) = mechanism.completion_proc() {
                    procs.push(proc_id);
                }
                if mechanism.target != u16::MAX {
                    links.push(mechanism.target);
                }
            }
            if let Some(name) = mechanism
                .state
                .platform
                .as_ref()
                .and_then(|p| p.rider.as_ref())
            {
                if let Some(player) = self.mechanism_role(name) {
                    Self::carry_role(&player, mechanism.state.height, mechanism.layer);
                }
            }
        }
        self.sync_platform_objects();
        // Start links after releasing the state borrow; cycles advance at most once per tick.
        for target in links {
            self.trigger_mechanism(target);
        }
        procs.extend(self.take_mechanism_procs());
        procs
    }

    fn carry_role(player: &ComRc<IEntity>, height: f32, layer: u16) {
        let mut position = player.transform().borrow().position();
        position.y = height;
        player.transform().borrow_mut().set_position(&position);
        if let Some(controller) = RoleController::get_role_controller(player.clone()) {
            controller
                .inner::<RoleController>()
                .set_nav_layer(layer as usize);
        }
    }

    fn update_switch_interaction(&self, mechanism: &mut Mechanism, delta_sec: f32) {
        let Some(interaction) = mechanism.state.interaction.as_ref() else {
            return;
        };
        let Some(player) = self.mechanism_role(&interaction.player) else {
            // Scene changes/removal must not leave the interaction lock latched.
            mechanism.state.interaction = None;
            return;
        };
        let controller = RoleController::get_role_controller(player.clone());
        match interaction.phase {
            SwitchPhase::Approach => {
                let mut position = player.transform().borrow().position();
                let remain = Vec3::sub(&interaction.target, &position);
                let distance = (remain.x * remain.x + remain.z * remain.z).sqrt();
                // Original basedata/scene.txt: PWalkSpeed=100.
                let step = 100. * delta_sec;
                let arrived = distance <= step;
                if distance > 0. {
                    let step = step.min(distance);
                    position.x += remain.x * step / distance;
                    position.z += remain.z * step / distance;
                    if let Some(point) = self.nav.point_at(mechanism.layer as usize, &position) {
                        position.y = point.height;
                    }
                    if let Some(controller) = &controller {
                        controller.inner::<RoleController>().walk();
                    }
                }
                let to = self
                    .get_root_object(mechanism.id as i32)
                    .map(|e| e.transform().borrow().position())
                    .unwrap_or(mechanism.position);
                let transform = player.transform();
                let mut transform = transform.borrow_mut();
                transform.set_position(&position);
                if to.x != position.x || to.z != position.z {
                    transform.look_at(&Vec3::new(to.x, position.y, to.z));
                }
                if arrived {
                    mechanism.begin_switch_action();
                }
            }
            SwitchPhase::Action { started } => {
                let position = player.transform().borrow().position();
                let to = self
                    .get_root_object(mechanism.id as i32)
                    .map(|e| e.transform().borrow().position())
                    .unwrap_or(mechanism.position);
                if to.x != position.x || to.z != position.z {
                    player
                        .transform()
                        .borrow_mut()
                        .look_at(&Vec3::new(to.x, position.y, to.z));
                }
                let playing = if let Some(controller) = &controller {
                    let controller = controller.inner::<RoleController>();
                    if !controller.is_active() {
                        false
                    } else if !started {
                        // Native action 11 (0x414e30) resolves to c10.
                        controller.try_play_anim("c10", RoleAnimationRepeatMode::Repeat(1))
                    } else {
                        matches!(
                            controller.state(),
                            RoleState::PlayingAnimation | RoleState::AnimationHolding
                        )
                    }
                } else {
                    false
                };
                if playing {
                    if let Some(interaction) = &mut mechanism.state.interaction {
                        interaction.phase = SwitchPhase::Action { started: true };
                    }
                } else {
                    mechanism.start_switch_animation();
                }
            }
            SwitchPhase::Animating => {}
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
        let distance_floor = self
            .nav
            .get(layer, nav_coord_floor.0 as i32, nav_coord_floor.1 as i32)
            .unwrap();
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
        coord.0 >= boundary.0.0
            && coord.1 >= boundary.0.1
            && coord.0 <= boundary.1.0
            && coord.1 <= boundary.1.1
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

                return Some(role_model.unwrap().inner::<RoleController>().proc_id() as u32);
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
        self.nav.get(layer, x as i32, y as i32).unwrap().height
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

    /// All role entities currently in the scene (named `ROLE_<n>`).
    pub fn role_entities(&self) -> Vec<ComRc<IEntity>> {
        self.scene
            .entities()
            .iter()
            .filter(|e| e.name().starts_with("ROLE_"))
            .cloned()
            .collect()
    }

    /// Advance ambient NPC patrols by `delta_sec`. Only roles that were given a
    /// patrol path at load time (non-scripted townsfolk) move; scripted roles
    /// are untouched. Movement is ground-snapped via the role's nav layer.
    pub fn step_patrols(&self, delta_sec: f32) {
        // Default walk speed when the role record carries no per-role speed
        // (0x150 is zero for most patrol roles).
        const DEFAULT_PATROL_SPEED: f32 = 60.;
        for entity in self.role_entities() {
            let rc = match RoleController::get_role_controller(entity.clone()) {
                Some(rc) => rc,
                None => continue,
            };
            let controller = rc.inner::<RoleController>();
            if !controller.has_patrol() {
                continue;
            }
            let target = match controller.patrol_target() {
                Some(t) => t,
                None => continue,
            };
            let layer = controller.nav_layer();
            let position = entity.transform().borrow().position();
            let to = Vec3::new(target.x - position.x, 0., target.z - position.z);
            let dist = (to.x * to.x + to.z * to.z).sqrt();
            let speed = {
                let s = controller.patrol_speed();
                if s > f32::EPSILON {
                    s
                } else {
                    DEFAULT_PATROL_SPEED
                }
            };
            let step = speed * delta_sec;

            if dist <= step.max(1.0) {
                controller.advance_patrol();
                continue;
            }

            let dir = Vec3::new(to.x / dist, 0., to.z / dist);
            let mut new_position = Vec3::add(&position, &Vec3::scalar_mul(step, &dir));
            let nav_coord = self.scene_coord_to_nav_coord(layer, &new_position);
            new_position.y = self.get_height(layer, nav_coord);

            let look_at = Vec3::new(target.x, new_position.y, target.z);
            entity
                .transform()
                .borrow_mut()
                .look_at(&look_at)
                .set_position(&new_position);
            controller.walk();
        }
    }

    fn test_sphere_aabb(s: &Vec3, r: f32, aabb1: &Vec3, aabb2: &Vec3) -> bool {
        macro_rules! dist_sqr {
            ($s: expr_2021, $min: expr_2021, $max: expr_2021) => {
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
                    match self.asset_mgr.load_scn_pol(
                        &self.cpk_name,
                        &self.scn_file.scn_base_name,
                        &obj.name,
                        self.scn_file.is_night,
                        obj.index,
                    ) {
                        Some(p) => {
                            entity = Some(p);
                        }
                        _ => {
                            match self.asset_mgr.load_scn_cvd(
                                &self.cpk_name,
                                &self.scn_file.scn_base_name,
                                &obj.name,
                                self.scn_file.is_night,
                                obj.index,
                            ) {
                                Some(c) => {
                                    entity = Some(c);
                                }
                                _ => {
                                    log::error!("Cannot load object: {}", obj.name);
                                }
                            }
                        }
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
                    entity = self.build_scene_effect(obj);
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
                | ScnNodeTypes::ITEM_TRIGGER3 => {
                    self.item_triggers.borrow_mut().push(SceItemTrigger {
                        coord: obj.position,
                        sce_proc_id: obj.sce_proc_id,
                    });
                }
                ScnNodeTypes::TRIGGER_SOURCE if self.asset_mgr.game() != crate::GameType::PAL3 => {
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
                if self.asset_mgr.game() == crate::GameType::PAL3
                    && matches!(obj.node_type, 7 | 8 | 22 | 23 | 31)
                {
                    p.set_visible(obj.active != 0);
                }
                entities.push(p);
            }
        }

        for entity in entities {
            self.scene.add_entity(entity);
        }
    }

    /// Build a PAL3 scene "effect" entity for a `+`-prefixed node.
    ///
    /// These nodes mark `EffectScn` effects (candles, oil lamps, torches,
    /// fires). The visual-effect class is node_type 17 ("+3"); the effect
    /// variant is selected by `dw184[3]` (record offset 0x190). We build
    /// the static holder mesh (where the effect has one) plus an animated
    /// additive flame billboard.
    fn build_scene_effect(
        &self,
        obj: &crate::openpal3::loaders::scn_loader::ScnNode,
    ) -> Option<ComRc<IEntity>> {
        if obj.node_type != 17 {
            return None;
        }
        let effect_id = obj.dw184.get(3).copied().unwrap_or(0);
        self.asset_mgr.load_scene_effect(effect_id, obj.index)
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
                let layer = (role.nav_layer as usize).min(self.nav.layer_count().saturating_sub(1));
                let nav_coord = self.scene_coord_to_nav_coord(
                    layer,
                    &Vec3::new(role.position_x, 0., role.position_z),
                );
                let height = self.get_height(layer, nav_coord);
                // Role model's authored forward is +Z (SOUTH); apply the role's
                // facing (degrees, 0x44) on top of that base. Sign mirrors
                // SceCommandRoleTurnFace (`-degree.to_radians()`). The base is a
                // single tunable constant for visual calibration.
                const ROLE_FACING_BASE: f32 = std::f32::consts::PI;
                let facing = ROLE_FACING_BASE - role.facing.to_radians();
                entity
                    .transform()
                    .borrow_mut()
                    .set_position(&Vec3::new(role.position_x, height, role.position_z))
                    .rotate_axis_angle_local(&Vec3::UP, facing);

                let role_controller = entity.get_component(IRoleController::uuid()).unwrap();
                let role_controller = role_controller
                    .query_interface::<IRoleController>()
                    .unwrap();
                {
                    let rc = role_controller.inner::<RoleController>();
                    rc.set_nav_layer(layer);
                    // Only non-scripted ambient NPCs auto-patrol, so they don't
                    // fight SCE RoleMoveTo/RolePathTo. Mark them active so their
                    // walk animation loops (driven by RoleController::on_updating).
                    if role.sce_proc_id == 0 && role.patrol_path.len() >= 2 {
                        rc.set_patrol(
                            role.patrol_path.clone(),
                            role.path_mode,
                            role.move_speed as f32,
                        );
                        rc.set_active(true);
                    }
                }
                if role.sce_proc_id != 0 {
                    {
                        let rc = role_controller.inner::<RoleController>();
                        rc.set_active(true);
                        rc.set_proc_id(role.sce_proc_id as i32);
                    };
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
    native: bool,
    enabled_groups: RefCell<HashSet<u8>>,
    floors: RefCell<BTreeMap<u16, MechanismFloor>>,
}

struct MechanismFloor {
    layer: usize,
    position: Vec3,
    rotation: f32,
    min: Vec3,
    max: Vec3,
    enabled: bool,
}

impl MechanismFloor {
    fn contains_cell(&self, x: i32, z: i32, origin: &Vec3, block: (f32, f32)) -> bool {
        let (sin, cos) = self.rotation.sin_cos();
        let corners = [
            (self.min.x, self.min.z),
            (self.max.x, self.min.z),
            (self.max.x, self.max.z),
            (self.min.x, self.max.z),
        ]
        .map(|(local_x, local_z)| {
            (
                ((self.position.x + cos * local_x + sin * local_z - origin.x) / block.0) as i32,
                ((self.position.z - sin * local_x + cos * local_z - origin.z) / block.1) as i32,
            )
        });
        if x < corners.iter().map(|p| p.0).min().unwrap()
            || x >= corners.iter().map(|p| p.0).max().unwrap()
        {
            return false;
        }
        let mut hits = [0; 4];
        let mut count = 0;
        for i in 0..4 {
            let a = corners[i];
            let b = corners[(i + 1) % 4];
            if a.0 == b.0 {
                if x == a.0 {
                    hits[0] = a.1;
                    hits[1] = b.1;
                    count = 2;
                    break;
                }
            } else if x > a.0.min(b.0) && x <= a.0.max(b.0) {
                let hit = ((b.1 - a.1) as f32 * (x - a.0) as f32 / (b.0 - a.0) as f32
                    + a.1 as f32
                    + 0.5) as i32;
                if hit >= a.1.min(b.1) && hit <= a.1.max(b.1) {
                    hits[count] = hit;
                    count += 1;
                }
            }
        }
        if count != 2 {
            return false;
        }
        let mut min = hits[0].min(hits[1]);
        let mut max = hits[0].max(hits[1]);
        // Native rasterizer 0x42ff40 uses half-open spans, with a five-cell
        // vertical span for narrow intersections, rather than cell-center tests.
        if max - min < 4 {
            let center = (min + max) as f32 * 0.5;
            min = (center - 2.5) as i32;
            max = (center + 2.5) as i32;
        }
        z >= min && z < max
    }
}

impl Nav {
    fn floor_at(&self, layer: usize, position: &Vec3) -> Option<u16> {
        let map = self.nav_file.maps.get(layer)?;
        let block = *self.block_sizes.get(layer)?;
        let x = ((position.x - map.min_coord.x) / block.0) as i32;
        let z = ((position.z - map.min_coord.z) / block.1) as i32;
        if x < 0 || z < 0 || x >= map.width as i32 || z >= map.height as i32 {
            return None;
        }
        self.floors.borrow().iter().rev().find_map(|(id, floor)| {
            (floor.enabled
                && floor.layer == layer
                && floor.contains_cell(x, z, &map.min_coord, block))
            .then_some(*id)
        })
    }

    fn point_at(&self, layer: usize, position: &Vec3) -> Option<NavMapPoint> {
        let map = self.nav_file.maps.get(layer)?;
        let block = *self.block_sizes.get(layer)?;
        self.get(
            layer,
            ((position.x - map.min_coord.x) / block.0) as i32,
            ((position.z - map.min_coord.z) / block.1) as i32,
        )
    }

    pub fn new(nav_file: NavFile, native: bool) -> Self {
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
            native,
            enabled_groups: RefCell::new(HashSet::new()),
            floors: RefCell::new(BTreeMap::new()),
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
        let map = self.nav_file.maps.get(layer)?;
        if nav_coord_x < 0
            || nav_coord_z < 0
            || nav_coord_x as u32 >= map.width
            || nav_coord_z as u32 >= map.height
        {
            None
        } else {
            let mut point = map.map[nav_coord_z as usize][nav_coord_x as usize];
            if self.native {
                // Native NAV cells contain separate clearance/group bytes (0x4214eb).
                let clearance = point.distance_to_border as u8;
                let group = (point.distance_to_border >> 8) as u8;
                point.distance_to_border =
                    if group == 1 || self.enabled_groups.borrow().contains(&group) {
                        clearance.max(2) as u32
                    } else if clearance < 2 {
                        0
                    } else {
                        clearance as u32
                    };
                for floor in self.floors.borrow().values() {
                    if floor.enabled
                        && floor.layer == layer
                        && floor.contains_cell(
                            nav_coord_x,
                            nav_coord_z,
                            &map.min_coord,
                            self.block_sizes[layer],
                        )
                    {
                        point.height = floor.position.y;
                        point.distance_to_border = point.distance_to_border.max(2);
                    }
                }
            }
            Some(point)
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
