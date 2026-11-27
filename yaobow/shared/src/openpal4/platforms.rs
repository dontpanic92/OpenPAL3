use crosscom::ComRc;
use fileformats::pal4::gob::{
    GobEntry, GobFixedProperty as Fixed, GobFloatProperty as Float, GobMachineProperty as Machine,
    GobObjectType, GobParameterBlock, GobProperty,
};
use radiance::{
    comdef::{IEntity, IEntityExt},
    components::collision::CollisionWorldComponent,
    math::Vec3,
    scene::ISceneExt,
};

use super::{
    object_component::Pal4ObjectComponent,
    scene::{Pal4Scene, object_component},
};

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
        && integer(Fixed::STORE, 0) == 0
        && integer(Fixed::CLIP, 0) == 0
        && integer(Fixed::AUTO_DISAPPEAR, 0) == 0
        && integer(Fixed::MARKER, 0) == 0
        && integer(Fixed::HIDE, 0) == 0
        && integer(Fixed::VALIDFOR, -1) == -1
        && entry.rotation[0] == 0.
        && entry.rotation[2] == 0.
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
            || (!entry
                .parameters
                .iter()
                .any(|property| property.name().starts_with(Machine::TASK_LIST_PREFIX))
                && entry
                    .get_property(Fixed::RESEARCH_NUM)
                    .and_then(GobProperty::value_i32)
                    .unwrap_or(-1)
                    == -1))
}

pub(crate) struct PlatformRide {
    platform: ComRc<IEntity>,
    leader: usize,
    position: Vec3,
    yaw: f32,
    rider_position: Vec3,
}

fn yaw(entity: &ComRc<IEntity>) -> f32 {
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
        let Some((platform, depth)) = world.dynamic_floor_hit(&origin) else {
            retain_stationary();
            return;
        };
        if (BOARD_HEIGHT - depth).abs() > BOARD_HEIGHT {
            retain_stationary();
            return;
        }
        if world
            .cast_aa_ny(&origin)
            .is_some_and(|nearest| depth > nearest + 0.001)
            || !platform.visible()
            || !object_component(&platform)
                .is_some_and(|component| component.inner::<Pal4ObjectComponent>().platform.get())
        {
            return;
        }

        let position = platform.transform().borrow().position();
        let yaw = yaw(&platform);
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
        for (scene, block) in [("M15", "1"), ("M15", "2"), ("M17", "2")] {
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
                } else if entry
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
                "M15/1/JPuzzleE_2",
                "M15/1/JPuzzleE_3",
                "M15/2/JPuzzleG_4",
                "M15/2/JPuzzleG_8",
                "M15/2/JPuzzleI_4",
                "M15/2/trans05",
                "M15/2/trans06",
                "M15/2/trans13",
                "M17/2/trans01",
                "M17/2/trans02",
            ]
        );
        let bytes = vfs
            .read_to_end("/gamedata/scenedata/M08/2/GameObjs.gob")
            .unwrap();
        let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
        for (name, supported) in [("floor02", true), ("floor01", false)] {
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
