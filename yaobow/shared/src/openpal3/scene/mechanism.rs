//! Clean-room PAL3 mechanism rules, traced in the supplied original executable.
//! SCE 124: 0x438350; SCE 126: 0x438390; dispatcher: 0x424c20.
//! Type 23: 0x425b25..0x425c86, completion/link: 0x429396..0x42953d.
//! Type 22: 0x4259d9..0x425a88, motion: 0x428a66..0x428ba7 (30 units/s).
//! Types 7/8: 0x4251a4, 0x427edd; type 31: 0x4263c6, 0x42971c.
//! Type 5 path motion: 0x41f120/0x427444..0x427cda; type 41: 0x426b7e/0x429917.
//! Plates/hazards: 0x426273/0x429547; linked CVD/movers: 0x42594a/0x42652b.
//! Type 26 colored chests and other native object types are deliberately not approximated here.
//! Boarding: 0x42b450/0x42b510; manual switch action: 0x425c89/0x42924b.
//! The switch's special water/effect parameters are not implemented yet.
//! Native mechanism camera cues and sound actions are not reproduced here.

use serde::{Deserialize, Serialize};

use crate::openpal3::loaders::scn_loader::ScnNode;
use crate::openpal3::scene::path::Polyline;
use radiance::math::Vec3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismState {
    pub remaining: u8,
    pub switched: u8,
    pub height: f32,
    pub raised: bool,
    pub motion: Option<f32>,
    pub animation_time: Option<f32>,
    #[serde(default)]
    pub native: Option<NativeState>,
    #[serde(default)]
    pub layer: Option<u16>,
    #[serde(default)]
    pub platform: Option<PlatformState>,
    #[serde(default)]
    pub interaction: Option<SwitchInteraction>,
    #[serde(default)]
    pub action: Option<MechanismAction>,
    #[serde(default)]
    pub hazard: Option<HazardState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HazardState {
    pub elapsed: f32,
    pub swing: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionPhase {
    Idle,
    Approach,
    Animating,
    Moving,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismAction {
    pub phase: ActionPhase,
    pub player: Option<String>,
    pub position: Vec3,
    origin: Vec3,
    target: Vec3,
    elapsed: f32,
    pub animation_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickupState {
    pub claimed: bool,
    pub interaction: Option<ApproachInteraction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproachInteraction {
    pub player: String,
    pub target: Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportState {
    pub phase: TransportPhase,
    pub distance: f32,
    pub rider: Option<String>,
    pub approach: Option<ApproachInteraction>,
    pub exit: Option<Vec3>,
    pub waypoint: usize,
    pub direction: i8,
    pub elapsed: f32,
    pub duration: f32,
    pub incoming: Vec3,
    pub position: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TransportPhase {
    Start,
    End,
    Boarding,
    Outbound,
    Return,
    Disembarking,
    Select,
    Segment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformState {
    pub rider: Option<String>,
    pub boarding_pending: bool,
    #[serde(default)]
    pub pickup: Option<PickupState>,
    #[serde(default)]
    pub transport: Option<TransportState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchInteraction {
    pub player: String,
    pub target: Vec3,
    pub phase: SwitchPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwitchPhase {
    Approach,
    Action {
        #[serde(skip)]
        started: bool,
    },
    Animating,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeState {
    pub active: bool,
    pub rotation: f32,
    pub rotation_target: Option<f32>,
    pub movement: Option<Movement>,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movement {
    from: f32,
    to: f32,
    elapsed: f32,
    duration: f32,
}

impl Movement {
    fn new(from: f32, to: f32, duration: f32) -> Self {
        Self {
            from,
            to,
            elapsed: 0.,
            duration,
        }
    }

    fn update(&mut self, delta_sec: f32) -> (f32, bool) {
        self.elapsed = (self.elapsed + delta_sec).min(self.duration);
        let fraction = if self.duration > 0. {
            self.elapsed / self.duration
        } else {
            1.
        };
        // Native action type 1, 0x42f4d2: half-cosine easing.
        let weight = (1. - (std::f32::consts::PI * fraction).cos()) * 0.5;
        (
            self.from * (1. - weight) + self.to * weight,
            self.elapsed >= self.duration,
        )
    }
}

pub struct Mechanism {
    pub id: u16,
    pub kind: u16,
    pub active: bool,
    pub layer: u16,
    pub position: radiance::math::Vec3,
    pub state: MechanismState,
    pub target: u16,
    proc_id: u32,
    mode: i32,
    persistent_switch: bool,
    target_height: f32,
    path: Option<Polyline>,
    rotation: f32,
    amplitude: f32,
    sound: String,
}

#[derive(Debug, PartialEq)]
pub enum Activation {
    Ignored,
    Script(u32),
    Started,
    Interact,
}

impl Mechanism {
    pub fn from_node(node: &ScnNode) -> Option<Self> {
        if !matches!(
            node.node_type,
            5 | 7 | 8 | 19 | 22 | 23 | 28 | 29 | 31 | 34 | 41
        ) {
            return None;
        }
        let path = (node.node_type == 5)
            .then(|| Polyline::new(&node.path_waypoints))
            .flatten();
        if node.node_type == 5 && path.is_none() {
            log::warn!("PAL3 path transport {} has no valid path", node.index);
            return None;
        }
        if node.node_type == 23 && node.ladder_nav_coord2.0 != 0 {
            log::warn!(
                "PAL3 switch {} has an unsupported special effect parameter",
                node.index
            );
        }
        Some(Self {
            id: node.index,
            kind: node.node_type,
            active: node.active != 0,
            layer: node.nav_layer,
            position: node.position,
            state: MechanismState {
                remaining: node.remaining_triggers,
                switched: node.switch_state,
                height: node.position.y,
                raised: false,
                motion: None,
                animation_time: None,
                native: matches!(node.node_type, 7 | 8 | 31).then_some(NativeState {
                    active: node.active != 0,
                    rotation: node.rotation,
                    rotation_target: None,
                    movement: None,
                    completed: false,
                }),
                layer: Some(node.nav_layer),
                platform: matches!(node.node_type, 5 | 22 | 41).then_some(PlatformState {
                    rider: None,
                    boarding_pending: false,
                    pickup: (node.node_type == 41).then_some(PickupState {
                        claimed: false,
                        interaction: None,
                    }),
                    transport: (node.node_type == 5).then_some(TransportState {
                        phase: TransportPhase::Start,
                        distance: 0.,
                        rider: None,
                        approach: None,
                        exit: None,
                        waypoint: 0,
                        direction: 1,
                        elapsed: 0.,
                        duration: 0.,
                        incoming: Vec3::new(0., 0., 1.),
                        position: node
                            .path_waypoints
                            .first()
                            .copied()
                            .unwrap_or(node.position),
                    }),
                }),
                interaction: None,
                action: matches!(node.node_type, 19 | 29 | 34).then_some(MechanismAction {
                    phase: ActionPhase::Idle,
                    player: None,
                    position: node.position,
                    origin: node.position,
                    target: node.position,
                    elapsed: 0.,
                    animation_complete: false,
                }),
                hazard: (node.node_type == 28).then_some(HazardState {
                    elapsed: 0.,
                    swing: 0.,
                }),
            },
            target: node.w148,
            proc_id: node.sce_proc_id,
            mode: if matches!(node.node_type, 5 | 28) {
                node.ladder_nav_coord1.0
            } else {
                node.ladder_nav_coord1.1
            },
            persistent_switch: node.ladder_nav_coord1.0 != 0,
            target_height: node.ladder_nav_coord1.0 as f32,
            path,
            rotation: node.rotation,
            amplitude: node.ladder_nav_coord1.1 as f32,
            sound: {
                let bytes: Vec<_> = node
                    .dw184
                    .iter()
                    .skip(1)
                    .take(4)
                    .flat_map(|v| v.to_le_bytes())
                    .take_while(|&b| b != 0)
                    .collect();
                String::from_utf8_lossy(&bytes).into_owned()
            },
        })
    }

    pub fn restore(&mut self, state: &MechanismState) {
        let initial_native = self.state.native.clone();
        let initial_platform = self.state.platform.clone();
        let initial_action = self.state.action.clone();
        let initial_hazard = self.state.hazard.clone();
        self.state = state.clone();
        if self.state.native.is_none() {
            self.state.native = initial_native;
        }
        if let Some(native) = &self.state.native {
            self.active = native.active;
        }
        if self.state.platform.is_none() {
            self.state.platform = initial_platform;
        }
        if self.state.action.is_none() {
            self.state.action = initial_action;
        }
        if self.state.hazard.is_none() {
            self.state.hazard = initial_hazard;
        }
        if self.kind == 41
            && self
                .state
                .platform
                .as_ref()
                .is_none_or(|state| state.pickup.is_none())
        {
            self.state.platform = Some(PlatformState {
                rider: None,
                boarding_pending: false,
                pickup: Some(PickupState {
                    claimed: false,
                    interaction: None,
                }),
                transport: None,
            });
        }
        if self.kind == 5
            && self
                .state
                .platform
                .as_ref()
                .is_none_or(|state| state.transport.is_none())
        {
            self.state.platform = Some(PlatformState {
                rider: None,
                boarding_pending: false,
                pickup: None,
                transport: Some(TransportState {
                    phase: TransportPhase::Start,
                    distance: 0.,
                    rider: None,
                    approach: None,
                    exit: None,
                    waypoint: 0,
                    direction: 1,
                    elapsed: 0.,
                    duration: 0.,
                    incoming: Vec3::new(0., 0., 1.),
                    position: self
                        .path
                        .as_ref()
                        .map(Polyline::first)
                        .unwrap_or(self.position),
                }),
            });
        }
        if self.pickup_claimed() {
            self.active = false;
        }
        self.layer = self.state.layer.unwrap_or_else(|| {
            if self.kind == 22 && self.mode != 0 {
                u16::from(self.state.raised)
            } else {
                self.layer
            }
        });
        self.state.layer = Some(self.layer);
        if let Some(SwitchInteraction {
            phase: SwitchPhase::Action { started },
            ..
        }) = &mut self.state.interaction
        {
            *started = false;
        }
    }

    pub fn restore_raised(&mut self) {
        self.state.height = self.target_height;
        self.state.raised = true;
        if self.mode != 0 {
            self.layer = 1;
            self.state.layer = Some(1);
        }
    }

    pub fn activate(&mut self, scripted: bool) -> Activation {
        // Explicit Trigger ignores visibility/active, but not the use counter.
        if self.state.remaining == 0 || (!scripted && !self.active) {
            return Activation::Ignored;
        }
        if matches!(self.kind, 5 | 41) {
            return Activation::Ignored;
        }
        if self.kind == 28 {
            if !scripted || self.mode != 1 {
                return Activation::Ignored;
            }
            self.state.switched = u8::from(self.state.switched == 0);
            return Activation::Started;
        }
        if let Some(action) = &mut self.state.action {
            if action.phase != ActionPhase::Idle {
                return Activation::Ignored;
            }
            if self.kind == 29 {
                return if scripted {
                    Activation::Ignored
                } else {
                    Activation::Interact
                };
            }
            if !scripted {
                return Activation::Ignored;
            }
            action.elapsed = 0.;
            if self.kind == 19 {
                action.phase = ActionPhase::Animating;
                self.state.animation_time = Some(0.);
            } else {
                action.phase = ActionPhase::Moving;
                action.origin = action.position;
                // 0x426570 uses the authored value directly in x87 sin/cos,
                // unlike the degree-to-radian conversion in the model transform.
                let distance = if self.target_height > 0. {
                    self.target_height
                } else {
                    120.
                };
                let (sin, cos) = (self.rotation as f64).sin_cos();
                // Native spills sin, but keeps the Z product extended through translation.
                action.target = Vec3::new(
                    action.position.x + (cos * distance as f64) as f32,
                    action.position.y,
                    (action.position.z as f64 + sin as f32 as f64 * distance as f64) as f32,
                );
                if self.state.remaining != u8::MAX {
                    self.state.remaining -= 1;
                }
            }
            return Activation::Started;
        }
        if self.kind == 23 {
            if self.state.animation_time.is_some() || self.state.interaction.is_some() {
                return Activation::Ignored;
            }
            if !scripted {
                match self.mode {
                    0 => return Activation::Interact,
                    2 if self.proc_id != 0 => return Activation::Script(self.proc_id),
                    _ => return Activation::Ignored,
                }
            } else if self.mode == 0 {
                return Activation::Ignored;
            }
            self.start_switch_animation();
        } else if matches!(self.kind, 7 | 8 | 31) {
            let native = self.state.native.as_mut().unwrap();
            if !scripted || native.rotation_target.is_some() || native.completed {
                return Activation::Ignored;
            }
            if self.kind == 31 {
                // Retriggers still call the proc. Deactivation after the first
                // movement prevents subsequent native queued moves from running.
                if native.movement.is_none() {
                    native.movement = Some(Movement::new(
                        self.state.height,
                        self.target_height,
                        (self.target_height - self.state.height).abs() * 0.25,
                    ));
                }
            } else {
                native.rotation_target = Some(native.rotation + 90.);
            }
        } else {
            if !scripted || self.state.motion.is_some() {
                return Activation::Ignored;
            }
            self.state.motion = Some(if self.state.raised {
                self.position.y
            } else {
                self.target_height
            });
            if let Some(platform) = &mut self.state.platform {
                platform.boarding_pending = true;
                platform.rider = None;
            }
        }
        if self.kind != 31 && self.state.remaining != u8::MAX {
            self.state.remaining -= 1;
        }
        Activation::Started
    }

    pub fn begin_switch_action(&mut self) {
        self.state.switched = u8::from(self.state.switched == 0);
        if self.state.remaining != u8::MAX {
            self.state.remaining -= 1;
        }
        if let Some(interaction) = &mut self.state.interaction {
            interaction.phase = SwitchPhase::Action { started: false };
        }
    }

    pub fn start_switch_animation(&mut self) {
        self.state.animation_time = Some(0.);
        if let Some(interaction) = &mut self.state.interaction {
            interaction.phase = SwitchPhase::Animating;
        } else {
            self.state.switched = u8::from(self.state.switched == 0);
        }
    }

    pub fn animation_time(&self) -> Option<f32> {
        self.state.animation_time
    }

    pub fn pose_at_end(&self) -> bool {
        if let Some(action) = &self.state.action {
            return action.animation_complete;
        }
        self.state.remaining == 0 || (self.persistent_switch && self.state.switched != 0)
    }

    pub fn plate_requirement(&self) -> usize {
        self.target_height.max(0.) as usize
    }

    pub fn begin_plate(&mut self, player: String) {
        let action = self.state.action.as_mut().unwrap();
        action.player = Some(player);
        action.phase = ActionPhase::Approach;
    }

    pub fn finish_plate_approach(&mut self) {
        let action = self.state.action.as_mut().unwrap();
        action.player = None;
        action.phase = ActionPhase::Animating;
        self.state.animation_time = Some(0.);
    }

    pub fn action_locked(&self) -> bool {
        self.kind != 34
            && self.state.action.as_ref().is_some_and(|action| {
                matches!(
                    action.phase,
                    ActionPhase::Approach | ActionPhase::Animating | ActionPhase::Moving
                )
            })
    }

    pub fn action_sound(&self, variant: bool) -> &str {
        if !self.sound.is_empty() {
            return &self.sound;
        }
        match self.kind {
            19 => "Wg001",
            29 if variant => "We026",
            _ => "Wg005",
        }
    }

    pub fn update_hazard(&mut self, delta_sec: f32, ground: Option<f32>) {
        if !self.active {
            return;
        }
        let Some(hazard) = &mut self.state.hazard else {
            return;
        };
        hazard.elapsed += delta_sec.max(0.);
        if self.mode == 0 {
            hazard.swing = hazard.elapsed.sin() * 24.;
        } else if self.state.switched != 0 {
            self.state.height =
                self.position.y + (hazard.elapsed.sin() + 1.) * self.amplitude * 0.5;
            if let Some(ground) = ground {
                self.state.height = self.state.height.max(ground);
            }
        }
    }

    pub fn hazard_damage(&self) -> Option<f32> {
        (self.active && self.kind == 28 && (self.mode != 1 || self.state.switched != 0))
            .then_some(if self.mode == 1 { 0.15 } else { 0.1 })
    }

    /// Returns true once when a mechanism completes, allowing its link/proc to run.
    pub fn update(&mut self, delta_sec: f32, animation_duration: f32) -> bool {
        let delta_sec = delta_sec.max(0.);
        if let Some(action) = &mut self.state.action {
            if !self.active {
                return false;
            }
            match action.phase {
                ActionPhase::Animating => {
                    let time = self.state.animation_time.get_or_insert(0.);
                    *time = (*time + delta_sec * 2.).min(animation_duration);
                    if *time >= animation_duration {
                        self.state.animation_time = None;
                        action.animation_complete = true;
                        if self.kind == 19 {
                            action.phase = ActionPhase::Complete;
                        } else {
                            action.phase = ActionPhase::Moving;
                            action.origin = action.position;
                            action.target = action.position;
                            action.target.y -= 7.;
                            action.elapsed = 0.;
                        }
                    }
                }
                ActionPhase::Moving => {
                    let duration = if self.kind == 29 { 2.5 } else { 4. };
                    action.elapsed = (action.elapsed + delta_sec).min(duration);
                    let weight =
                        (1. - (std::f32::consts::PI * action.elapsed / duration).cos()) * 0.5;
                    action.position = Vec3::lerp(&action.origin, &action.target, weight);
                    self.state.height = action.position.y;
                    if action.elapsed >= duration {
                        action.position = action.target;
                        action.phase = if self.kind == 29 {
                            ActionPhase::Complete
                        } else {
                            ActionPhase::Idle
                        };
                        return true;
                    }
                }
                _ => {}
            }
            return false;
        }
        if let Some(native) = &mut self.state.native {
            if !self.active {
                return false;
            }
            if let Some(target) = native.rotation_target {
                native.rotation = (native.rotation + delta_sec * 30.).min(target);
                if native.rotation >= target {
                    native.rotation_target = None;
                    return true;
                }
            }
            if let Some(movement) = &mut native.movement {
                let (height, complete) = movement.update(delta_sec);
                self.state.height = height;
                if complete {
                    native.movement = None;
                    native.completed = true;
                    native.active = false;
                    self.active = false;
                    return true;
                }
            }
            return false;
        }
        if let Some(time) = &mut self.state.animation_time {
            *time = (*time + delta_sec * 2.).min(animation_duration);
            if *time >= animation_duration {
                self.state.animation_time = None;
                return true;
            }
        }
        if let Some(target) = self.state.motion {
            let step = delta_sec * 30.;
            // The original chooses direction from the phase, not the height.
            let arrived = if self.state.raised {
                self.state.height -= step;
                self.state.height <= target
            } else {
                self.state.height += step;
                self.state.height >= target
            };
            if arrived {
                self.state.height = target;
                self.state.raised = !self.state.raised;
                self.state.motion = None;
                if self.mode != 0 {
                    self.layer = u16::from(self.state.raised);
                    self.state.layer = Some(self.layer);
                }
                return true;
            }
        }
        false
    }

    pub fn completion_proc(&self) -> Option<u32> {
        (matches!(self.kind, 22 | 23) && self.proc_id != 0).then_some(self.proc_id)
    }

    pub fn activation_proc(&self) -> Option<u32> {
        (matches!(self.kind, 29 | 31) && self.proc_id != 0).then_some(self.proc_id)
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if let Some(native) = &mut self.state.native {
            native.active = active;
        }
    }

    pub fn provides_floor(&self) -> bool {
        self.active
            && (self.kind == 5
                && self.transport().is_some_and(|s| {
                    matches!(s.phase, TransportPhase::Start | TransportPhase::End)
                })
                || self.kind == 22
                || self.kind == 29
                || self.kind == 19
                    && self
                        .state
                        .action
                        .as_ref()
                        .is_some_and(|a| a.animation_complete)
                || self.kind == 34
                    && self
                        .state
                        .action
                        .as_ref()
                        .is_some_and(|a| a.phase == ActionPhase::Idle)
                || self.kind == 7
                    && self
                        .state
                        .native
                        .as_ref()
                        .is_some_and(|s| s.rotation_target.is_none()))
    }

    pub fn saved_state(&self) -> MechanismState {
        self.state.clone()
    }

    pub fn path(&self) -> Option<&Polyline> {
        self.path.as_ref()
    }

    pub fn mode(&self) -> i32 {
        self.mode
    }

    pub fn claim_pickup(&mut self) -> bool {
        let Some(pickup) = self
            .state
            .platform
            .as_mut()
            .and_then(|state| state.pickup.as_mut())
        else {
            return false;
        };
        if pickup.claimed {
            return false;
        }
        pickup.claimed = true;
        pickup.interaction = None;
        self.active = false;
        true
    }

    pub fn pickup_claimed(&self) -> bool {
        self.state
            .platform
            .as_ref()
            .and_then(|state| state.pickup.as_ref())
            .is_some_and(|p| p.claimed)
    }

    pub fn set_transport_switch(&mut self, switched: u8) {
        self.state.switched = switched;
        if switched == 0 {
            return;
        }
        let Some(path) = &self.path else {
            return;
        };
        let Some(state) = self
            .state
            .platform
            .as_mut()
            .and_then(|state| state.transport.as_mut())
        else {
            return;
        };
        state.rider = None;
        state.approach = None;
        state.exit = None;
        match state.phase {
            TransportPhase::Start => {
                state.distance = 0.;
                state.phase = TransportPhase::Outbound;
            }
            _ => {
                state.distance = path.total();
                state.phase = TransportPhase::Return;
            }
        }
    }

    pub fn pickup(&self) -> Option<&PickupState> {
        self.state.platform.as_ref()?.pickup.as_ref()
    }

    pub fn pickup_mut(&mut self) -> Option<&mut PickupState> {
        self.state.platform.as_mut()?.pickup.as_mut()
    }

    pub fn transport(&self) -> Option<&TransportState> {
        self.state.platform.as_ref()?.transport.as_ref()
    }

    pub fn take_transport(&mut self) -> Option<TransportState> {
        self.state.platform.as_mut()?.transport.take()
    }

    pub fn put_transport(&mut self, state: TransportState) {
        if let Some(platform) = &mut self.state.platform {
            platform.transport = Some(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mechanism(kind: u16) -> Mechanism {
        Mechanism {
            id: 8,
            kind,
            active: true,
            layer: 0,
            position: radiance::math::Vec3::new(0., 10., 0.),
            state: MechanismState {
                remaining: 1,
                switched: 0,
                height: 10.,
                raised: false,
                motion: None,
                animation_time: None,
                native: None,
                layer: Some(0),
                platform: None,
                interaction: None,
                action: None,
                hazard: None,
            },
            target: 3,
            proc_id: 500,
            mode: 1,
            persistent_switch: true,
            target_height: 40.,
            path: None,
            rotation: 0.,
            amplitude: 0.,
            sound: String::new(),
        }
    }

    #[test]
    fn switch_script_mode_defers_toggle_until_explicit_trigger() {
        let mut switch = mechanism(23);
        switch.mode = 2;
        assert_eq!(switch.activate(false), Activation::Script(500));
        assert_eq!(switch.state.remaining, 1);
        assert_eq!(switch.state.switched, 0);
        assert_eq!(switch.activate(true), Activation::Started);
        assert_eq!(switch.state.switched, 1);
        assert_eq!(switch.state.remaining, 0);
        assert!(!switch.update(0.2, 1.));
        assert!(switch.update(0.3, 1.));
        assert!(!switch.update(1., 1.));
        assert_eq!(switch.activate(true), Activation::Ignored);
    }

    #[test]
    fn unlimited_switch_rejects_reentry_but_can_toggle_again() {
        let mut switch = mechanism(23);
        switch.state.remaining = 255;
        switch.mode = 1;
        assert_eq!(switch.activate(false), Activation::Ignored);
        assert_eq!(switch.activate(true), Activation::Started);
        assert_eq!(switch.activate(true), Activation::Ignored);
        assert_eq!(switch.state.switched, 1);
        assert!(switch.update(1., 1.));
        assert_eq!(switch.activate(true), Activation::Started);
        assert_eq!(switch.state.switched, 0);
        assert_eq!(switch.state.remaining, 255);
    }

    #[test]
    fn inactive_platform_accepts_script_and_moves_at_native_speed() {
        let mut platform = mechanism(22);
        platform.active = false;
        platform.state.remaining = 255;
        assert_eq!(platform.activate(false), Activation::Ignored);
        assert_eq!(platform.activate(true), Activation::Started);
        assert!(!platform.update(0.5, 0.));
        assert_eq!(platform.state.height, 25.);
        assert!(platform.update(1., 0.));
        assert_eq!(platform.state.height, 40.);
        assert_eq!(platform.activate(true), Activation::Started);
        assert!(platform.update(2., 0.));
        assert_eq!(platform.state.height, 10.);
    }

    #[test]
    fn restored_motion_completes_once_and_retains_nonpersistent_switch_flag() {
        let mut platform = mechanism(22);
        platform.activate(true);
        platform.update(0.5, 0.);
        let mut restored = mechanism(22);
        restored.restore(&platform.saved_state());
        assert!(restored.update(0.5, 0.));
        assert!(!restored.update(1., 0.));
        assert!(restored.state.raised);

        let mut switch = mechanism(23);
        switch.persistent_switch = false;
        switch.state.remaining = 255;
        switch.activate(true);
        switch.update(1., 1.);
        let mut restored = mechanism(23);
        restored.persistent_switch = false;
        restored.restore(&switch.saved_state());
        assert_eq!(restored.state.switched, 1);
        assert!(!restored.pose_at_end());
    }

    fn native_mechanism(kind: u16) -> Mechanism {
        let mut mechanism = mechanism(kind);
        mechanism.state.native = Some(NativeState {
            active: true,
            rotation: 180.,
            rotation_target: None,
            movement: None,
            completed: false,
        });
        mechanism
    }

    #[test]
    fn plate_model_animation_is_blocking_and_inactive_actions_do_not_advance() {
        let mut plate = mechanism(29);
        plate.state.action = Some(MechanismAction {
            phase: ActionPhase::Idle,
            player: None,
            position: plate.position,
            origin: plate.position,
            target: plate.position,
            elapsed: 0.,
            animation_complete: false,
        });
        assert_eq!(plate.activate(true), Activation::Ignored);
        assert_eq!(plate.activate(false), Activation::Interact);
        plate.begin_plate("ROLE_0".into());
        plate.finish_plate_approach();
        plate.set_active(false);
        assert!(!plate.update(10., 2.));
        assert_eq!(plate.animation_time(), Some(0.));
        plate.set_active(true);
        assert!(!plate.update(0.5, 2.));
        assert_eq!(plate.state.height, 10.);
        assert_eq!(plate.animation_time(), Some(1.));
        assert!(!plate.update(0.5, 2.));
        assert_eq!(plate.state.height, 10.);
        assert!(!plate.update(1.25, 2.));
        assert!((plate.state.height - 6.5).abs() < 0.0001);
        assert!(plate.update(1.25, 2.));
        assert_eq!(plate.state.height, 3.);
        assert!(!plate.action_locked());
        assert_eq!(plate.activate(false), Activation::Ignored);
        assert_eq!(plate.state.remaining, 1);
    }

    #[test]
    fn quarter_turn_uses_native_speed_and_only_consumes_accepted_events() {
        for kind in [7, 8] {
            let mut rotating = native_mechanism(kind);
            assert_eq!(rotating.activate(false), Activation::Ignored);
            assert_eq!(rotating.activate(true), Activation::Started);
            assert_eq!(rotating.state.remaining, 0);
            assert_eq!(rotating.state.switched, 0);
            assert_eq!(rotating.activate(true), Activation::Ignored);
            assert!(!rotating.update(1., 0.));
            assert_eq!(rotating.state.native.as_ref().unwrap().rotation, 210.);
            assert!(rotating.update(5., 0.));
            assert_eq!(rotating.state.native.as_ref().unwrap().rotation, 270.);
            assert!(!rotating.update(5., 0.));
        }
    }

    #[test]
    fn unlimited_quarter_turn_restores_inflight_target_and_respects_object_active() {
        let mut rotating = native_mechanism(7);
        rotating.state.remaining = 255;
        rotating.activate(true);
        rotating.update(1., 0.);
        let mut restored = native_mechanism(7);
        restored.restore(&rotating.saved_state());
        restored.set_active(false);
        assert!(!restored.update(10., 0.));
        assert_eq!(restored.state.native.as_ref().unwrap().rotation, 210.);
        restored.set_active(true);
        assert!(restored.update(2., 0.));
        assert!(restored.provides_floor());
        assert_eq!(restored.activate(true), Activation::Started);
        assert!(!restored.provides_floor());
        assert!(restored.update(3., 0.));
        assert_eq!(restored.state.native.as_ref().unwrap().rotation, 360.);
        assert_eq!(restored.state.remaining, 255);
    }

    #[test]
    fn gate_eases_to_absolute_height_then_deactivates_without_toggling_or_spending() {
        let mut gate = native_mechanism(31);
        assert_eq!(gate.activate(true), Activation::Started);
        assert_eq!(gate.activation_proc(), Some(500));
        assert_eq!(gate.completion_proc(), None);
        assert_eq!(gate.state.remaining, 1);
        let duration = (40. - 10.) * 0.25;
        assert!(!gate.update(duration * 0.25, 0.));
        let expected = 10. + 30. * (1. - std::f32::consts::FRAC_1_SQRT_2) * 0.5;
        assert!((gate.state.height - expected).abs() < 0.0001);
        assert!(gate.update(duration, 0.));
        assert_eq!(gate.state.height, 40.);
        assert_eq!(gate.state.switched, 0);
        assert!(!gate.active);
        assert_eq!(gate.activate(true), Activation::Ignored);
        assert!(!gate.update(10., 0.));

        let mut gate = native_mechanism(31);
        gate.target_height = 10.;
        gate.activate(true);
        assert!(gate.update(0., 0.));
        assert_eq!(gate.state.height, 10.);
    }
}
