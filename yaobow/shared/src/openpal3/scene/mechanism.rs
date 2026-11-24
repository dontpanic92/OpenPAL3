//! Clean-room PAL3 mechanism rules, traced in the supplied original executable.
//! SCE 124: 0x438350; SCE 126: 0x438390; dispatcher: 0x424c20.
//! Type 23: 0x425b25..0x425c86, completion/link: 0x429396..0x42953d.
//! Type 22: 0x4259d9..0x425a88, motion: 0x428a66..0x428ba7 (30 units/s).
//! Other native object types are deliberately not approximated here.
//! Player approach/action phases, platform collision/navigation and rider transfer,
//! and the switch's special water/effect parameters are not implemented yet.

use serde::{Deserialize, Serialize};

use crate::openpal3::loaders::scn_loader::ScnNode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanismState {
    pub remaining: u8,
    pub switched: u8,
    pub height: f32,
    pub raised: bool,
    pub motion: Option<f32>,
    pub animation_time: Option<f32>,
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
}

#[derive(Debug, PartialEq)]
pub enum Activation {
    Ignored,
    Script(u32),
    Started,
}

impl Mechanism {
    pub fn from_node(node: &ScnNode) -> Option<Self> {
        if !matches!(node.node_type, 22 | 23) {
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
            },
            target: node.w148,
            proc_id: node.sce_proc_id,
            mode: node.ladder_nav_coord1.1,
            persistent_switch: node.ladder_nav_coord1.0 != 0,
            target_height: node.ladder_nav_coord1.0 as f32,
        })
    }

    pub fn restore(&mut self, state: &MechanismState) {
        self.state = state.clone();
    }

    pub fn restore_raised(&mut self) {
        self.state.height = self.target_height;
        self.state.raised = true;
    }

    pub fn activate(&mut self, scripted: bool) -> Activation {
        // Explicit Trigger ignores visibility/active, but not the use counter.
        if self.state.remaining == 0 || (!scripted && !self.active) {
            return Activation::Ignored;
        }
        if self.kind == 23 {
            if !scripted {
                match self.mode {
                    0 => {}
                    2 if self.proc_id != 0 => return Activation::Script(self.proc_id),
                    _ => return Activation::Ignored,
                }
            }
            if self.state.animation_time.is_some() {
                return Activation::Ignored;
            }
            self.state.switched = u8::from(self.state.switched == 0);
            self.state.animation_time = Some(0.);
        } else {
            if !scripted || self.state.motion.is_some() {
                return Activation::Ignored;
            }
            self.state.motion = Some(if self.state.raised {
                self.position.y
            } else {
                self.target_height
            });
        }
        if self.state.remaining != u8::MAX {
            self.state.remaining -= 1;
        }
        Activation::Started
    }

    pub fn animation_time(&self) -> Option<f32> {
        self.state.animation_time
    }

    pub fn pose_at_end(&self) -> bool {
        self.state.remaining == 0 || (self.persistent_switch && self.state.switched != 0)
    }

    /// Returns true once when a mechanism completes, allowing its link/proc to run.
    pub fn update(&mut self, delta_sec: f32, animation_duration: f32) -> bool {
        let delta_sec = delta_sec.max(0.);
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
                return true;
            }
        }
        false
    }

    pub fn completion_proc(&self) -> Option<u32> {
        (self.proc_id != 0).then_some(self.proc_id)
    }

    pub fn saved_state(&self) -> MechanismState {
        self.state.clone()
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
            },
            target: 3,
            proc_id: 500,
            mode: 0,
            persistent_switch: true,
            target_height: 40.,
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
}
