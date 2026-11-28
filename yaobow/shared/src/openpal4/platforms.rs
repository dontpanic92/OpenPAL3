use crosscom::ComRc;
use fileformats::pal4::gob::{
    GobEntry, GobFixedProperty as Fixed, GobFloatProperty as Float, GobGomTaskProperty as Task,
    GobMachineProperty as Machine, GobObjectType, GobParameterBlock, GobProperty,
};
use radiance::{
    comdef::{IEntity, IEntityExt},
    components::collision::CollisionWorldComponent,
    math::Vec3,
    scene::ISceneExt,
};

use super::vm_context::Pal4VmContext;
use super::{
    object_component::Pal4ObjectComponent,
    scene::{Pal4Scene, object_component},
};
use crate::scripting::angelscript::ScriptVm;

const BOARD_HEIGHT: f32 = 15.;

pub(crate) fn supported_metadata(entry: &GobEntry) -> bool {
    let integer = |name, default| {
        entry
            .get_property(name)
            .or_else(|| entry.get_parameter(name))
            .and_then(GobProperty::value_i32)
            .unwrap_or(default)
    };
    entry.active != 0
        && entry
            .research_function
            .to_string()
            .is_ok_and(|name| name.is_empty())
        && matches!(integer(GobParameterBlock::FLOAT, -1), 1 | 2)
        && matches!(integer(Float::AUTO_TOUCH, 0), 0 | 1)
        && matches!(integer(Float::TOUCH_TYPE, 1), 0 | 1)
        && matches!(integer(Fixed::STORE, 0), 0 | 1)
        && matches!(integer(Fixed::CLIP, 0), 0 | 1)
        && matches!(integer(Fixed::AUTO_DISAPPEAR, 0), 0 | 1)
        && integer(Fixed::MARKER, 0) == 0
        && integer(Fixed::HIDE, 0) == 0
        && integer(Fixed::VALIDFOR, -1) == -1
        && !entry.parameters.iter().any(|property| {
            if let GobProperty::GobPropertyObjectArray(array) = property {
                property.name() == Machine::CONDITION
                    && array.0.iter().any(|condition| {
                        condition
                            .get_property(Machine::PROPID)
                            .and_then(GobProperty::value_i32)
                            .is_some_and(|id| id >= 0)
                    })
            } else {
                false
            }
        })
}

pub(crate) fn supported_platform(entry: &GobEntry, kind: u32, has_motion: bool) -> bool {
    kind == GobObjectType::FLOAT
        && supported_metadata(entry)
        && (has_motion
            || (entry
                .get_property(Fixed::CLIP)
                .or_else(|| entry.get_parameter(Fixed::CLIP))
                .and_then(GobProperty::value_i32)
                .unwrap_or(0)
                == 0
                && entry
                    .get_property(Fixed::AUTO_DISAPPEAR)
                    .or_else(|| entry.get_parameter(Fixed::AUTO_DISAPPEAR))
                    .and_then(GobProperty::value_i32)
                    .unwrap_or(0)
                    == 0
                && entry
                    .get_property(Fixed::STORE)
                    .or_else(|| entry.get_parameter(Fixed::STORE))
                    .and_then(GobProperty::value_i32)
                    .unwrap_or(0)
                    == 0
                && (ScriptPlatform::from_entry(entry, kind).is_some()
                    || (entry.rotation[0] == 0.
                        && entry.rotation[2] == 0.
                        && !entry.parameters.iter().any(|property| {
                            property.name().starts_with(Machine::TASK_LIST_PREFIX)
                        })
                        && entry
                            .get_property(Fixed::RESEARCH_NUM)
                            .and_then(GobProperty::value_i32)
                            .unwrap_or(-1)
                            == -1))))
}

enum ScriptPlatformPhase {
    Idle,
    Waiting(f32),
    Running,
    Settling,
}

pub(crate) struct ScriptPlatform {
    function: String,
    wait: bool,
    delay: f32,
    remaining: i32,
    phase: ScriptPlatformPhase,
}

impl ScriptPlatform {
    pub(crate) fn function_name(&self) -> &str {
        &self.function
    }

    pub(crate) fn from_entry(entry: &GobEntry, kind: u32) -> Option<Self> {
        if kind != GobObjectType::FLOAT || !supported_metadata(entry) {
            return None;
        }
        let integer = |name, default| {
            entry
                .get_property(name)
                .or_else(|| entry.get_parameter(name))
                .and_then(GobProperty::value_i32)
                .unwrap_or(default)
        };
        if integer(Fixed::CLIP, 0) != 0
            || integer(Fixed::STORE, 0) != 0
            || integer(Fixed::AUTO_DISAPPEAR, 0) != 0
            || integer(Float::TOUCH_TYPE, 1) != 0
            || integer(Float::AUTO_TOUCH, 0) != 0
        {
            return None;
        }
        let tasks: Vec<_> = entry
            .parameters
            .iter()
            .filter_map(|property| match property {
                GobProperty::GobPropertyObjectArray(array)
                    if property.name().starts_with(Machine::TASK_LIST_PREFIX) =>
                {
                    Some(&array.0)
                }
                _ => None,
            })
            .flatten()
            .collect();
        let [task] = tasks.as_slice() else {
            return None;
        };
        let get_i32 = |name| task.get_property(name).and_then(GobProperty::value_i32);
        let get_f32 = |name| task.get_property(name).and_then(GobProperty::value_f32);
        let delay = get_f32(Task::WAIT_TIME)?;
        let function = task
            .get_property(Task::SCRIPT_FUNC)?
            .value_string()?
            .to_owned();
        // With no predecessor, start-relative and finish-relative waits coincide.
        if get_i32(Task::TYPE) != Some(2)
            || !matches!(get_i32(Task::WAIT_MODE), Some(1 | 2))
            || !matches!(get_i32(Task::SCRIPT_WAIT), Some(0 | 1))
            || get_f32(Task::EXPT_TIME) != Some(0.)
            || !delay.is_finite()
            || delay < 0.
            || function.is_empty()
        {
            return None;
        }
        Some(Self {
            function,
            wait: get_i32(Task::SCRIPT_WAIT) == Some(1),
            delay,
            remaining: integer(Fixed::RESEARCH_NUM, -1),
            phase: ScriptPlatformPhase::Idle,
        })
    }

    fn board(&mut self) {
        if matches!(self.phase, ScriptPlatformPhase::Idle) && self.remaining != 0 {
            if self.remaining > 0 {
                self.remaining -= 1;
            }
            self.phase = ScriptPlatformPhase::Waiting(0.);
        }
    }

    pub(crate) fn reset(&mut self) {
        // Native task destruction/reset does not destroy the separately registered script.
        self.phase = ScriptPlatformPhase::Idle;
    }

    fn advance(&mut self, delta_sec: f32, scripts_running: bool) -> Option<String> {
        if !delta_sec.is_finite() || delta_sec <= 0. {
            return None;
        }
        match &mut self.phase {
            ScriptPlatformPhase::Waiting(clock) => {
                *clock += delta_sec;
                if *clock > self.delay {
                    self.phase = if self.wait {
                        ScriptPlatformPhase::Running
                    } else {
                        ScriptPlatformPhase::Settling
                    };
                    return Some(self.function.clone());
                }
            }
            ScriptPlatformPhase::Running if !scripts_running => {
                self.phase = ScriptPlatformPhase::Settling;
            }
            ScriptPlatformPhase::Settling => self.phase = ScriptPlatformPhase::Idle,
            _ => {}
        }
        None
    }
}

pub(crate) fn enable_camera_clip(entity: &ComRc<IEntity>) -> bool {
    use radiance::{comdef::IStaticMeshComponent, components::mesh::StaticMeshComponent};
    fn collect(entity: &ComRc<IEntity>, meshes: &mut Vec<ComRc<IStaticMeshComponent>>) -> bool {
        if let Some(mesh) = entity.get_component(IStaticMeshComponent::uuid()) {
            meshes.push(mesh.query_interface::<IStaticMeshComponent>().unwrap());
        } else if entity.get_rendering_component().is_some() {
            log::debug!(
                "Camera clipping unavailable for {}: rendering subtree is not rigid",
                entity.name()
            );
            return false;
        }
        entity.children().iter().all(|child| collect(child, meshes))
    }
    let mut meshes = Vec::new();
    if !collect(entity, &mut meshes) || meshes.is_empty() {
        return false;
    }
    for mesh in &meshes {
        if !mesh
            .inner::<StaticMeshComponent>()
            .enable_camera_clip(1200.)
        {
            log::debug!(
                "Camera clipping unavailable for {}: missing sphere or unsupported renderer/material",
                entity.name()
            );
            for mesh in &meshes {
                mesh.inner::<StaticMeshComponent>().disable_camera_clip();
            }
            return false;
        }
    }
    true
}

impl ScriptVm<Pal4VmContext> {
    pub(crate) fn tick_platform_scripts(&mut self, delta_sec: f32) {
        let (entities, module) = {
            let scene = self.vm_context().scene.borrow();
            (
                scene.scene.find_entities_by_tag(super::scene::TAG_OBJECT),
                scene.module.clone(),
            )
        };
        let Some(module) = module else { return };
        for entity in entities {
            if !entity.enabled() {
                continue;
            }
            let Some(component) = object_component(&entity) else {
                continue;
            };
            let component = component.inner::<Pal4ObjectComponent>();
            if !component.platform.get() {
                continue;
            }
            if let Some(program) = component.platform_script.as_ref() {
                let function = program
                    .borrow_mut()
                    .advance(delta_sec, self.has_task_scripts());
                if let Some(function) = function {
                    self.start_task_script(module.clone(), &function);
                }
            }
        }
    }
}

pub(crate) struct PlatformRide {
    platform: ComRc<IEntity>,
    leader: usize,
    position: Vec3,
    yaw: f32,
    rider_position: Vec3,
}

pub(super) fn yaw(entity: &ComRc<IEntity>) -> f32 {
    if let Some(component) = object_component(entity) {
        if let Some(motion) = component.inner::<Pal4ObjectComponent>().motion.as_ref() {
            if let Some(angle) = motion.borrow().platform_yaw(entity) {
                return angle;
            }
        }
    }
    let transform = entity.transform();
    let transform = transform.borrow();
    transform.matrix()[0][2].atan2(transform.matrix()[2][2])
}

impl Pal4Scene {
    pub(crate) fn clear_platform_rider_for(&self, entity: &ComRc<IEntity>) {
        let mut ride = self.platform_rider.borrow_mut();
        if ride
            .as_ref()
            .is_some_and(|ride| ride.platform.ptr_value() == entity.ptr_value())
        {
            ride.take();
        }
    }
    pub(crate) fn prepare_platform_rider(&self, leader: usize) {
        let previous = self.platform_rider.borrow_mut().take();
        let Some(player) = self.players.get(leader) else {
            return;
        };
        let point = player.transform().borrow().position();
        let previous_platform = previous
            .as_ref()
            .map(|ride| (ride.platform.ptr_value(), ride.leader));
        let origin = Vec3::new(point.x, point.y + BOARD_HEIGHT, point.z);
        let world = self.scene.collision_world();
        let world = world.inner::<CollisionWorldComponent>();
        let retain_stationary = || {
            // Native selection leaves ownership intact on a missed/rejected query.
            // A moved/teleported actor must establish fresh mesh contact.
            if let Some(mut ride) = previous {
                if ride.leader == leader
                    && [point.x, point.y, point.z]
                        == [
                            ride.rider_position.x,
                            ride.rider_position.y,
                            ride.rider_position.z,
                        ]
                    && ride.platform.enabled()
                    && ride.platform.visible()
                {
                    ride.position = ride.platform.transform().borrow().position();
                    ride.yaw = yaw(&ride.platform);
                    *self.platform_rider.borrow_mut() = Some(ride);
                }
            }
        };
        // Native selection queries floating atomics, independently of the static floor.
        let Some((platform, depth)) = world.dynamic_floor_hit(&origin) else {
            retain_stationary();
            return;
        };
        if (BOARD_HEIGHT - depth).abs() > BOARD_HEIGHT {
            retain_stationary();
            return;
        }
        if !platform.visible()
            || !object_component(&platform)
                .is_some_and(|component| component.inner::<Pal4ObjectComponent>().platform.get())
        {
            return;
        }

        let position = platform.transform().borrow().position();
        let yaw = yaw(&platform);
        if previous_platform != Some((platform.ptr_value(), leader)) {
            if let Some(component) = object_component(&platform) {
                if let Some(program) = component
                    .inner::<Pal4ObjectComponent>()
                    .platform_script
                    .as_ref()
                {
                    program.borrow_mut().board();
                }
            }
        }
        *self.platform_rider.borrow_mut() = Some(PlatformRide {
            platform,
            leader,
            position,
            yaw,
            rider_position: point,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radiance::{math::Transform, scene::CoreEntity};

    #[test]
    fn platform_carries_only_the_boarded_leader_and_transports_yaw_with_translation() {
        let scene = Pal4Scene::new_empty();
        let platform = CoreEntity::create("platform".into(), true);
        scene.players[0]
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(2., 3., 0.));
        *scene.platform_rider.borrow_mut() = Some(PlatformRide {
            platform: platform.clone(),
            leader: 0,
            position: Vec3::new(0., 0., 0.),
            yaw: 0.,
            rider_position: Vec3::new(2., 3., 0.),
        });
        platform
            .transform()
            .borrow_mut()
            .rotate_axis_angle_local(&Vec3::UP, std::f32::consts::FRAC_PI_2);
        scene.carry_platform_rider();
        let position = scene.players[0].transform().borrow().position();
        assert_eq!([position.x, position.y, position.z], [2., 3., 0.]);
        platform
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(10., 5., 20.));
        scene.carry_platform_rider();
        let position = scene.players[0].transform().borrow().position();
        assert!((position.x - 10.).abs() < 0.0001);
        assert_eq!(position.y, 8.);
        assert!((position.z - 18.).abs() < 0.0001);
        for player in &scene.players[1..] {
            assert_eq!(
                player.transform().borrow().matrix().floats(),
                Transform::new().matrix().floats()
            );
        }
        scene.clear_platform_rider_for(&platform);
        assert!(scene.platform_rider.borrow().is_none());
        scene.carry_platform_rider();
        let unchanged = scene.players[0].transform().borrow().position();
        assert_eq!(
            [position.x, position.y, position.z],
            [unchanged.x, unchanged.y, unchanged.z]
        );
    }

    #[test]
    fn fixed_pitch_and_roll_do_not_tilt_or_rotate_a_translated_rider() {
        for (pitch, roll) in [(35_f32, 33_f32), (180., 180.)] {
            let scene = Pal4Scene::new_empty();
            let platform = CoreEntity::create("tilted".into(), true);
            platform
                .transform()
                .borrow_mut()
                .rotate_axis_angle_local(&Vec3::BACK, roll.to_radians())
                .rotate_axis_angle_local(&Vec3::UP, 273_f32.to_radians())
                .rotate_axis_angle_local(&Vec3::EAST, pitch.to_radians())
                .set_position(&Vec3::new(100., 200., 300.));
            let player = scene.players[0].transform();
            player
                .borrow_mut()
                .rotate_axis_angle_local(&Vec3::UP, 0.4)
                .set_position(&Vec3::new(140., 208., 260.));
            let mut expected = *player.borrow().matrix();
            *scene.platform_rider.borrow_mut() = Some(PlatformRide {
                platform: platform.clone(),
                leader: 0,
                position: platform.transform().borrow().position(),
                yaw: yaw(&platform),
                rider_position: player.borrow().position(),
            });
            platform
                .transform()
                .borrow_mut()
                .set_position(&Vec3::new(125., 180., 310.));
            scene.carry_platform_rider();
            expected[0][3] += 25.;
            expected[1][3] -= 20.;
            expected[2][3] += 10.;
            assert_eq!(player.borrow().matrix().floats(), expected.floats());
        }
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn floating_selection_is_independent_of_a_nearer_static_floor() {
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        use radiance::rendering::{
            ComponentFactory, MaterialDef, RenderObjectHandle, RenderTarget, RenderingComponent,
            Texture, TextureDef, VertexBuffer, VideoPlayer,
        };
        use std::rc::Rc;
        struct UnrenderedFactory;
        impl ComponentFactory for UnrenderedFactory {
            fn create_texture(&self, _: &TextureDef) -> Box<dyn Texture> {
                unreachable!()
            }
            fn create_imgui_texture(
                &self,
                _: &[u8],
                _: u32,
                _: u32,
                _: u32,
                _: Option<imgui::TextureId>,
            ) -> (Box<dyn Texture>, imgui::TextureId) {
                unreachable!()
            }
            fn remove_imgui_texture(&self, _: Option<imgui::TextureId>) {
                unreachable!()
            }
            fn create_render_object(
                &self,
                _: VertexBuffer,
                _: Vec<u32>,
                _: &MaterialDef,
                _: bool,
            ) -> RenderObjectHandle {
                unreachable!()
            }
            fn create_rendering_component(&self, _: Vec<RenderObjectHandle>) -> RenderingComponent {
                unreachable!()
            }
            fn create_video_player(&self) -> Box<VideoPlayer> {
                unreachable!()
            }
            fn create_render_target(&self, _: u32, _: u32) -> Box<dyn RenderTarget> {
                unreachable!()
            }
        }
        let vfs = packfs::init_virtual_fs(&std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        let gob = GobFile::read(&mut std::io::Cursor::new(
            vfs.read_to_end("/gamedata/scenedata/M15/1/GameObjs.gob")
                .unwrap(),
        ))
        .unwrap();
        let entry = gob
            .entries
            .iter()
            .find(|e| e.name.to_string().unwrap() == "trans12")
            .unwrap();
        let scene = Pal4Scene::new_empty();
        let platform = radiance::debug::create_box_entity(Rc::new(UnrenderedFactory));
        let component = Pal4ObjectComponent::create(
            entry.clone(),
            GobObjectType::FLOAT,
            *platform.transform().borrow().matrix(),
        );
        component.inner::<Pal4ObjectComponent>().platform.set(true);
        platform.add_component(
            crate::openpal4::comdef::IPal4ObjectComponent::uuid(),
            component
                .query_interface::<radiance::comdef::IComponent>()
                .unwrap(),
        );
        let world = scene.scene.collision_world();
        let world = world.inner::<CollisionWorldComponent>();
        assert!(world.attach_dynamic_collider(&platform));
        let origin = Vec3::new(0., 10000., 0.);
        let (_, depth) = world.dynamic_floor_hit(&origin).unwrap();
        let static_floor = radiance::debug::create_box_entity(Rc::new(UnrenderedFactory));
        static_floor
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(0., 4., 0.));
        world.attach_collider(&static_floor);
        assert!(world.cast_aa_ny(&origin).unwrap() < depth);
        scene.players[0]
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(0., 10000. - depth + 8., 0.));
        scene.prepare_platform_rider(0);
        assert_eq!(
            scene
                .platform_rider
                .borrow()
                .as_ref()
                .unwrap()
                .platform
                .ptr_value(),
            platform.ptr_value()
        );
        platform
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(10., 0., 0.));
        scene.carry_platform_rider();
        assert_eq!(scene.players[0].transform().borrow().position().x, 10.);
    }

    #[test]
    fn stationary_rider_survives_a_missed_query_but_a_moved_or_disabled_rider_does_not() {
        let scene = Pal4Scene::new_empty();
        let platform = CoreEntity::create("platform".into(), true);
        let ride = || PlatformRide {
            platform: platform.clone(),
            leader: 0,
            position: Vec3::new(0., 0., 0.),
            yaw: 0.,
            rider_position: Vec3::new(0., 0., 0.),
        };
        *scene.platform_rider.borrow_mut() = Some(ride());
        scene.prepare_platform_rider(0);
        assert!(scene.platform_rider.borrow().is_some());
        platform
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(10., 0., 0.));
        scene.carry_platform_rider();
        scene.prepare_platform_rider(0);
        platform
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(20., 0., 0.));
        scene.carry_platform_rider();
        assert_eq!(scene.players[0].transform().borrow().position().x, 20.);
        scene.prepare_platform_rider(1);
        assert!(scene.platform_rider.borrow().is_none());
        *scene.platform_rider.borrow_mut() = Some(ride());
        scene.prepare_platform_rider(0);
        assert!(scene.platform_rider.borrow().is_none());
        scene.players[0]
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(0., 0., 0.));
        *scene.platform_rider.borrow_mut() = Some(ride());
        platform.set_enabled(false);
        scene.prepare_platform_rider(0);
        assert!(scene.platform_rider.borrow().is_none());
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_linear_platforms_are_complete_and_other_task_families_stay_gated() {
        use super::super::mechanisms::ObjectMotion;
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        let vfs = packfs::init_virtual_fs(&std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        let mut accepted = Vec::new();
        for (scene, block) in [("M14", "3"), ("M15", "1"), ("M15", "2"), ("M17", "2")] {
            let bytes = vfs
                .read_to_end(&format!("/gamedata/scenedata/{scene}/{block}/GameObjs.gob"))
                .unwrap();
            let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            for (index, entry) in gob.entries.iter().enumerate() {
                if gob.header.object_types[index] != GobObjectType::FLOAT {
                    continue;
                }
                let motion = ObjectMotion::from_entry(entry, GobObjectType::FLOAT);
                if motion.is_some() {
                    assert!(supported_platform(entry, GobObjectType::FLOAT, true));
                    accepted.push(format!(
                        "{scene}/{block}/{}",
                        entry.name.to_string().unwrap()
                    ));
                } else if ScriptPlatform::from_entry(entry, GobObjectType::FLOAT).is_none()
                    && entry
                        .parameters
                        .iter()
                        .any(|p| p.name().starts_with(Machine::TASK_LIST_PREFIX))
                {
                    assert!(!supported_platform(entry, GobObjectType::FLOAT, false));
                }
            }
        }
        assert_eq!(
            accepted,
            [
                "M14/3/trans01",
                "M14/3/trans02",
                "M14/3/trans03",
                "M14/3/trans04",
                "M14/3/trans05",
                "M14/3/trans06",
                "M14/3/trans07",
                "M14/3/trans08",
                "M15/1/JPuzzleA01",
                "M15/1/JPuzzleA03",
                "M15/1/JPuzzleA05",
                "M15/1/JPuzzleB04",
                "M15/1/JPuzzleB05",
                "M15/1/JPuzzleB06",
                "M15/1/JPuzzleC05",
                "M15/1/JPuzzleD_1",
                "M15/1/JPuzzleD_2",
                "M15/1/JPuzzleD_9",
                "M15/1/JPuzzleE_2",
                "M15/1/JPuzzleE_3",
                "M15/1/JPuzzleE_5",
                "M15/1/JPuzzleE_8",
                "M15/1/trans01",
                "M15/1/trans02",
                "M15/1/trans03",
                "M15/1/trans04",
                "M15/1/trans05",
                "M15/1/trans06",
                "M15/1/trans07",
                "M15/1/trans09",
                "M15/1/trans10",
                "M15/1/trans11",
                "M15/1/trans12",
                "M15/2/JPuzzleF_1",
                "M15/2/JPuzzleF_2",
                "M15/2/JPuzzleF_5",
                "M15/2/JPuzzleF_6",
                "M15/2/JPuzzleG_2",
                "M15/2/JPuzzleG_4",
                "M15/2/JPuzzleG_7",
                "M15/2/JPuzzleG_8",
                "M15/2/JPuzzleH_1",
                "M15/2/JPuzzleH_11",
                "M15/2/JPuzzleH_3",
                "M15/2/JPuzzleH_8",
                "M15/2/JPuzzleI_13",
                "M15/2/JPuzzleI_14",
                "M15/2/JPuzzleI_4",
                "M15/2/JPuzzleI_9",
                "M15/2/trans01",
                "M15/2/trans02",
                "M15/2/trans03",
                "M15/2/trans04",
                "M15/2/trans05",
                "M15/2/trans06",
                "M15/2/trans07",
                "M15/2/trans11",
                "M15/2/trans12",
                "M15/2/trans13",
                "M15/2/trans14",
                "M15/2/trans15",
                "M15/2/trans16",
                "M15/2/trans17",
                "M15/2/trans18",
                "M15/2/trans20",
                "M15/2/trans21",
                "M15/2/trans34",
                "M17/2/trans01",
                "M17/2/trans02",
            ]
        );
        let bytes = vfs
            .read_to_end("/gamedata/scenedata/M08/2/GameObjs.gob")
            .unwrap();
        let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
        for (name, supported) in [("floor02", true), ("floor01", true)] {
            let index = gob
                .entries
                .iter()
                .position(|e| e.name.to_string().unwrap() == name)
                .unwrap();
            assert_eq!(
                supported_platform(&gob.entries[index], gob.header.object_types[index], false),
                supported
            );
        }
    }

    #[test]
    fn boarding_program_waits_strictly_deduplicates_board_edges_and_settles() {
        let mut program = ScriptPlatform {
            function: "board".into(),
            wait: true,
            delay: 0.2,
            remaining: 2,
            phase: ScriptPlatformPhase::Idle,
        };
        program.board();
        program.board();
        assert_eq!(program.remaining, 1);
        assert!(program.advance(0.2, false).is_none());
        assert_eq!(program.advance(0.1, false).as_deref(), Some("board"));
        program.board();
        assert_eq!(program.remaining, 1);
        assert!(program.advance(1., true).is_none());
        assert!(matches!(program.phase, ScriptPlatformPhase::Running));
        program.advance(0.1, false);
        assert!(matches!(program.phase, ScriptPlatformPhase::Settling));
        program.advance(0.1, false);
        program.board();
        assert_eq!(program.remaining, 0);
        program.reset();
        program.board();
        assert!(matches!(program.phase, ScriptPlatformPhase::Idle));

        program.remaining = -1;
        program.wait = false;
        program.board();
        assert_eq!(program.advance(0.3, true).as_deref(), Some("board"));
        program.advance(0.1, true);
        assert!(matches!(program.phase, ScriptPlatformPhase::Idle));
        program.board();
        assert!(matches!(program.phase, ScriptPlatformPhase::Waiting(_)));
        program.reset();
        assert!(program.advance(1., false).is_none());
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_boarding_platforms_have_complete_programs_and_script_entries() {
        use crate::scripting::angelscript::ScriptModule;
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        let vfs = packfs::init_virtual_fs(&std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        let mut accepted = 0;
        let mut waited = 0;
        let mut start_relative = 0;
        for (scene, blocks) in [("M08", vec!["2", "3", "4"]), ("M15", vec!["2"])] {
            let script = ScriptModule::read_from_buffer(
                &vfs.read_to_end(&format!("/gamedata/script/{scene}.csb"))
                    .unwrap(),
            )
            .unwrap();
            for block in blocks {
                let bytes = vfs
                    .read_to_end(&format!("/gamedata/scenedata/{scene}/{block}/GameObjs.gob"))
                    .unwrap();
                let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
                for (index, entry) in gob.entries.iter().enumerate() {
                    if let Some(program) =
                        ScriptPlatform::from_entry(entry, gob.header.object_types[index])
                    {
                        assert!(supported_platform(entry, GobObjectType::FLOAT, false));
                        assert!(script.functions.iter().any(|f| f.name == program.function));
                        accepted += 1;
                        waited += usize::from(program.wait);
                        for property in &entry.parameters {
                            if let GobProperty::GobPropertyObjectArray(array) = property {
                                if property.name().starts_with(Machine::TASK_LIST_PREFIX) {
                                    start_relative += usize::from(
                                        array.0[0]
                                            .get_property(Task::WAIT_MODE)
                                            .and_then(GobProperty::value_i32)
                                            == Some(2),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            let context = super::super::scripting::create_context();
            for name in if scene == "M08" {
                vec!["func7004"]
            } else {
                vec!["func7022"]
            } {
                let function = script.functions.iter().find(|f| f.name == name).unwrap();
                let calls: Vec<_> = function
                    .inst2
                    .iter()
                    .filter(|i| i.inst == 97)
                    .map(|i| {
                        let operand = i32::from_le_bytes(i.params[..4].try_into().unwrap());
                        context.functions()[(-operand - 1) as usize].name.as_str()
                    })
                    .collect();
                if scene == "M08" {
                    assert!(calls.contains(&"giScriptClearCTXButCurrent"));
                    assert!(calls.contains(&"giPlayerCurrentSetPos"));
                    assert!(calls.contains(&"giFlashOutWhite"));
                } else {
                    for call in function.inst2.iter().filter(|i| i.inst == 12) {
                        let index =
                            u32::from_le_bytes(call.params[..4].try_into().unwrap()) as usize;
                        let helper = &script.functions[index];
                        assert_eq!(helper.param_types.len(), 10);
                        assert_eq!(helper.inst2.last().unwrap().params, 10u16.to_le_bytes());
                        let helper_calls: Vec<_> = helper
                            .inst2
                            .iter()
                            .filter(|i| i.inst == 97)
                            .map(|i| {
                                let operand = i32::from_le_bytes(i.params[..4].try_into().unwrap());
                                context.functions()[(-operand - 1) as usize].name.as_str()
                            })
                            .collect();
                        assert_eq!(helper.name, "func7021");
                        assert!(helper_calls.contains(&"giGetVisibleObject"));
                        assert!(helper_calls.contains(&"giSetObjectVisible"));
                    }
                }
            }
        }
        assert_eq!((accepted, waited, start_relative), (127, 116, 3));
        let bytes = vfs
            .read_to_end("/gamedata/scenedata/M14/3/GameObjs.gob")
            .unwrap();
        let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
        let entry = gob
            .entries
            .iter()
            .find(|e| e.name.to_string().unwrap() == "ctrl_flat01")
            .unwrap();
        assert!(ScriptPlatform::from_entry(entry, GobObjectType::FLOAT).is_none());
        assert!(!supported_platform(entry, GobObjectType::FLOAT, false));
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_visibility_puzzle_callbacks_execute_their_helper_with_ten_arguments() {
        use crate::scripting::angelscript::{
            GlobalFunctionState, ScriptGlobalContext, ScriptGlobalFunction, ScriptModule,
        };
        use common::store_ext::StoreExt2;
        use std::{cell::RefCell, collections::HashMap, rc::Rc};
        let vfs = packfs::init_virtual_fs(&std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        let module = Rc::new(RefCell::new(
            ScriptModule::read_from_buffer(&vfs.read_to_end("/gamedata/script/M15.csb").unwrap())
                .unwrap(),
        ));
        let native = super::super::scripting::create_context();
        let mut globals = ScriptGlobalContext::<HashMap<String, bool>>::new();
        for function in native.functions().iter().skip(globals.functions().len()) {
            globals.register_function(ScriptGlobalFunction::new(
                &function.name,
                Box::new(|name, vm: &mut ScriptVm<HashMap<String, bool>>| {
                    match name {
                        "giIMMBegin" => vm.imm = true,
                        "giIMMEnd" => {
                            vm.imm = false;
                            vm.vm_context_mut().insert("completed".into(), true);
                        }
                        "giGetVisibleObject" | "giSetObjectVisible" => {
                            let value = vm.stack_pop::<u32>();
                            let index = vm.resolve_object_index(value).unwrap();
                            let key = vm.get_object(index).unwrap().clone();
                            if name == "giGetVisibleObject" {
                                let visible = vm.vm_context().get(&key).copied().unwrap_or(true);
                                vm.set_ret_value(i32::from(visible));
                            } else {
                                let visible = vm.stack_pop::<i32>() != 0;
                                vm.vm_context_mut().insert(key, visible);
                            }
                        }
                        "gi2DSoundPlay" => {
                            vm.stack_pop::<u32>();
                            vm.stack_pop::<i32>();
                            vm.set_ret_value(0i32);
                        }
                        _ => panic!("unexpected original puzzle syscall {name}"),
                    }
                    GlobalFunctionState::Completed
                }),
            ));
        }
        let globals = Rc::new(RefCell::new(globals));
        for index in 7022..=7033 {
            let mut vm = ScriptVm::new_idle(globals.clone(), HashMap::new());
            assert!(vm.start_task_script(module.clone(), &format!("func{index}")));
            for _ in 0..300 {
                vm.execute_task_scripts(0.1);
                if vm.is_idle() {
                    break;
                }
            }
            assert!(vm.is_idle(), "func{index} did not settle");
            assert_eq!(
                vm.vm_context().get("completed"),
                Some(&true),
                "func{index} aborted before its helper completed"
            );
            assert!(
                vm.vm_context().keys().any(|name| name.starts_with("trans")),
                "func{index} did not change a puzzle surface"
            );
        }
    }
}
impl Pal4Scene {
    pub(crate) fn carry_platform_rider(&self) {
        let mut ride = self.platform_rider.borrow_mut();
        let Some(ride) = ride.as_mut() else { return };
        if !ride.platform.enabled() || !ride.platform.visible() {
            return;
        }
        let position = ride.platform.transform().borrow().position();
        let delta = Vec3::sub(&position, &ride.position);
        // Native floating update transports yaw only when translation also changed.
        if delta.x == 0. && delta.y == 0. && delta.z == 0. {
            return;
        }
        let angle = yaw(&ride.platform) - ride.yaw;
        let player = self.players[ride.leader].transform();
        let mut player = player.borrow_mut();
        let moved = Vec3::add(&player.position(), &delta);
        let offset = Vec3::sub(&moved, &position);
        let (sin, cos) = angle.sin_cos();
        let moved = Vec3::new(
            position.x + cos * offset.x + sin * offset.z,
            moved.y,
            position.z - sin * offset.x + cos * offset.z,
        );
        player
            .rotate_axis_angle(&Vec3::UP, angle)
            .set_position(&moved);
        ride.rider_position = moved;
    }
}
