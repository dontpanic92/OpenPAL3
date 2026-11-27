use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crosscom::ComRc;
use fileformats::pal4::gob::{
    GobEntry, GobFixedProperty as Fixed, GobGomTaskProperty as Task, GobMachineProperty as Machine,
    GobObjectType, GobProperty,
};
use radiance::{
    audio::{AudioMemorySource, AudioSourceState, Codec},
    comdef::{IArmatureComponentExt, IEntity, IEntityExt, ISceneExt},
    components::mesh::skinned_mesh::AnimationState,
    math::{Mat44, Transform, Vec3},
};
use serde::{Deserialize, Serialize};

use super::{
    object_component::Pal4ObjectComponent,
    scene::{TAG_OBJECT, object_armature, object_component, play_object_animation},
    vm_context::Pal4VmContext,
};

pub(crate) struct ScriptObjectMotion {
    start: Vec3,
    target: Vec3,
    duration: f32,
    elapsed: f32,
    completed: Rc<Cell<bool>>,
}

impl ScriptObjectMotion {
    fn new(entity: &ComRc<IEntity>, duration: f32, target: Vec3) -> Option<Self> {
        if !entity.enabled()
            || !duration.is_finite()
            || duration <= 0.
            || ![target.x, target.y, target.z].iter().all(|v| v.is_finite())
        {
            return None;
        }
        Some(Self {
            start: entity.transform().borrow().position(),
            target,
            duration,
            elapsed: 0.,
            completed: Rc::new(Cell::new(false)),
        })
    }

    fn advance(&mut self, entity: &ComRc<IEntity>, delta_sec: f32) {
        if !entity.enabled() {
            self.completed.set(true);
            return;
        }
        if self.completed.get() || !delta_sec.is_finite() || delta_sec <= 0. {
            return;
        }
        self.elapsed = (self.elapsed + delta_sec).min(self.duration);
        let amount = self.elapsed / self.duration;
        entity.transform().borrow_mut().set_position(&Vec3::new(
            self.start.x * (1. - amount) + self.target.x * amount,
            self.start.y * (1. - amount) + self.target.y * amount,
            self.start.z * (1. - amount) + self.target.z * amount,
        ));
        self.completed.set(self.elapsed == self.duration);
    }
}

impl Drop for ScriptObjectMotion {
    fn drop(&mut self) {
        self.completed.set(true);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct MotionProgress {
    remaining: i32,
    elapsed: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TaskPhase {
    Waiting,
    Running,
    Finished,
}

#[derive(Clone, Copy)]
struct MotionTaskClock {
    phase: TaskPhase,
    clock: f32,
    elapsed: f32,
    finished_at: f32,
}

#[derive(Clone, Copy)]
enum MotionTaskEvent {
    Started(usize),
    Moved(f32),
}

struct MotionTaskGroup<const N: usize = 3> {
    tasks: [MotionTaskClock; N],
    waits: [f32; N],
    movement: usize,
}

impl<const N: usize> MotionTaskGroup<N> {
    fn new(waits: [f32; N], movement: usize) -> Self {
        Self {
            tasks: [MotionTaskClock {
                phase: TaskPhase::Waiting,
                clock: 0.,
                elapsed: 0.,
                finished_at: 0.,
            }; N],
            waits,
            movement,
        }
    }

    fn advance(&mut self, delta_sec: f32, duration: f32) -> ([Option<MotionTaskEvent>; N], bool) {
        let mut events = [None; N];
        let mut finished = true;
        for (index, event) in events.iter_mut().enumerate() {
            if self.tasks[index].phase == TaskPhase::Finished {
                continue;
            }
            finished = false;
            let predecessor = index.checked_sub(1).map(|i| self.tasks[i]);
            let task = &mut self.tasks[index];
            task.clock += delta_sec;
            match task.phase {
                TaskPhase::Waiting => {
                    let ready = predecessor.is_none_or(|previous| {
                        previous.phase == TaskPhase::Finished
                            && task.clock - previous.finished_at > self.waits[index]
                    });
                    if ready && (predecessor.is_some() || task.clock > self.waits[index]) {
                        task.phase = TaskPhase::Running;
                        *event = Some(MotionTaskEvent::Started(index));
                    }
                }
                TaskPhase::Running => {
                    if index == self.movement {
                        task.elapsed = (task.elapsed + delta_sec).min(duration);
                        *event = Some(MotionTaskEvent::Moved(task.elapsed));
                    }
                    if index != self.movement || task.elapsed == duration {
                        task.finished_at = task.clock;
                        task.phase = TaskPhase::Finished;
                    }
                }
                TaskPhase::Finished => unreachable!(),
            }
        }
        // Native groups recognize completion on the tick after all tasks finish.
        (events, finished)
    }
}

enum RollbackPhase {
    Idle,
    Outbound(MotionTaskGroup),
    Returning(MotionTaskGroup),
}

struct RollbackProgram {
    waits: [f32; 3],
    phase: RollbackPhase,
    repeat: bool,
    automatic_pending: bool,
    outbound_start: Option<Vec3>,
    return_start: Option<Vec3>,
}

struct LinearProgram {
    waits: [f32; 2],
    group: Option<MotionTaskGroup<2>>,
    start: Option<Vec3>,
}

pub(crate) struct ObjectMotion {
    pub initial_transform: Mat44,
    target: Vec3,
    duration: f32,
    stored: bool,
    sound: Option<String>,
    sound_source: Option<Box<dyn AudioMemorySource>>,
    rollback: Option<RollbackProgram>,
    linear: Option<LinearProgram>,
    pub progress: MotionProgress,
}

impl ObjectMotion {
    pub fn from_entry(entry: &GobEntry, kind: u32) -> Option<RefCell<Self>> {
        use fileformats::pal4::gob::GobFloatProperty;
        let floating = kind == GobObjectType::FLOAT;
        let integer = |name, default| {
            entry
                .get_property(name)
                .or_else(|| entry.get_parameter(name))
                .and_then(GobProperty::value_i32)
                .unwrap_or(default)
        };
        if (!floating && kind != GobObjectType::MACHINE)
            || !entry.research_function.to_string().ok()?.is_empty()
            || (!floating
                && entry
                    .parameters
                    .iter()
                    .any(|p| p.name().starts_with("PAL4_GAMEOBJECT_float")))
            || (floating && !super::platforms::supported_metadata(entry))
            || integer(Fixed::CLIP, 0) != 0
            || integer(Fixed::AUTO_DISAPPEAR, 0) != 0
            || integer(Fixed::VALIDFOR, -1) != -1
            || integer(Fixed::MARKER, 0) != 0
        {
            return None;
        }
        let mut tasks = Vec::new();
        for property in &entry.parameters {
            if let GobProperty::GobPropertyObjectArray(array) = property {
                if property.name() == Machine::CONDITION {
                    if array.0.iter().any(|condition| {
                        condition
                            .get_property(Machine::PROPID)
                            .and_then(GobProperty::value_i32)
                            .is_some_and(|id| id >= 0)
                    }) {
                        return None;
                    }
                } else if property.name().starts_with(Machine::TASK_LIST_PREFIX) {
                    tasks.extend(&array.0);
                }
            }
        }
        let (start, movement, rollback_task) = match tasks.as_slice() {
            [start, movement] => (*start, *movement, None),
            [start, movement, rollback] => (*start, *movement, Some(*rollback)),
            _ => return None,
        };
        if floating
            && (integer(GobFloatProperty::TOUCH_TYPE, 1) != 1
                || (rollback_task.is_some() && integer(GobFloatProperty::AUTO_TOUCH, 0) != 1))
        {
            return None;
        }
        let automatic = integer(
            if floating {
                GobFloatProperty::AUTO_TOUCH
            } else {
                Machine::AUTO_TOUCH
            },
            0,
        );
        let repeat = integer(Machine::REPEAT, 0);
        if !matches!(automatic, 0 | 1)
            || !matches!(repeat, 0 | 1)
            || (rollback_task.is_none() && (automatic != 0 || repeat != 0))
        {
            return None;
        }
        let scalar = |task: &fileformats::pal4::gob::GobObject, name| {
            task.get_property(name)
                .and_then(GobProperty::value_f32)
                .filter(|value| value.is_finite())
        };
        for task in tasks {
            if task
                .get_property(Task::WAIT_MODE)
                .and_then(GobProperty::value_i32)
                != Some(1)
                || !scalar(task, Task::WAIT_TIME).is_some_and(|time| {
                    if rollback_task.is_some() || floating {
                        time >= 0.
                    } else {
                        time == 0.
                    }
                })
            {
                return None;
            }
        }
        if start
            .get_property(Task::TYPE)
            .and_then(GobProperty::value_i32)
            != Some(16)
            || movement
                .get_property(Task::TYPE)
                .and_then(GobProperty::value_i32)
                != Some(0)
            || scalar(start, Task::EXPT_TIME) != Some(0.)
            || movement
                .get_property(Task::MOVE_EFFECT_NAME)
                .and_then(GobProperty::value_string)
                != Some("")
        {
            return None;
        }
        let sound = movement
            .get_property(Task::MOVE_SOUND_NAME)?
            .value_string()?;
        if floating && !sound.is_empty() {
            return None;
        }
        if !sound.is_empty()
            && movement
                .get_property(Task::MOVE_SOUND_TIME)
                .and_then(GobProperty::value_i32)
                != Some(-1)
        {
            return None;
        }
        let rollback = if let Some(task) = rollback_task {
            if task
                .get_property(Task::TYPE)
                .and_then(GobProperty::value_i32)
                != Some(6)
                || scalar(task, Task::EXPT_TIME) != Some(0.)
                || !sound.is_empty()
                || integer(Fixed::STORE, 0) != 0
            {
                return None;
            }
            Some(RollbackProgram {
                waits: [
                    scalar(start, Task::WAIT_TIME)?,
                    scalar(movement, Task::WAIT_TIME)?,
                    scalar(task, Task::WAIT_TIME)?,
                ],
                phase: RollbackPhase::Idle,
                repeat: repeat != 0,
                automatic_pending: automatic != 0 && entry.active != 0,
                outbound_start: None,
                return_start: None,
            })
        } else {
            None
        };
        let linear = if floating && rollback_task.is_none() {
            Some(LinearProgram {
                waits: [
                    scalar(start, Task::WAIT_TIME)?,
                    scalar(movement, Task::WAIT_TIME)?,
                ],
                group: None,
                start: None,
            })
        } else {
            None
        };
        let duration = scalar(movement, Task::EXPT_TIME).filter(|time| *time > 0.)?;
        let position = Vec3::new(
            scalar(start, Task::START_POS_X)?,
            scalar(start, Task::START_POS_Y)?,
            scalar(start, Task::START_POS_Z)?,
        );
        let rotation = Vec3::new(
            scalar(start, Task::START_ROT_X)?,
            scalar(start, Task::START_ROT_Y)?,
            scalar(start, Task::START_ROT_Z)?,
        );
        let scale = entry
            .get_property(Fixed::SCALE)
            .and_then(GobProperty::value_f32)
            .unwrap_or(1.);
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        let mut transform = Transform::new();
        transform
            .scale_local(&Vec3::new(scale, scale, scale))
            .rotate_axis_angle_local(&Vec3::BACK, rotation.z.to_radians())
            .rotate_axis_angle_local(&Vec3::UP, rotation.y.to_radians())
            .rotate_axis_angle_local(&Vec3::EAST, rotation.x.to_radians())
            .set_position(&position);
        Some(RefCell::new(Self {
            initial_transform: *transform.matrix(),
            target: Vec3::new(
                scalar(movement, Task::MOVE_TARGET_X)?,
                scalar(movement, Task::MOVE_TARGET_Y)?,
                scalar(movement, Task::MOVE_TARGET_Z)?,
            ),
            duration,
            stored: integer(Fixed::STORE, 0) != 0,
            sound: (!sound.is_empty()).then(|| sound.to_owned()),
            sound_source: None,
            rollback,
            linear,
            progress: MotionProgress {
                remaining: if floating {
                    -1
                } else {
                    integer(Fixed::RESEARCH_NUM, -1)
                },
                elapsed: None,
            },
        }))
    }

    fn start(&mut self, entity: &ComRc<IEntity>) -> bool {
        if self.progress.remaining == 0 || self.running() {
            return false;
        }
        if self.progress.remaining > 0 {
            self.progress.remaining -= 1;
        }
        self.progress.elapsed = Some(0.);
        self.stop_sound();
        if let Some(rollback) = self.rollback.as_mut() {
            rollback.automatic_pending = false;
            rollback.outbound_start = None;
            rollback.return_start = None;
            rollback.phase = RollbackPhase::Outbound(MotionTaskGroup::new(rollback.waits, 1));
        } else if let Some(linear) = self.linear.as_mut() {
            linear.group = Some(MotionTaskGroup::new(linear.waits, 1));
            linear.start = None;
        } else {
            self.apply_pose(entity);
        }
        true
    }

    fn advance(&mut self, entity: &ComRc<IEntity>, delta_sec: f32) {
        if !delta_sec.is_finite() || delta_sec <= 0. {
            return;
        }
        if let Some(source) = self.sound_source.as_mut() {
            if !entity.enabled() {
                if source.state() == AudioSourceState::Playing {
                    source.pause();
                }
            } else {
                if source.state() == AudioSourceState::Paused {
                    source.resume();
                }
                source.update();
            }
        }
        if !entity.enabled() {
            return;
        }
        if self.rollback.is_some() {
            self.advance_rollback(entity, delta_sec);
            return;
        }
        if self.linear.is_some() {
            self.advance_linear(entity, delta_sec);
            return;
        }
        if let Some(elapsed) = self.progress.elapsed.as_mut() {
            if *elapsed < self.duration {
                *elapsed = (*elapsed + delta_sec).min(self.duration);
                self.apply_position(entity);
                if !self.running() {
                    self.stop_sound();
                }
            }
        }
    }

    fn advance_linear(&mut self, entity: &ComRc<IEntity>, delta_sec: f32) {
        let linear = self.linear.as_mut().unwrap();
        let Some(group) = linear.group.as_mut() else {
            return;
        };
        let (events, finished) = group.advance(delta_sec, self.duration);
        if finished {
            linear.group = None;
        }
        for event in events.into_iter().flatten() {
            match event {
                MotionTaskEvent::Started(0) => self.apply_authored_pose(entity),
                MotionTaskEvent::Started(1) => {
                    self.linear.as_mut().unwrap().start =
                        Some(entity.transform().borrow().position());
                }
                MotionTaskEvent::Moved(elapsed) => {
                    self.progress.elapsed = Some(elapsed);
                    let start = self.linear.as_ref().unwrap().start.unwrap();
                    entity.transform().borrow_mut().set_position(&Vec3::lerp(
                        &start,
                        &self.target,
                        elapsed / self.duration,
                    ));
                }
                _ => {}
            }
        }
    }

    fn advance_rollback(&mut self, entity: &ComRc<IEntity>, delta_sec: f32) {
        if self.rollback.as_ref().unwrap().automatic_pending {
            self.rollback.as_mut().unwrap().automatic_pending = false;
            self.start(entity);
            return;
        }
        let rollback = self.rollback.as_mut().unwrap();
        let returning = matches!(rollback.phase, RollbackPhase::Returning(_));
        let (events, finished) = match &mut rollback.phase {
            RollbackPhase::Idle => return,
            RollbackPhase::Outbound(group) | RollbackPhase::Returning(group) => {
                group.advance(delta_sec, self.duration)
            }
        };
        if finished {
            if returning {
                rollback.phase = RollbackPhase::Idle;
                rollback.automatic_pending = rollback.repeat;
            } else {
                // Task 6 recursively queues move-back, start-pose and cleanup tasks.
                rollback.phase = RollbackPhase::Returning(MotionTaskGroup::new(
                    [rollback.waits[1], rollback.waits[0], 0.],
                    0,
                ));
            }
        }
        for event in events.into_iter().flatten() {
            match event {
                MotionTaskEvent::Started(index) if index == usize::from(returning) => {
                    self.progress.elapsed = Some(0.);
                    self.apply_authored_pose(entity);
                }
                MotionTaskEvent::Moved(elapsed) => {
                    self.progress.elapsed = Some(if returning {
                        self.duration - elapsed
                    } else {
                        elapsed
                    });
                    let rollback = self.rollback.as_ref().unwrap();
                    let start = if returning {
                        rollback.return_start.unwrap()
                    } else {
                        rollback.outbound_start.unwrap()
                    };
                    let target = if returning {
                        rollback.outbound_start.unwrap()
                    } else {
                        self.target
                    };
                    entity.transform().borrow_mut().set_position(&Vec3::lerp(
                        &start,
                        &target,
                        elapsed / self.duration,
                    ));
                }
                MotionTaskEvent::Started(index) if index == usize::from(!returning) => {
                    let rollback = self.rollback.as_mut().unwrap();
                    let start = entity.transform().borrow().position();
                    if returning {
                        rollback.return_start = Some(start);
                    } else {
                        rollback.outbound_start = Some(start);
                    }
                }
                _ => {}
            }
        }
    }

    fn apply_authored_pose(&self, entity: &ComRc<IEntity>) {
        let transform = entity.transform();
        let mut transform = transform.borrow_mut();
        let mut pose = self.initial_transform;
        // Task 16 sets position/rotation, not the object's current scale.
        for column in 0..3 {
            let length = |matrix: &Mat44| {
                (0..3)
                    .map(|row| matrix[row][column].powi(2))
                    .sum::<f32>()
                    .sqrt()
            };
            let initial_scale = length(&pose);
            if initial_scale > 0. {
                let ratio = length(transform.matrix()) / initial_scale;
                for row in 0..3 {
                    pose[row][column] *= ratio;
                }
            }
        }
        transform.set_matrix(pose);
    }

    fn apply_pose(&self, entity: &ComRc<IEntity>) {
        entity
            .transform()
            .borrow_mut()
            .set_matrix(self.initial_transform);
        self.apply_position(entity);
    }

    fn apply_position(&self, entity: &ComRc<IEntity>) {
        if let Some(elapsed) = self.progress.elapsed {
            let amount = (elapsed / self.duration).clamp(0., 1.);
            let start = Transform::from_matrix(self.initial_transform).position();
            let position = Vec3::new(
                start.x * (1. - amount) + self.target.x * amount,
                start.y * (1. - amount) + self.target.y * amount,
                start.z * (1. - amount) + self.target.z * amount,
            );
            entity.transform().borrow_mut().set_position(&position);
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.progress.elapsed = None;
        if let Some(rollback) = self.rollback.as_mut() {
            rollback.phase = RollbackPhase::Idle;
            rollback.automatic_pending = false;
        }
        if let Some(linear) = self.linear.as_mut() {
            linear.group = None;
            linear.start = None;
        }
        self.stop_sound();
    }

    fn running(&self) -> bool {
        if let Some(rollback) = &self.rollback {
            return !matches!(rollback.phase, RollbackPhase::Idle);
        }
        if let Some(linear) = &self.linear {
            return linear.group.is_some();
        }
        self.progress
            .elapsed
            .is_some_and(|time| time < self.duration)
    }

    pub(crate) fn stop_sound(&mut self) {
        if let Some(mut source) = self.sound_source.take() {
            source.stop();
            log::debug!(
                "PAL4 motion sound stopped: {}",
                self.sound.as_deref().unwrap_or("")
            );
        }
    }
}

impl Drop for ObjectMotion {
    fn drop(&mut self) {
        self.stop_sound();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SwitchPhase {
    Idle,
    Animation,
    Delay(f32),
    Target,
}

pub(crate) struct ObjectSwitch {
    script: String,
    action: String,
    target: String,
    delay: f32,
    wait_target: bool,
    remaining: i32,
    phase: SwitchPhase,
}

impl ObjectSwitch {
    pub(crate) fn from_entry(entry: &GobEntry, kind: u32) -> Option<RefCell<Self>> {
        let integer = |name, default| {
            entry
                .get_property(name)
                .or_else(|| entry.get_parameter(name))
                .and_then(GobProperty::value_i32)
                .unwrap_or(default)
        };
        if kind != GobObjectType::MACHINE
            || !entry.research_function.to_string().ok()?.is_empty()
            || entry
                .parameters
                .iter()
                .any(|p| p.name().starts_with("PAL4_GAMEOBJECT_float"))
            || integer(Machine::AUTO_TOUCH, 0) != 0
            || integer(Machine::REPEAT, 0) != 0
            || integer(Fixed::STORE, 0) != 0
            || integer(Fixed::RESEARCH_NUM, -1) != 1
            || integer(Fixed::CLIP, 0) != 0
            || integer(Fixed::AUTO_DISAPPEAR, 0) != 0
            || integer(Fixed::VALIDFOR, -1) != -1
            || integer(Fixed::MARKER, 0) != 0
        {
            return None;
        }
        let mut tasks = Vec::new();
        for property in &entry.parameters {
            if let GobProperty::GobPropertyObjectArray(array) = property {
                if property.name() == Machine::CONDITION {
                    if array.0.iter().any(|condition| {
                        condition
                            .get_property(Machine::PROPID)
                            .and_then(GobProperty::value_i32)
                            .is_some_and(|id| id >= 0)
                    }) {
                        return None;
                    }
                } else if property.name().starts_with(Machine::TASK_LIST_PREFIX) {
                    tasks.extend(&array.0);
                }
            }
        }
        let [script, action, linked] = tasks.as_slice() else {
            return None;
        };
        for (task, kind) in [(script, 2), (action, 3), (linked, 17)] {
            if task
                .get_property(Task::TYPE)
                .and_then(GobProperty::value_i32)
                != Some(kind)
                || task
                    .get_property(Task::WAIT_MODE)
                    .and_then(GobProperty::value_i32)
                    != Some(1)
                || task
                    .get_property(Task::EXPT_TIME)
                    .and_then(GobProperty::value_f32)
                    != Some(0.)
            {
                return None;
            }
        }
        if script
            .get_property(Task::SCRIPT_WAIT)
            .and_then(GobProperty::value_i32)
            != Some(0)
            || action
                .get_property(Task::ACTION_TIME)
                .and_then(GobProperty::value_i32)
                != Some(1)
            || [script, action].iter().any(|task| {
                task.get_property(Task::WAIT_TIME)
                    .and_then(GobProperty::value_f32)
                    != Some(0.)
            })
        {
            return None;
        }
        let delay = linked.get_property(Task::WAIT_TIME)?.value_f32()?;
        if !delay.is_finite() || delay < 0. {
            return None;
        }
        let wait_target = match linked.get_property(Task::TOUCH_OTHER_WAIT)?.value_i32()? {
            0 => false,
            1 => true,
            _ => return None,
        };
        let script = script.get_property(Task::SCRIPT_FUNC)?.value_string()?;
        let action = action.get_property(Task::ACTION_NAME)?.value_string()?;
        let target = linked
            .get_property(Task::TOUCH_OTHER_NAME)?
            .value_string()?;
        if [script, action, target].iter().any(|name| name.is_empty()) {
            return None;
        }
        Some(RefCell::new(Self {
            script: script.to_owned(),
            action: action.to_owned(),
            target: target.to_owned(),
            delay,
            wait_target,
            remaining: 1,
            phase: SwitchPhase::Idle,
        }))
    }

    pub(crate) fn available(&self) -> bool {
        self.remaining != 0 && self.phase == SwitchPhase::Idle
    }

    fn start(&mut self) -> Option<String> {
        if !self.available() {
            return None;
        }
        self.remaining -= 1;
        self.phase = SwitchPhase::Animation;
        Some(self.script.clone())
    }

    fn advance(&mut self, delta_sec: f32, animation_running: bool, target_running: bool) -> bool {
        if !delta_sec.is_finite() || delta_sec <= 0. {
            return false;
        }
        match &mut self.phase {
            SwitchPhase::Animation if !animation_running => self.phase = SwitchPhase::Delay(0.),
            SwitchPhase::Delay(elapsed) => {
                *elapsed += delta_sec;
                // Mode 1's predecessor-relative delay is strictly greater than the authored time.
                if *elapsed > self.delay {
                    self.phase = if self.wait_target {
                        SwitchPhase::Target
                    } else {
                        SwitchPhase::Idle
                    };
                    return true;
                }
            }
            SwitchPhase::Target if !target_running => self.phase = SwitchPhase::Idle,
            _ => {}
        }
        false
    }

    pub(crate) fn cancel(&mut self) {
        self.phase = SwitchPhase::Idle;
    }
}

impl Pal4VmContext {
    pub(crate) fn move_script_object(
        &self,
        name: &str,
        duration: f32,
        target: Vec3,
    ) -> Option<Rc<Cell<bool>>> {
        let entity = self.scene.borrow().get_object(name)?;
        let component = object_component(&entity)?;
        let motion = ScriptObjectMotion::new(&entity, duration, target)?;
        let completed = motion.completed.clone();
        *component
            .inner::<Pal4ObjectComponent>()
            .script_motion
            .borrow_mut() = Some(motion);
        Some(completed)
    }

    pub(crate) fn start_object_switch(&mut self, entity: &ComRc<IEntity>) -> Option<String> {
        let component = object_component(entity)?;
        let component = component.inner::<Pal4ObjectComponent>();
        let switch = component.switch.as_ref()?;
        let mut switch = switch.borrow_mut();
        if component.entry().active == 0
            || !entity.enabled()
            || !entity.visible()
            || !switch.available()
        {
            return None;
        }
        let height_limited = component
            .entry()
            .get_property(Fixed::HEIGHT_LIMIT)
            .and_then(GobProperty::value_i32)
            .unwrap_or(0)
            != 0;
        if height_limited {
            let leader = self.session_handle().borrow().state().leader();
            let scene = self.scene.borrow();
            let player_y = scene.get_player(leader).world_transform().position().y;
            let object_y = entity.world_transform().position().y;
            let tolerance = scene.player_interaction_diameters[leader];
            if !(object_y > player_y - tolerance && object_y < player_y + tolerance) {
                return None;
            }
        }
        let module = self.scene.borrow().module.clone()?;
        if !module
            .borrow()
            .functions
            .iter()
            .any(|function| function.name.as_str() == switch.script.as_str())
        {
            return None;
        }
        let target = self.scene.borrow().get_object(&switch.target)?;
        if !object_component(&target)
            .is_some_and(|c| c.inner::<Pal4ObjectComponent>().motion.is_some())
        {
            return None;
        }
        let animation = match self
            .loader
            .load_object_animation(&component.folder(), &switch.action)
        {
            Ok(animation) => animation,
            Err(error) => {
                log::warn!("PAL4 switch animation '{}': {error:#}", switch.action);
                return None;
            }
        };
        if !play_object_animation(entity, animation.keyframes, animation.events, false, true) {
            return None;
        }
        log::debug!(
            "PAL4 switch started: {} -> {}",
            entity.name(),
            switch.target
        );
        switch.start()
    }

    pub(crate) fn tick_object_switches(&mut self, delta_sec: f32) {
        let entities = self.scene.borrow().scene.find_entities_by_tag(TAG_OBJECT);
        for entity in entities {
            if !entity.enabled() {
                continue;
            }
            let Some(component) = object_component(&entity) else {
                continue;
            };
            let Some(switch) = component.inner::<Pal4ObjectComponent>().switch.as_ref() else {
                continue;
            };
            let mut switch = switch.borrow_mut();
            if switch.phase == SwitchPhase::Idle {
                continue;
            }
            let target_running =
                self.scene
                    .borrow()
                    .get_object(&switch.target)
                    .is_some_and(|target| {
                        target.enabled()
                            && object_component(&target).is_some_and(|c| {
                                c.inner::<Pal4ObjectComponent>()
                                    .motion
                                    .as_ref()
                                    .is_some_and(|motion| motion.borrow().running())
                            })
                    });
            let animation_running = object_armature(&entity)
                .is_some_and(|armature| armature.animation_state() == AnimationState::Playing);
            if switch.advance(delta_sec, animation_running, target_running) {
                let target = switch.target.clone();
                drop(switch);
                self.touch_object_motion(&target);
                log::debug!("PAL4 switch activated: {} -> {}", entity.name(), target);
            }
        }
    }

    pub(crate) fn touch_object_motion(&mut self, name: &str) -> bool {
        let Some(entity) = self.scene.borrow().get_object(name) else {
            return false;
        };
        let Some(component) = object_component(&entity) else {
            return false;
        };
        let component = component.inner::<Pal4ObjectComponent>();
        if component.object_type() == GobObjectType::FLOAT && !component.platform.get() {
            return false;
        }
        let Some(motion) = component.motion.as_ref() else {
            return false;
        };
        if component.entry().active == 0 || !entity.enabled() {
            return false;
        }
        let started = motion.borrow_mut().start(&entity);
        if started {
            self.start_object_motion_sound(&entity);
        }
        self.persist_object_motion(&entity);
        started
    }

    fn start_object_motion_sound(&mut self, entity: &ComRc<IEntity>) {
        let component = object_component(entity).unwrap();
        let motion = component
            .inner::<Pal4ObjectComponent>()
            .motion
            .as_ref()
            .unwrap();
        let mut motion = motion.borrow_mut();
        motion.stop_sound();
        if !motion.running() {
            return;
        }
        let Some(sound) = motion.sound.as_ref() else {
            return;
        };
        match self.play_sound_internal(sound, Codec::Wav, true) {
            Ok(mut source) => {
                log::debug!("PAL4 motion sound started: {} ({})", sound, entity.name());
                if !entity.enabled() {
                    source.pause();
                }
                motion.sound_source = Some(source);
            }
            Err(error) => log::warn!("PAL4 motion sound '{}': {error:#}", sound),
        }
    }

    pub(crate) fn stop_object_motion_sounds(&self) {
        for entity in self.scene.borrow().scene.find_entities_by_tag(TAG_OBJECT) {
            if let Some(component) = object_component(&entity) {
                if let Some(motion) = component.inner::<Pal4ObjectComponent>().motion.as_ref() {
                    motion.borrow_mut().stop_sound();
                }
            }
        }
    }

    pub(crate) fn persist_object_motion(&self, entity: &ComRc<IEntity>) {
        let Some(component) = object_component(entity) else {
            return;
        };
        let component = component.inner::<Pal4ObjectComponent>();
        let Some(motion) = component.motion.as_ref() else {
            return;
        };
        let motion = motion.borrow();
        if motion.stored {
            let key = self.reward_key(entity);
            self.session_handle()
                .borrow_mut()
                .state_mut()
                .set_object_motion(key, motion.progress.clone());
        }
    }

    pub(crate) fn tick_object_motions(&mut self, delta_sec: f32) {
        let entities = self.scene.borrow().scene.find_entities_by_tag(TAG_OBJECT);
        for entity in entities {
            let Some(component) = object_component(&entity) else {
                continue;
            };
            let component = component.inner::<Pal4ObjectComponent>();
            if component.object_type() != GobObjectType::FLOAT || component.platform.get() {
                if let Some(motion) = component.motion.as_ref() {
                    motion.borrow_mut().advance(&entity, delta_sec);
                    self.persist_object_motion(&entity);
                }
            }
            let mut script_motion = component.script_motion.borrow_mut();
            if let Some(motion) = script_motion.as_mut() {
                motion.advance(&entity, delta_sec);
                if motion.completed.get() {
                    *script_motion = None;
                }
            }
        }
    }

    pub(crate) fn restore_object_motions(&mut self) {
        let entities = self.scene.borrow().scene.find_entities_by_tag(TAG_OBJECT);
        for entity in entities {
            let Some(component) = object_component(&entity) else {
                continue;
            };
            let component = component.inner::<Pal4ObjectComponent>();
            if component.object_type() == GobObjectType::FLOAT && !component.platform.get() {
                continue;
            }
            let Some(motion) = component.motion.as_ref() else {
                continue;
            };
            {
                let mut motion = motion.borrow_mut();
                if entity.enabled()
                    && motion
                        .rollback
                        .as_ref()
                        .is_some_and(|r| r.automatic_pending)
                {
                    motion.start(&entity);
                }
            }
            let restored = self
                .session_handle()
                .borrow()
                .state()
                .object_motion(&self.reward_key(&entity))
                .cloned();
            let Some(mut restored) = restored else {
                continue;
            };
            let mut motion = motion.borrow_mut();
            if !motion.stored {
                continue;
            }
            if let Some(time) = restored.elapsed {
                if !time.is_finite() {
                    continue;
                }
                restored.elapsed = Some(time.clamp(0., motion.duration));
            }
            motion.progress = restored;
            motion.apply_pose(&entity);
            drop(motion);
            self.start_object_motion_sound(&entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        comdef::IPal4ObjectComponent,
        scene::Pal4Scene,
        states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState},
    };
    use super::*;
    use radiance::{
        comdef::{IComponent, IEntityExt},
        scene::CoreEntity,
    };

    use radiance::audio::AudioSource;
    use std::sync::{Arc, Mutex};

    #[test]
    fn scripted_motion_uses_duration_and_xyz_without_changing_rotation_or_scale() {
        let entity = CoreEntity::create("prop".into(), true);
        entity
            .transform()
            .borrow_mut()
            .scale_local(&Vec3::new(2., 3., 4.))
            .rotate_axis_angle_local(&Vec3::UP, 0.7)
            .set_position(&Vec3::new(30., 40., 50.));
        let initial = *entity.transform().borrow().matrix();
        let mut motion = ScriptObjectMotion::new(&entity, 2., Vec3::new(10., 20., -30.)).unwrap();
        let completed = motion.completed.clone();
        assert_eq!(
            entity.transform().borrow().matrix().floats(),
            initial.floats()
        );
        for delta in [0., -1., f32::NAN, f32::INFINITY] {
            motion.advance(&entity, delta);
        }
        assert_eq!(motion.elapsed, 0.);
        motion.advance(&entity, 1.);
        let position = entity.transform().borrow().position();
        assert_eq!([position.x, position.y, position.z], [20., 30., 10.]);
        assert!(!completed.get());
        motion.advance(&entity, 100.);
        let position = entity.transform().borrow().position();
        assert_eq!([position.x, position.y, position.z], [10., 20., -30.]);
        assert!(completed.get());
        for row in 0..3 {
            for column in 0..3 {
                assert_eq!(
                    entity.transform().borrow().matrix()[row][column],
                    initial[row][column]
                );
            }
        }
    }

    #[test]
    fn scripted_motion_rejects_invalid_duration_and_finishes_if_disabled() {
        let entity = CoreEntity::create("prop".into(), true);
        for duration in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(ScriptObjectMotion::new(&entity, duration, Vec3::new(1., 2., 3.)).is_none());
        }
        assert!(ScriptObjectMotion::new(&entity, 1., Vec3::new(f32::NAN, 2., 3.)).is_none());
        let mut motion = ScriptObjectMotion::new(&entity, 2., Vec3::new(10., 20., 30.)).unwrap();
        motion.advance(&entity, 1.);
        let position = entity.transform().borrow().position();
        entity.set_enabled(false);
        motion.advance(&entity, 1.);
        assert!(motion.completed.get());
        entity.set_enabled(true);
        motion.advance(&entity, 1.);
        let actual = entity.transform().borrow().position();
        assert_eq!(
            [actual.x, actual.y, actual.z],
            [position.x, position.y, position.z]
        );
    }

    struct TestSound {
        events: Arc<Mutex<Vec<&'static str>>>,
        state: AudioSourceState,
    }

    impl AudioSource for TestSound {
        fn update(&mut self) {}
        fn play(&mut self, _: bool) {
            self.state = AudioSourceState::Playing;
        }
        fn restart(&mut self) {
            self.play(true);
        }
        fn pause(&mut self) {
            self.events.lock().unwrap().push("pause");
            self.state = AudioSourceState::Paused;
        }
        fn resume(&mut self) {
            self.events.lock().unwrap().push("resume");
            self.state = AudioSourceState::Playing;
        }
        fn stop(&mut self) {
            self.events.lock().unwrap().push("stop");
            self.state = AudioSourceState::Stopped;
        }
        fn state(&self) -> AudioSourceState {
            self.state
        }
    }

    impl AudioMemorySource for TestSound {
        fn set_data(&mut self, _: Vec<u8>, _: Codec) {}
    }

    fn attach_test_sound(motion: &mut ObjectMotion) -> Arc<Mutex<Vec<&'static str>>> {
        let events = Arc::new(Mutex::new(Vec::new()));
        motion.sound_source = Some(Box::new(TestSound {
            events: events.clone(),
            state: AudioSourceState::Playing,
        }));
        events
    }

    #[test]
    fn owned_motion_sound_pauses_and_stops_on_finish_cancel_and_drop() {
        let entity = CoreEntity::create("door".into(), true);
        let mut motion = motion();
        assert!(motion.start(&entity));
        let events = attach_test_sound(&mut motion);
        assert!(!motion.start(&entity));
        assert!(events.lock().unwrap().is_empty());
        entity.set_enabled(false);
        motion.advance(&entity, 1.);
        motion.advance(&entity, 1.);
        assert_eq!(motion.progress.elapsed, Some(0.));
        entity.set_enabled(true);
        motion.advance(&entity, 2.);
        motion.advance(&entity, 2.);
        assert_eq!(*events.lock().unwrap(), ["pause", "resume", "stop"]);
        let cancelled = attach_test_sound(&mut motion);
        motion.cancel();
        assert_eq!(*cancelled.lock().unwrap(), ["stop"]);
        let unloaded = attach_test_sound(&mut motion);
        drop(motion);
        assert_eq!(*unloaded.lock().unwrap(), ["stop"]);
    }

    fn switch(wait_target: bool) -> ObjectSwitch {
        ObjectSwitch {
            script: "callback".into(),
            action: "handle".into(),
            target: "door".into(),
            delay: 0.3,
            wait_target,
            remaining: 1,
            phase: SwitchPhase::Idle,
        }
    }

    #[test]
    fn linked_switch_orders_animation_delay_and_optional_target_wait() {
        for wait in [false, true] {
            let mut switch = switch(wait);
            assert_eq!(switch.start().as_deref(), Some("callback"));
            assert!(switch.start().is_none());
            assert!(!switch.advance(10., true, false));
            assert!(!switch.advance(0.1, false, false));
            assert!(!switch.advance(f32::NAN, false, false));
            assert!(!switch.advance(-1., false, false));
            assert!(!switch.advance(0.3, false, false));
            assert!(switch.advance(0.01, false, false));
            assert!(!switch.advance(1., false, true));
            assert_eq!(
                switch.phase,
                if wait {
                    SwitchPhase::Target
                } else {
                    SwitchPhase::Idle
                }
            );
            assert!(!switch.advance(0.1, false, false));
            assert_eq!(switch.phase, SwitchPhase::Idle);
            assert!(switch.start().is_none());
        }
    }

    #[test]
    fn resetting_linked_switch_cancels_pending_activation_without_recharging_it() {
        let mut switch = switch(true);
        assert!(switch.start().is_some());
        switch.advance(0.1, false, false);
        switch.advance(0.1, false, false);
        switch.cancel();
        assert!(!switch.advance(1., false, false));
        assert!(!switch.available());
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_m07_switches_resolve_complete_motion_targets_and_physical_interaction() {
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        use radiance::input::{Axis, AxisState, InputEngine, Key, KeyState, SyntheticInputBridge};
        struct Input;
        impl InputEngine for Input {
            fn get_key_state(&self, _: Key) -> KeyState {
                KeyState::default()
            }
            fn get_axis_state(&self, _: Axis) -> AxisState {
                AxisState::default()
            }
        }
        let input = || {
            let bridge = SyntheticInputBridge::new(std::rc::Rc::new(RefCell::new(Input)));
            bridge.tap(Key::F);
            std::rc::Rc::new(RefCell::new(bridge)) as std::rc::Rc<RefCell<dyn InputEngine>>
        };
        let root = std::env::var("PAL4_ASSET_ROOT").unwrap();
        let vfs = packfs::init_virtual_fs(&root, None);
        let mut found = 0;
        for block in ["2", "3", "4", "5", "8", "10a", "10b", "10c", "11a", "11b"] {
            let bytes = vfs
                .read_to_end(&format!("/gamedata/scenedata/M07/{block}/GameObjs.gob"))
                .unwrap();
            let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            for (entry, kind) in gob.entries.iter().zip(&gob.header.object_types) {
                let Some(switch) = ObjectSwitch::from_entry(entry, *kind) else {
                    continue;
                };
                found += 1;
                let target = gob
                    .entries
                    .iter()
                    .position(|entry| entry.name.to_string().unwrap() == switch.borrow().target)
                    .unwrap();
                assert!(
                    ObjectMotion::from_entry(&gob.entries[target], gob.header.object_types[target])
                        .is_some()
                );
                let animation = format!(
                    "/{}{}.anm",
                    entry.folder.to_string().unwrap(),
                    switch.borrow().action
                )
                .replace("\\", "/");
                assert!(!vfs.read_to_end(&animation).unwrap().is_empty());
                let mut unsupported = entry.clone();
                for property in &mut unsupported.properties {
                    if let GobProperty::GobPropertyI32(property) = property {
                        if property.name == Fixed::STORE {
                            property.value = 1;
                        }
                    }
                }
                assert!(ObjectSwitch::from_entry(&unsupported, *kind).is_none());

                let entity = CoreEntity::create(entry.name.to_string().unwrap(), true);
                let component =
                    Pal4ObjectComponent::create(entry.clone(), *kind, Mat44::new_identity());
                entity.add_component(
                    IPal4ObjectComponent::uuid(),
                    component.query_interface::<IComponent>().unwrap(),
                );
                entity.add_tag(TAG_OBJECT);
                let mut scene = Pal4Scene::new_empty();
                scene.player_interaction_diameters[0] = 80.;
                scene.scene.add_entity(entity.clone());
                assert!(matches!(
                    scene.test_interaction(input(), 0),
                    Some(super::super::scene::SceneInteraction::Switch(_))
                ));
                entity.set_enabled(false);
                assert!(scene.test_interaction(input(), 0).is_none());
                entity.set_enabled(true);
                if entry
                    .get_property(Fixed::HEIGHT_LIMIT)
                    .and_then(GobProperty::value_i32)
                    == Some(1)
                {
                    entity
                        .transform()
                        .borrow_mut()
                        .set_position(&Vec3::new(0., 100., 0.));
                    entity.update_world_transform(&Transform::new());
                    assert!(scene.test_interaction(input(), 0).is_none());
                    entity
                        .transform()
                        .borrow_mut()
                        .set_position(&Vec3::new(0., 0., 0.));
                    entity.update_world_transform(&Transform::new());
                }
                entity.set_visible(false);
                assert!(scene.test_interaction(input(), 0).is_none());
                entity.set_visible(true);
                let component = component.inner::<Pal4ObjectComponent>();
                component.switch.as_ref().unwrap().borrow_mut().start();
                assert!(scene.test_interaction(input(), 0).is_none());
                assert!(scene.reset_object(&entity.name()));
                for _ in 0..2 {
                    assert!(
                        !component
                            .switch
                            .as_ref()
                            .unwrap()
                            .borrow_mut()
                            .advance(1., false, false)
                    );
                }
            }
        }
        assert_eq!(found, 14);
    }

    fn motion() -> ObjectMotion {
        let mut transform = Transform::new();
        transform
            .scale_local(&Vec3::new(2., 2., 2.))
            .rotate_axis_angle_local(&Vec3::UP, 0.4)
            .set_position(&Vec3::new(30., 40., 50.));
        ObjectMotion {
            initial_transform: *transform.matrix(),
            target: Vec3::new(10., 20., -30.),
            duration: 2.,
            stored: true,
            sound: None,
            sound_source: None,
            rollback: None,
            linear: None,
            progress: MotionProgress {
                remaining: 1,
                elapsed: None,
            },
        }
    }

    #[test]
    fn compound_task_clock_uses_strict_delays_and_separate_start_update_finish_ticks() {
        let mut group = MotionTaskGroup::new([0.5, 0., 0.5], 1);
        assert!(!group.advance(0.5, 1.).1);
        assert_eq!(group.tasks[0].phase, TaskPhase::Waiting);
        group.advance(0.125, 1.);
        assert_eq!(group.tasks[0].phase, TaskPhase::Running);
        group.advance(0.125, 1.);
        assert_eq!(group.tasks[0].phase, TaskPhase::Finished);
        assert_eq!(group.tasks[1].phase, TaskPhase::Waiting);
        group.advance(0.125, 1.);
        assert_eq!(group.tasks[1].phase, TaskPhase::Running);
        assert_eq!(group.tasks[1].elapsed, 0.);
        group.advance(0.5, 1.);
        assert_eq!(group.tasks[1].elapsed, 0.5);
        group.advance(0.5, 1.);
        assert_eq!(group.tasks[1].phase, TaskPhase::Finished);
        group.advance(0.5, 1.);
        assert_eq!(group.tasks[2].phase, TaskPhase::Waiting);
        group.advance(0.125, 1.);
        assert_eq!(group.tasks[2].phase, TaskPhase::Running);
        assert!(!group.advance(0.125, 1.).1);
        assert!(group.advance(0.125, 1.).1);
    }

    #[test]
    fn linear_platform_waits_captures_actual_start_and_preserves_scale_and_lifetime() {
        let entity = CoreEntity::create("platform".into(), true);
        entity
            .transform()
            .borrow_mut()
            .scale_local(&Vec3::new(3., 3., 3.));
        let mut motion = motion();
        motion.stored = false;
        motion.progress.remaining = -1;
        motion.linear = Some(LinearProgram {
            waits: [0.5, 0.25],
            group: None,
            start: None,
        });
        assert!(motion.start(&entity));
        assert!(!motion.start(&entity));
        motion.advance(&entity, 0.5);
        assert_eq!(entity.transform().borrow().position().x, 0.);
        motion.advance(&entity, 0.125);
        assert_eq!(entity.transform().borrow().position().x, 30.);
        motion.advance(&entity, 0.125);
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(70., 80., 90.));
        motion.advance(&entity, 0.25);
        assert!(motion.linear.as_ref().unwrap().start.is_none());
        motion.advance(&entity, 0.125);
        assert_eq!(entity.transform().borrow().position().x, 70.);
        entity.set_enabled(false);
        motion.advance(&entity, 10.);
        assert_eq!(entity.transform().borrow().position().x, 70.);
        entity.set_enabled(true);
        motion.advance(&entity, 1.);
        let transform = entity.transform();
        let transform = transform.borrow();
        let position = transform.position();
        assert_eq!([position.x, position.y, position.z], [40., 50., 30.]);
        let scale = (0..3)
            .map(|row| transform.matrix()[row][0].powi(2))
            .sum::<f32>()
            .sqrt();
        assert!((scale - 3.).abs() < 0.0001);
        drop(transform);
        motion.advance(&entity, 1.);
        assert!(motion.running());
        motion.advance(&entity, 0.125);
        assert!(!motion.running());
        assert!(motion.start(&entity));
        motion.cancel();
        motion.advance(&entity, 10.);
        assert_eq!(entity.transform().borrow().position().x, motion.target.x);
        assert!(!motion.running());
    }

    #[test]
    fn rollback_captures_each_leg_start_and_then_reapplies_the_authored_pose() {
        let entity = CoreEntity::create("bob".into(), true);
        let mut motion = motion();
        entity
            .transform()
            .borrow_mut()
            .set_matrix(motion.initial_transform);
        motion.rollback = Some(RollbackProgram {
            waits: [0., 0., 0.],
            phase: RollbackPhase::Idle,
            repeat: false,
            automatic_pending: false,
            outbound_start: None,
            return_start: None,
        });
        assert!(motion.start(&entity));
        motion.advance(&entity, 0.1);
        motion.advance(&entity, 0.1);
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(3., 4., 5.));
        motion.advance(&entity, 0.1);
        motion.advance(&entity, 1.);
        let midpoint = entity.transform().borrow().position();
        assert_eq!([midpoint.x, midpoint.y, midpoint.z], [6.5, 12., -12.5]);
        motion.advance(&entity, 1.);
        for _ in 0..10 {
            motion.advance(&entity, 0.1);
            if matches!(
                motion.rollback.as_ref().unwrap().phase,
                RollbackPhase::Returning(_)
            ) {
                break;
            }
        }
        assert!(matches!(
            motion.rollback.as_ref().unwrap().phase,
            RollbackPhase::Returning(_)
        ));
        entity
            .transform()
            .borrow_mut()
            .set_position(&Vec3::new(70., 80., 90.));
        entity
            .transform()
            .borrow_mut()
            .scale_local(&Vec3::new(1.5, 2., 2.5));
        motion.advance(&entity, 0.1);
        motion.advance(&entity, 1.);
        let midpoint = entity.transform().borrow().position();
        assert_eq!([midpoint.x, midpoint.y, midpoint.z], [36.5, 42., 47.5]);
        motion.advance(&entity, 1.);
        let returned = entity.transform().borrow().position();
        assert_eq!([returned.x, returned.y, returned.z], [3., 4., 5.]);
        motion.advance(&entity, 0.1);
        let mut expected = Transform::from_matrix(motion.initial_transform);
        expected.scale_local(&Vec3::new(1.5, 2., 2.5));
        let actual = *entity.transform().borrow().matrix();
        for (actual, expected) in actual
            .floats()
            .iter()
            .flatten()
            .zip(expected.matrix().floats().iter().flatten())
        {
            assert!((actual - expected).abs() < 0.0001);
        }
    }

    #[test]
    fn automatic_rollback_waits_for_the_return_before_repeating_and_reset_cancels_it() {
        for remaining in [-1, 1] {
            let entity = CoreEntity::create("bob".into(), true);
            let mut motion = motion();
            entity
                .transform()
                .borrow_mut()
                .set_matrix(motion.initial_transform);
            motion.progress.remaining = remaining;
            motion.rollback = Some(RollbackProgram {
                waits: [0., 0., 0.],
                phase: RollbackPhase::Idle,
                repeat: true,
                automatic_pending: true,
                outbound_start: None,
                return_start: None,
            });
            entity.set_enabled(false);
            motion.advance(&entity, 1.);
            assert!(!motion.running());
            entity.set_enabled(true);
            motion.advance(&entity, 0.1);
            assert!(motion.running());
            let mut returned = false;
            for _ in 0..100 {
                motion.advance(&entity, 0.1);
                if !motion.running() {
                    returned = true;
                    break;
                }
            }
            assert!(returned);
            let initial = Transform::from_matrix(motion.initial_transform).position();
            let actual = entity.transform().borrow().position();
            assert_eq!(
                [actual.x, actual.y, actual.z],
                [initial.x, initial.y, initial.z]
            );
            motion.advance(&entity, 0.1);
            assert_eq!(motion.running(), remaining == -1);
            let consumed = motion.progress.remaining;
            motion.cancel();
            motion.advance(&entity, 100.);
            assert!(!motion.running());
            assert_eq!(motion.progress.remaining, consumed);
            assert!(!motion.rollback.as_ref().unwrap().automatic_pending);
        }
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_silent_rollback_programs_return_linearly_and_reject_partial_variants() {
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        let root = std::env::var("PAL4_ASSET_ROOT").unwrap();
        let vfs = packfs::init_virtual_fs(&root, None);
        let mut found = 0;
        for (scene_name, block, names) in [
            ("M13", "1", vec!["SO003"]),
            (
                "M17",
                "5",
                vec![
                    "move1", "move2", "move3", "move4", "move5", "move6", "move7", "move8",
                ],
            ),
            ("Q04", "Q04C", vec!["MO003"]),
        ] {
            let bytes = vfs
                .read_to_end(&format!(
                    "/gamedata/scenedata/{scene_name}/{block}/GameObjs.gob"
                ))
                .unwrap();
            let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            for name in names {
                let index = gob
                    .entries
                    .iter()
                    .position(|e| e.name.to_string().unwrap() == name)
                    .unwrap();
                let entry = &gob.entries[index];
                let runtime =
                    ObjectMotion::from_entry(entry, gob.header.object_types[index]).unwrap();
                let mut motion = runtime.borrow_mut();
                assert!(motion.rollback.is_some());
                assert!(!motion.stored);
                let entity = CoreEntity::create(name.into(), true);
                entity
                    .transform()
                    .borrow_mut()
                    .set_matrix(motion.initial_transform);
                assert!(motion.start(&entity));
                let mut returning = false;
                for _ in 0..300 {
                    motion.advance(&entity, 0.1);
                    if matches!(&motion.rollback.as_ref().unwrap().phase, RollbackPhase::Returning(group) if group.tasks[0].phase == TaskPhase::Running)
                    {
                        returning = true;
                        break;
                    }
                }
                assert!(returning, "{scene_name}/{block}/{name}");
                assert!(motion.running());
                assert!(!motion.start(&entity));
                let duration = motion.duration;
                motion.advance(&entity, duration / 2.);
                let halfway = entity.transform().borrow().position();
                let start = Transform::from_matrix(motion.initial_transform).position();
                let target = motion.target;
                for (actual, expected) in [
                    (halfway.x, (start.x + target.x) / 2.),
                    (halfway.y, (start.y + target.y) / 2.),
                    (halfway.z, (start.z + target.z) / 2.),
                ] {
                    assert!((actual - expected).abs() < 0.001);
                }
                entity.set_enabled(false);
                motion.advance(&entity, duration);
                let paused = entity.transform().borrow().position();
                assert_eq!(
                    [paused.x, paused.y, paused.z],
                    [halfway.x, halfway.y, halfway.z]
                );
                entity.set_enabled(true);
                motion.advance(&entity, duration / 2.);
                for _ in 0..30 {
                    motion.advance(&entity, 0.1);
                    if !motion.running() {
                        break;
                    }
                }
                assert!(!motion.running());
                assert_eq!(
                    entity.transform().borrow().matrix().floats(),
                    motion.initial_transform.floats()
                );
                motion.cancel();
                motion.advance(&entity, 100.);
                assert!(!motion.running());
                found += 1;
                for property_name in [Fixed::STORE, Fixed::CLIP] {
                    let mut unsupported = entry.clone();
                    for property in &mut unsupported.properties {
                        if let GobProperty::GobPropertyI32(p) = property {
                            if p.name == property_name {
                                p.value = 1;
                            }
                        }
                    }
                    assert!(
                        ObjectMotion::from_entry(&unsupported, gob.header.object_types[index])
                            .is_none()
                    );
                }
            }
        }
        assert_eq!(found, 10);
        for (scene_name, block, name) in [("M15", "1", "trans02"), ("Q04", "Q04CY", "MO003")] {
            let bytes = vfs
                .read_to_end(&format!(
                    "/gamedata/scenedata/{scene_name}/{block}/GameObjs.gob"
                ))
                .unwrap();
            let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            let index = gob
                .entries
                .iter()
                .position(|e| e.name.to_string().unwrap() == name)
                .unwrap();
            assert!(
                ObjectMotion::from_entry(&gob.entries[index], gob.header.object_types[index])
                    .is_none()
            );
        }
    }

    #[test]
    fn motion_is_linear_does_not_restart_and_preserves_later_script_rotation() {
        let entity = CoreEntity::create("lift".into(), true);
        let mut motion = motion();
        assert!(motion.start(&entity));
        assert_eq!(motion.progress.remaining, 0);
        assert!(!motion.start(&entity));
        entity
            .transform()
            .borrow_mut()
            .rotate_axis_angle_local(&Vec3::UP, 0.3);
        let rotated = *entity.transform().borrow().matrix();
        motion.advance(&entity, 1.);
        let actual = entity.transform().borrow().position();
        assert_eq!([actual.x, actual.y, actual.z], [20., 30., 10.]);
        for row in 0..3 {
            for column in 0..3 {
                assert_eq!(
                    entity.transform().borrow().matrix()[row][column],
                    rotated[row][column]
                );
            }
        }
        entity.set_enabled(false);
        motion.advance(&entity, 1.);
        assert_eq!(motion.progress.elapsed, Some(1.));
        entity.set_enabled(true);
        for delta in [0., -1., f32::NAN, f32::INFINITY] {
            motion.advance(&entity, delta);
        }
        assert_eq!(motion.progress.elapsed, Some(1.));
        motion.advance(&entity, 100.);
        let actual = entity.transform().borrow().position();
        assert_eq!([actual.x, actual.y, actual.z], [10., 20., -30.]);
        assert_eq!(motion.progress.elapsed, Some(2.));
        assert!(!motion.start(&entity));
    }

    #[test]
    fn stored_motion_resumes_at_saved_time_and_restores_completed_pose() {
        let entity = CoreEntity::create("lift".into(), true);
        let mut original = motion();
        assert!(original.start(&entity));
        original.advance(&entity, 0.5);
        let mut state = Pal4PersistentState::new(PAL4_APP_NAME.into());
        state.set_object_motion("m07\\6\\dy".into(), original.progress.clone());
        let state: Pal4PersistentState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        let mut restored = motion();
        restored.progress = state.object_motion("m07\\6\\dy").unwrap().clone();
        restored.apply_pose(&entity);
        assert_eq!(restored.progress.remaining, 0);
        restored.advance(&entity, 1.5);
        let actual = entity.transform().borrow().position();
        assert_eq!([actual.x, actual.y, actual.z], [10., 20., -30.]);
        let mut completed = motion();
        completed.progress = restored.progress.clone();
        completed.apply_pose(&entity);
        assert_eq!(
            entity.transform().borrow().matrix().floats(),
            Transform::from_matrix(restored.initial_transform)
                .set_position(&restored.target)
                .matrix()
                .floats()
        );
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_motion_metadata_reset_sound_and_unsupported_platforms() {
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        let root = std::env::var("PAL4_ASSET_ROOT").unwrap();
        let vfs = packfs::init_virtual_fs(&root, None);
        for (scene_name, block, name, stored, sound) in [
            ("M07", "6", "dy", true, None),
            ("M07", "7", "ladder", false, None),
            ("M07", "1", "huaRongDoor01", true, Some("wc001")),
            ("M08", "2", "door01", true, Some("WJ069")),
        ] {
            let bytes = vfs
                .read_to_end(&format!(
                    "/gamedata/scenedata/{scene_name}/{block}/GameObjs.gob"
                ))
                .unwrap();
            let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            let index = gob
                .entries
                .iter()
                .position(|entry| entry.name.to_string().unwrap() == name)
                .unwrap();
            let entry = &gob.entries[index];
            let parsed = ObjectMotion::from_entry(entry, gob.header.object_types[index]).unwrap();
            assert_eq!(parsed.borrow().stored, stored);
            assert_eq!(parsed.borrow().sound.as_deref(), sound);
            if let Some(sound) = sound {
                assert!(
                    vfs.read_to_end(&format!("/gamedata/PALSound/{sound}.wav"))
                        .is_ok()
                );
                let mut unsupported = entry.clone();
                for property in &mut unsupported.parameters {
                    if let GobProperty::GobPropertyObjectArray(array) = property {
                        for task in &mut array.0 {
                            for property in &mut task.properties {
                                if let GobProperty::GobPropertyI32(property) = property {
                                    if property.name == Task::MOVE_SOUND_TIME {
                                        property.value = 1;
                                    }
                                }
                            }
                        }
                    }
                }
                assert!(ObjectMotion::from_entry(&unsupported, GobObjectType::MACHINE).is_none());
            }
            let entity = CoreEntity::create(name.into(), true);
            let component = Pal4ObjectComponent::create(
                entry.clone(),
                GobObjectType::MACHINE,
                Mat44::new_identity(),
            );
            entity.add_component(
                IPal4ObjectComponent::uuid(),
                component.query_interface::<IComponent>().unwrap(),
            );
            entity.add_tag(TAG_OBJECT);
            let pending = ScriptObjectMotion::new(&entity, 1., Vec3::new(10., 20., 30.)).unwrap();
            let replaced = pending.completed.clone();
            let component_impl = component.inner::<Pal4ObjectComponent>();
            *component_impl.script_motion.borrow_mut() = Some(pending);
            let replacement =
                ScriptObjectMotion::new(&entity, 2., Vec3::new(20., 30., 40.)).unwrap();
            let unloaded = replacement.completed.clone();
            *component_impl.script_motion.borrow_mut() = Some(replacement);
            assert!(replaced.get());
            assert!(!unloaded.get());
            component
                .query_interface::<IComponent>()
                .unwrap()
                .on_unloading();
            assert!(unloaded.get());
            let mut scene = Pal4Scene::new_empty();
            scene.scene.add_entity(entity.clone());
            let motion = component
                .inner::<Pal4ObjectComponent>()
                .motion
                .as_ref()
                .unwrap();
            let initial = motion.borrow().initial_transform;
            assert!(motion.borrow_mut().start(&entity));
            let duration = motion.borrow().duration;
            motion.borrow_mut().advance(&entity, duration / 2.);
            let halfway = entity.transform().borrow().position();
            let start = Transform::from_matrix(initial).position();
            let target = motion.borrow().target;
            assert!((halfway.y - (start.y + target.y) / 2.).abs() < 0.0001);
            let exhausted = motion.borrow().progress.remaining == 0;
            if !exhausted {
                assert!(scene.reset_object(name));
                motion.borrow_mut().advance(&entity, duration);
                assert_eq!(
                    entity.transform().borrow().matrix().floats(),
                    initial.floats()
                );
                assert!(motion.borrow_mut().start(&entity));
            }
            motion.borrow_mut().advance(&entity, duration);
            let actual = entity.transform().borrow().position();
            assert_eq!(
                [actual.x, actual.y, actual.z],
                [target.x, target.y, target.z]
            );
            if exhausted {
                assert!(scene.reset_object(name));
                assert!(!motion.borrow_mut().start(&entity));
                assert_eq!(
                    entity.transform().borrow().matrix().floats(),
                    initial.floats()
                );
            }
            let events = attach_test_sound(&mut motion.borrow_mut());
            component
                .query_interface::<IComponent>()
                .unwrap()
                .on_unloading();
            assert_eq!(*events.lock().unwrap(), ["stop"]);
        }
        for (scene, block, name) in [
            ("M15", "1", "JPuzzleA03"), // unsupported floating task program
            ("M17", "4", "rotate1"),    // different task program
        ] {
            let bytes = vfs
                .read_to_end(&format!("/gamedata/scenedata/{scene}/{block}/GameObjs.gob"))
                .unwrap();
            let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
            let index = gob
                .entries
                .iter()
                .position(|entry| entry.name.to_string().unwrap() == name)
                .unwrap();
            assert!(
                ObjectMotion::from_entry(&gob.entries[index], gob.header.object_types[index])
                    .is_none()
            );
        }
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_auto_platform_can_restart_after_return_despite_authored_single_use_count() {
        use common::store_ext::StoreExt2;
        use fileformats::{binrw::BinRead, pal4::gob::GobFile};
        let vfs = packfs::init_virtual_fs(&std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        let bytes = vfs
            .read_to_end("/gamedata/scenedata/M17/2/GameObjs.gob")
            .unwrap();
        let gob = GobFile::read(&mut std::io::Cursor::new(bytes)).unwrap();
        let index = gob
            .entries
            .iter()
            .position(|entry| entry.name.to_string().unwrap() == "trans01")
            .unwrap();
        let motion =
            ObjectMotion::from_entry(&gob.entries[index], gob.header.object_types[index]).unwrap();
        let mut motion = motion.borrow_mut();
        let entity = CoreEntity::create("trans01".into(), true);
        for _ in 0..3 {
            for _ in 0..200 {
                motion.advance(&entity, 0.1);
            }
            assert!(!motion.running());
            assert_eq!(
                entity.transform().borrow().matrix().floats(),
                motion.initial_transform.floats()
            );
            assert!(motion.start(&entity));
            assert!(!motion.start(&entity));
        }
    }
}
