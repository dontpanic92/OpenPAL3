use crosscom::ComRc;
use fileformats::pal4::gob::{
    GobEntry, GobFixedProperty as Fixed, GobFloatProperty as Float, GobGomTaskProperty as Task,
    GobMachineProperty as Machine, GobObject, GobObjectType, GobProperty,
};
use radiance::{
    comdef::{IEntity, IEntityExt},
    math::{Mat44, Transform, Vec3},
};
use serde::{Deserialize, Serialize};

use super::mechanisms::{
    MotionTaskClock, MotionTaskEvent, TaskPhase, captured_rotation, shortest_yaw_delta,
};

struct Leg {
    target: Vec3,
    duration: f32,
    wait: f32,
    yaw: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use radiance::scene::CoreEntity;

    #[test]
    fn sequential_clock_preserves_strict_predecessor_delays_without_extra_leg_ticks() {
        let mut clocks = [MotionTaskClock::new(); 4];
        let durations = [None, Some(0.2), Some(0.3), None];
        let mut starts = Vec::new();
        for tick in 0..15 {
            for index in 0..clocks.len() {
                if clocks[index].phase == TaskPhase::Finished {
                    continue;
                }
                let previous = index.checked_sub(1).map(|i| clocks[i]);
                if matches!(
                    clocks[index].advance(index, previous, 0.1, 0., durations[index]),
                    Some(MotionTaskEvent::Started(_))
                ) {
                    starts.push((tick, index));
                }
            }
        }
        assert_eq!(starts, [(0, 0), (2, 1), (5, 2), (9, 3)]);
        assert!(clocks.iter().all(|c| c.phase == TaskPhase::Finished));
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_sequential_platforms_reach_every_waypoint_reverse_and_restore_each_cursor() {
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        let vfs = packfs::init_virtual_fs(&std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        for (scene, block, name) in [
            ("M14", "3", "trans02"),
            ("M14", "3", "trans08"),
            ("M15", "1", "trans07"),
            ("M15", "1", "trans12"),
            ("M15", "2", "trans15"),
            ("M15", "2", "trans34"),
            ("M15", "1", "trans01"),
            ("M15", "1", "trans05"),
            ("M15", "1", "trans09"),
            ("M15", "1", "trans11"),
            ("M15", "2", "trans14"),
            ("M15", "2", "trans16"),
            ("M15", "2", "trans20"),
        ] {
            let gob = GobFile::read(&mut std::io::Cursor::new(
                vfs.read_to_end(&format!("/gamedata/scenedata/{scene}/{block}/GameObjs.gob"))
                    .unwrap(),
            ))
            .unwrap();
            let entry = gob
                .entries
                .iter()
                .find(|e| e.name.to_string().unwrap() == name)
                .unwrap();
            let fresh = || SequentialMotion::from_entry(entry, GobObjectType::FLOAT).unwrap();
            let mut original = fresh();
            let entity = CoreEntity::create(name.into(), true);
            entity
                .transform()
                .borrow_mut()
                .set_matrix(original.initial_transform);
            let mut reached = vec![false; original.legs.len()];
            let mut returned = vec![false; original.legs.len()];
            for _ in 0..700 {
                let saved: SequentialProgress = serde_json::from_str(
                    &serde_json::to_string(&original.snapshot(&entity)).unwrap(),
                )
                .unwrap();
                let reloaded = CoreEntity::create("reloaded".into(), true);
                let mut restored = fresh();
                assert!(restored.restore(&reloaded, saved));
                original.advance(&entity, 0.1);
                restored.advance(&reloaded, 0.1);
                assert_eq!(
                    entity.transform().borrow().matrix().floats(),
                    reloaded.transform().borrow().matrix().floats(),
                    "{scene}/{name}"
                );
                assert_eq!(
                    serde_json::to_value(original.snapshot(&entity)).unwrap(),
                    serde_json::to_value(restored.snapshot(&reloaded)).unwrap()
                );
                for (index, clock) in original.progress.clocks.iter().enumerate() {
                    if clock.phase != TaskPhase::Finished {
                        continue;
                    }
                    let Some(i) = original.leg_index(index) else {
                        continue;
                    };
                    let flags = if original.progress.phase == Phase::Outbound {
                        &mut reached
                    } else {
                        &mut returned
                    };
                    if flags[i] {
                        continue;
                    }
                    flags[i] = true;
                    let target = if original.progress.phase == Phase::Outbound {
                        original.legs[i].target
                    } else {
                        original.progress.outbound[i].as_ref().unwrap().position
                    };
                    let actual = entity.transform().borrow().position();
                    assert!(
                        (actual.x - target.x).abs() < 0.001
                            && (actual.y - target.y).abs() < 0.001
                            && (actual.z - target.z).abs() < 0.001
                    );
                    let previous_yaw = original.legs[..i]
                        .iter()
                        .rev()
                        .find_map(|leg| leg.yaw)
                        .unwrap_or(original.reference.y);
                    let expected_yaw = if original.progress.phase == Phase::Outbound {
                        original.legs[i].yaw.unwrap_or(previous_yaw)
                    } else {
                        previous_yaw
                    };
                    assert!(
                        shortest_yaw_delta(original.yaw(&entity), expected_yaw).abs() < 0.0001,
                        "{scene}/{name} leg {i} {:?} yaw",
                        original.progress.phase
                    );
                }
            }
            assert!(
                reached.iter().chain(&returned).all(|v| *v),
                "{name} waypoints"
            );
            assert!(!original.running());
            for (a, b) in entity
                .transform()
                .borrow()
                .matrix()
                .floats()
                .iter()
                .flatten()
                .zip(original.initial_transform.floats().iter().flatten())
            {
                assert!((a - b).abs() < 0.0001);
            }
            original.cancel();
            let mut restored = fresh();
            assert!(restored.restore(&entity, original.snapshot(&entity)));
            restored.advance(&entity, 100.);
            assert!(!restored.running());

            let mut original = fresh();
            original.start();
            for _ in 0..20 {
                original.advance(&entity, 0.1);
            }
            let valid = original.snapshot(&entity);
            for case in 0..4 {
                let mut saved = valid.clone();
                match case {
                    0 => {
                        saved.outbound.pop();
                    }
                    1 => saved.clocks[1].elapsed = f32::NAN,
                    2 => {
                        saved.outbound[0] = None;
                        saved.clocks[1].phase = TaskPhase::Running;
                    }
                    _ => saved.automatic_pending = true,
                }
                let reloaded = CoreEntity::create("invalid".into(), true);
                let before = *reloaded.transform().borrow().matrix();
                let mut restored = fresh();
                assert!(!restored.restore(&reloaded, saved), "{name} invalid {case}");
                assert_eq!(
                    reloaded.transform().borrow().matrix().floats(),
                    before.floats()
                );
                assert!(!restored.running());
            }
            for case in 0..3 {
                let mut unsupported = entry.clone();
                let task = unsupported
                    .parameters
                    .iter_mut()
                    .find_map(|property| {
                        if !property.name().starts_with(Machine::TASK_LIST_PREFIX) {
                            return None;
                        }
                        let GobProperty::GobPropertyObjectArray(array) = property else {
                            return None;
                        };
                        array.0.iter_mut().find(|task| {
                            matches!(
                                task.get_property(Task::TYPE)
                                    .and_then(GobProperty::value_i32),
                                Some(0 | 10)
                            )
                        })
                    })
                    .unwrap();
                let combined = task
                    .get_property(Task::TYPE)
                    .and_then(GobProperty::value_i32)
                    == Some(10);
                let property = task
                    .properties
                    .iter_mut()
                    .find(|p| {
                        p.name()
                            == match case {
                                0 => Task::WAIT_MODE,
                                1 => Task::TYPE,
                                _ if combined => Task::MOVMENT_SOUND_NAME,
                                _ => Task::MOVE_SOUND_NAME,
                            }
                    })
                    .unwrap();
                match property {
                    GobProperty::GobPropertyI32(value) => value.value = 2,
                    GobProperty::GobPropertyString(value) => value.value = "unsupported.wav".into(),
                    _ => unreachable!(),
                }
                assert!(
                    SequentialMotion::from_entry(&unsupported, GobObjectType::FLOAT).is_none(),
                    "{name} unsupported leg {case}"
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Phase {
    Idle,
    Outbound,
    Returning,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Capture {
    position: Vec3,
    rotation: Option<Vec3>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct SequentialProgress {
    phase: Phase,
    clocks: Vec<MotionTaskClock>,
    outbound: Vec<Option<Capture>>,
    returning: Vec<Option<Capture>>,
    automatic_pending: bool,
    pose: Mat44,
}

pub(super) struct SequentialMotion {
    pub initial_transform: Mat44,
    pub stored: bool,
    reference: Vec3,
    legs: Vec<Leg>,
    start_wait: f32,
    rollback_wait: f32,
    repeat: bool,
    progress: SequentialProgress,
}

impl SequentialMotion {
    pub fn from_entry(entry: &GobEntry, kind: u32) -> Option<Self> {
        let integer = |name, default| {
            entry
                .get_property(name)
                .or_else(|| entry.get_parameter(name))
                .and_then(GobProperty::value_i32)
                .unwrap_or(default)
        };
        if kind != GobObjectType::FLOAT
            || !super::platforms::supported_metadata(entry)
            || integer(Float::TOUCH_TYPE, 1) != 1
            || integer(Float::AUTO_TOUCH, 0) != 1
            || !matches!(integer(Machine::REPEAT, 0), 0 | 1)
        {
            return None;
        }
        let tasks: Vec<_> = entry
            .parameters
            .iter()
            .filter_map(|p| match p {
                GobProperty::GobPropertyObjectArray(array)
                    if p.name().starts_with(Machine::TASK_LIST_PREFIX) =>
                {
                    Some(&array.0)
                }
                _ => None,
            })
            .flatten()
            .collect();
        if tasks.len() < 4 {
            return None;
        }
        let scalar = |task: &GobObject, name| {
            task.get_property(name)?
                .value_f32()
                .filter(|v| v.is_finite())
        };
        let ty = |task: &GobObject| task.get_property(Task::TYPE)?.value_i32();
        let start = tasks[0];
        let rollback = tasks[tasks.len() - 1];
        if ty(start) != Some(16)
            || ty(rollback) != Some(6)
            || scalar(start, Task::EXPT_TIME) != Some(0.)
            || scalar(rollback, Task::EXPT_TIME) != Some(0.)
            || tasks.iter().any(|t| {
                t.get_property(Task::WAIT_MODE)
                    .and_then(GobProperty::value_i32)
                    != Some(1)
                    || !scalar(t, Task::WAIT_TIME).is_some_and(|v| v >= 0.)
            })
        {
            return None;
        }
        let start_scalar = |name| match start.get_property(name) {
            None => Some(0.),
            _ => scalar(start, name),
        };
        let reference = Vec3::new(
            start_scalar(Task::START_ROT_X)?.to_radians(),
            start_scalar(Task::START_ROT_Y)?.to_radians(),
            start_scalar(Task::START_ROT_Z)?.to_radians(),
        );
        let scale = entry
            .get_property(Fixed::SCALE)
            .and_then(GobProperty::value_f32)
            .unwrap_or(1.);
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        let mut initial = Transform::new();
        initial
            .rotate_axis_angle_local(&Vec3::BACK, reference.z)
            .rotate_axis_angle_local(&Vec3::UP, reference.y)
            .rotate_axis_angle_local(&Vec3::EAST, reference.x)
            .scale_local(&Vec3::new(scale, scale, scale))
            .set_position(&Vec3::new(
                start_scalar(Task::START_POS_X)?,
                start_scalar(Task::START_POS_Y)?,
                start_scalar(Task::START_POS_Z)?,
            ));
        let mut legs = Vec::new();
        for task in &tasks[1..tasks.len() - 1] {
            let combined = ty(task)? == 10;
            if !matches!(ty(task), Some(0 | 10)) {
                return None;
            }
            let (positions, effect, sound) = if combined {
                (
                    [
                        Task::MOVMENT_POS_X,
                        Task::MOVMENT_POS_Y,
                        Task::MOVMENT_POS_Z,
                    ],
                    Task::MOVMENT_EFFECT_NAME,
                    Task::MOVMENT_SOUND_NAME,
                )
            } else {
                (
                    [
                        Task::MOVE_TARGET_X,
                        Task::MOVE_TARGET_Y,
                        Task::MOVE_TARGET_Z,
                    ],
                    Task::MOVE_EFFECT_NAME,
                    Task::MOVE_SOUND_NAME,
                )
            };
            if [effect, sound]
                .iter()
                .any(|name| task.get_property(name).and_then(GobProperty::value_string) != Some(""))
            {
                return None;
            }
            let yaw = if combined {
                scalar(task, Task::MOVMENT_ROT_X)?;
                scalar(task, Task::MOVMENT_ROT_Z)?;
                Some(
                    scalar(task, Task::MOVMENT_ROT_Y)
                        .filter(|v| (0. ..=360.).contains(v))?
                        .to_radians(),
                )
            } else {
                None
            };
            legs.push(Leg {
                target: Vec3::new(
                    scalar(task, positions[0])?,
                    scalar(task, positions[1])?,
                    scalar(task, positions[2])?,
                ),
                duration: scalar(task, Task::EXPT_TIME).filter(|v| *v > 0.)?,
                wait: scalar(task, Task::WAIT_TIME)?,
                yaw,
            });
        }
        Some(Self {
            initial_transform: *initial.matrix(),
            stored: integer(Fixed::STORE, 0) != 0,
            reference,
            start_wait: scalar(start, Task::WAIT_TIME)?,
            rollback_wait: scalar(rollback, Task::WAIT_TIME)?,
            repeat: integer(Machine::REPEAT, 0) != 0,
            progress: SequentialProgress {
                phase: Phase::Idle,
                clocks: Vec::new(),
                outbound: vec![None; legs.len()],
                returning: vec![None; legs.len()],
                automatic_pending: true,
                pose: *initial.matrix(),
            },
            legs,
        })
    }

    pub fn running(&self) -> bool {
        self.progress.phase != Phase::Idle
    }

    pub fn start(&mut self) {
        self.progress.phase = Phase::Outbound;
        self.progress.clocks = vec![MotionTaskClock::new(); self.legs.len() + 2];
        self.progress.outbound.fill(None);
        self.progress.returning.fill(None);
        self.progress.automatic_pending = false;
    }

    pub fn cancel(&mut self) {
        self.progress.phase = Phase::Idle;
        self.progress.clocks.clear();
        self.progress.outbound.fill(None);
        self.progress.returning.fill(None);
        self.progress.automatic_pending = false;
    }

    fn leg_index(&self, index: usize) -> Option<usize> {
        if self.progress.phase == Phase::Returning {
            (index < self.legs.len()).then(|| self.legs.len() - index - 1)
        } else {
            (index > 0 && index <= self.legs.len()).then(|| index - 1)
        }
    }

    fn wait(&self, index: usize) -> f32 {
        self.leg_index(index)
            .map(|i| self.legs[i].wait)
            .unwrap_or_else(|| {
                if self.progress.phase == Phase::Returning {
                    if index == self.legs.len() {
                        self.start_wait
                    } else {
                        0.
                    }
                } else if index == 0 {
                    self.start_wait
                } else {
                    self.rollback_wait
                }
            })
    }

    pub fn advance(&mut self, entity: &ComRc<IEntity>, delta: f32) {
        if self.progress.automatic_pending {
            self.start();
            return;
        }
        if !self.running() {
            return;
        }
        if self
            .progress
            .clocks
            .iter()
            .all(|t| t.phase == TaskPhase::Finished)
        {
            if self.progress.phase == Phase::Outbound {
                self.progress.phase = Phase::Returning;
                self.progress.clocks.fill(MotionTaskClock::new());
            } else {
                self.progress.phase = Phase::Idle;
                self.progress.clocks.clear();
                self.progress.automatic_pending = self.repeat;
            }
            return;
        }
        for index in 0..self.progress.clocks.len() {
            if self.progress.clocks[index].phase == TaskPhase::Finished {
                continue;
            }
            let previous = index.checked_sub(1).map(|i| self.progress.clocks[i]);
            let leg = self.leg_index(index);
            let duration = leg.map(|i| self.legs[i].duration);
            let wait = self.wait(index);
            let event = self.progress.clocks[index].advance(index, previous, delta, wait, duration);
            match event {
                Some(MotionTaskEvent::Started(_)) => {
                    if let Some(i) = leg {
                        let capture = Capture {
                            position: entity.transform().borrow().position(),
                            rotation: self.legs[i].yaw.map(|_| {
                                captured_rotation(
                                    entity.transform().borrow().matrix(),
                                    &self.reference,
                                )
                            }),
                        };
                        if self.progress.phase == Phase::Returning {
                            self.progress.returning[i] = Some(capture);
                        } else {
                            self.progress.outbound[i] = Some(capture);
                        }
                    } else if index == 0 || index == self.legs.len() {
                        self.apply_initial(entity);
                    }
                }
                Some(MotionTaskEvent::Moved(elapsed)) => {
                    self.apply_leg(entity, leg.unwrap(), elapsed)
                }
                None => {}
            }
        }
    }

    fn apply_initial(&self, entity: &ComRc<IEntity>) {
        let transform = entity.transform();
        let mut transform = transform.borrow_mut();
        let mut pose = self.initial_transform;
        for column in 0..3 {
            let length = |m: &Mat44| (0..3).map(|r| m[r][column].powi(2)).sum::<f32>().sqrt();
            let ratio = length(transform.matrix()) / length(&pose);
            for row in 0..3 {
                pose[row][column] *= ratio;
            }
        }
        transform.set_matrix(pose);
    }

    fn apply_leg(&self, entity: &ComRc<IEntity>, i: usize, elapsed: f32) {
        let returning = self.progress.phase == Phase::Returning;
        let capture = if returning {
            &self.progress.returning[i]
        } else {
            &self.progress.outbound[i]
        }
        .as_ref()
        .unwrap();
        let amount = elapsed / self.legs[i].duration;
        let target = if returning {
            self.progress.outbound[i].as_ref().unwrap().position
        } else {
            self.legs[i].target
        };
        let transform = entity.transform();
        let mut transform = transform.borrow_mut();
        if let Some(start) = &capture.rotation {
            let yaw = if returning {
                self.progress.outbound[i]
                    .as_ref()
                    .unwrap()
                    .rotation
                    .as_ref()
                    .unwrap()
                    .y
            } else {
                self.legs[i].yaw.unwrap()
            };
            let scale = |column: usize| {
                (0..3)
                    .map(|r| transform.matrix()[r][column].powi(2))
                    .sum::<f32>()
                    .sqrt()
            };
            let mut pose = Transform::new();
            pose.rotate_axis_angle_local(&Vec3::BACK, start.z)
                .rotate_axis_angle_local(
                    &Vec3::UP,
                    start.y + shortest_yaw_delta(start.y, yaw) * amount,
                )
                .rotate_axis_angle_local(&Vec3::EAST, start.x)
                .scale_local(&Vec3::new(scale(0), scale(1), scale(2)));
            transform.set_matrix(*pose.matrix());
        }
        transform.set_position(&Vec3::lerp(&capture.position, &target, amount));
    }

    pub fn yaw(&self, entity: &ComRc<IEntity>) -> f32 {
        captured_rotation(entity.transform().borrow().matrix(), &self.reference).y
    }

    pub fn snapshot(&self, entity: &ComRc<IEntity>) -> SequentialProgress {
        let mut saved = self.progress.clone();
        saved.pose = *entity.transform().borrow().matrix();
        saved
    }

    pub fn restore(&mut self, entity: &ComRc<IEntity>, saved: SequentialProgress) -> bool {
        let n = self.legs.len();
        let finite = |v: &Vec3| [v.x, v.y, v.z].iter().all(|v| v.is_finite());
        if saved.outbound.len() != n
            || saved.returning.len() != n
            || !saved.pose.floats().iter().flatten().all(|v| v.is_finite())
            || saved
                .outbound
                .iter()
                .chain(&saved.returning)
                .enumerate()
                .any(|(i, capture)| {
                    capture.as_ref().is_some_and(|c| {
                        !finite(&c.position)
                            || c.rotation.as_ref().is_some_and(|r| !finite(r))
                            || c.rotation.is_some() != self.legs[i % n].yaw.is_some()
                    })
                })
            || (saved.phase == Phase::Idle && !saved.clocks.is_empty())
            || (saved.phase != Phase::Idle
                && (saved.clocks.len() != n + 2 || saved.automatic_pending))
        {
            return false;
        }
        for (index, clock) in saved.clocks.iter().enumerate() {
            let leg = if saved.phase == Phase::Returning {
                (index < n).then(|| n - index - 1)
            } else {
                (index > 0 && index <= n).then(|| index - 1)
            };
            if ![clock.clock, clock.elapsed, clock.finished_at]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.)
                || clock.finished_at > clock.clock
                || (clock.phase == TaskPhase::Waiting
                    && (clock.elapsed != 0. || clock.finished_at != 0.))
                || (clock.phase == TaskPhase::Running && clock.finished_at != 0.)
                || (index > 0
                    && clock.phase != TaskPhase::Waiting
                    && saved.clocks[index - 1].phase != TaskPhase::Finished)
                || leg.is_some_and(|i| {
                    clock.elapsed > self.legs[i].duration
                        || (clock.phase == TaskPhase::Finished
                            && clock.elapsed != self.legs[i].duration)
                })
                || (leg.is_none() && clock.elapsed != 0.)
                || (clock.phase != TaskPhase::Waiting
                    && leg.is_some_and(|i| {
                        saved.outbound[i].is_none()
                            || (saved.phase == Phase::Returning && saved.returning[i].is_none())
                    }))
            {
                return false;
            }
        }
        if saved.phase == Phase::Returning && saved.outbound.iter().any(Option::is_none) {
            return false;
        }
        entity.transform().borrow_mut().set_matrix(saved.pose);
        self.progress = saved;
        true
    }
}
