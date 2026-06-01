//! Synthetic input overlay used by the agent surface.
//!
//! [`SyntheticInputBridge`] wraps an inner [`InputEngine`] and ORs its
//! reported key / axis state with a programmatically-driven shadow set.
//! It is intentionally **not** plumbed into [`InputEngineInternal`]
//! (`update`/`as_input_engine` stay on the concrete `CoreInputEngine`).
//! Instead, consumers that want synthetic input replace the
//! `Rc<RefCell<dyn InputEngine>>` they receive from
//! [`crate::radiance::CoreRadianceEngine::input_engine`] with the
//! wrapped one. The original engine keeps owning real keyboard /
//! mouse / gamepad polling; this layer only adds an overlay.
//!
//! Tick semantics: synthetic `pressed` / `released` are *edge*
//! signals lasting exactly one frame. The bridge's [`Self::end_frame`]
//! must be called once per game tick (the PAL4 agent session does
//! this from the director's `update`) so that taps appear pressed for
//! a single frame and then drop back to the natural `is_down` state.

use std::cell::RefCell;
use std::rc::Rc;

use super::{Axis, AxisState, InputEngine, Key, KeyState, MouseButton};

/// Per-key shadow record set by the agent. Mirrors the same edge
/// information [`CoreInputEngine`](super::CoreInputEngine) maintains
/// internally.
#[derive(Clone, Copy, Debug, Default)]
struct SyntheticKey {
    /// `true` between an explicit `down` and the matching `up`.
    held: bool,
    /// Set by `tap` / `down`; cleared by [`SyntheticInputBridge::end_frame`].
    pressed: bool,
    /// Set by `up` / `tap`; cleared by [`SyntheticInputBridge::end_frame`].
    released: bool,
    /// `true` if this slot was modified before the next `end_frame`;
    /// drives the `OR`-with-inner merge.
    dirty: bool,
}

/// Wrapper that ORs synthetic input on top of an inner [`InputEngine`].
pub struct SyntheticInputBridge {
    inner: Rc<RefCell<dyn InputEngine>>,
    keys: RefCell<Vec<SyntheticKey>>,
    axes: RefCell<Vec<Option<f32>>>,
}

impl SyntheticInputBridge {
    /// Build a bridge around `inner`. The returned `Rc<RefCell<...>>`
    /// is suitable for any caller that expects a
    /// `Rc<RefCell<dyn InputEngine>>`.
    pub fn install(inner: Rc<RefCell<dyn InputEngine>>) -> Rc<RefCell<dyn InputEngine>> {
        Rc::new(RefCell::new(Self::new(inner))) as Rc<RefCell<dyn InputEngine>>
    }

    pub fn new(inner: Rc<RefCell<dyn InputEngine>>) -> Self {
        Self {
            inner,
            keys: RefCell::new(vec![SyntheticKey::default(); Key::Unknown as usize + 1]),
            axes: RefCell::new(vec![None; Axis::Unknown as usize + 1]),
        }
    }

    /// Begin a held key. Subsequent `get_key_state` calls report
    /// `is_down = true` and `pressed = true` for this frame.
    pub fn press_down(&self, key: Key) {
        let mut keys = self.keys.borrow_mut();
        let slot = &mut keys[key as usize];
        if !slot.held {
            slot.pressed = true;
        }
        slot.held = true;
        slot.dirty = true;
    }

    /// End a held key. Reports `released = true` for the next
    /// `end_frame`-bounded frame.
    pub fn release(&self, key: Key) {
        let mut keys = self.keys.borrow_mut();
        let slot = &mut keys[key as usize];
        if slot.held {
            slot.released = true;
        }
        slot.held = false;
        slot.dirty = true;
    }

    /// One-frame tap: appears `pressed + released + is_down` for the
    /// current frame, naturally back to `up` next frame.
    pub fn tap(&self, key: Key) {
        let mut keys = self.keys.borrow_mut();
        let slot = &mut keys[key as usize];
        slot.held = true; // appears down for this frame
        slot.pressed = true;
        slot.released = true;
        slot.dirty = true;
        // No flag tells us to drop `held` on `end_frame`; we want it
        // to release naturally next frame, so flip back here:
        //
        // (deferred to end_frame via the released flag — see below)
        slot.dirty = true;
    }

    /// Push an axis value. Overrides whatever the inner engine reports
    /// until [`Self::clear_axis`] is called.
    pub fn set_axis(&self, axis: Axis, value: f32) {
        let mut axes = self.axes.borrow_mut();
        axes[axis as usize] = Some(value.clamp(-1.0, 1.0));
    }

    /// Drop the synthetic override for `axis`; the inner engine's value
    /// is reported again.
    pub fn clear_axis(&self, axis: Axis) {
        let mut axes = self.axes.borrow_mut();
        axes[axis as usize] = None;
    }

    /// Roll one frame: clears single-frame `pressed` / `released`
    /// flags and turns tap edges into natural releases (`held = false`).
    pub fn end_frame(&self) {
        let mut keys = self.keys.borrow_mut();
        for slot in keys.iter_mut() {
            // A tap sets `pressed` + `released` in the same frame.
            // After end_frame the key should appear fully up.
            if slot.pressed && slot.released {
                slot.held = false;
            }
            slot.pressed = false;
            slot.released = false;
            if !slot.held {
                slot.dirty = false;
            }
        }
    }
}

impl InputEngine for SyntheticInputBridge {
    fn get_key_state(&self, key: Key) -> KeyState {
        let inner = self.inner.borrow().get_key_state(key);
        let keys = self.keys.borrow();
        let synth = keys[key as usize];

        if !synth.dirty && !synth.pressed && !synth.released {
            return inner;
        }

        KeyState::new(
            inner.is_down() || synth.held,
            inner.pressed() || synth.pressed,
            inner.released() || synth.released,
        )
    }

    fn get_axis_state(&self, axis: Axis) -> AxisState {
        let axes = self.axes.borrow();
        if let Some(v) = axes[axis as usize] {
            let mut state = AxisState::new();
            state.set_value(v);
            return state;
        }
        self.inner.borrow().get_axis_state(axis)
    }

    fn get_mouse_button_state(&self, button: MouseButton) -> KeyState {
        self.inner.borrow().get_mouse_button_state(button)
    }

    fn get_mouse_delta(&self) -> (f32, f32) {
        self.inner.borrow().get_mouse_delta()
    }

    fn get_mouse_wheel(&self) -> f32 {
        self.inner.borrow().get_mouse_wheel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal stub for the inner engine.
    struct StubEngine;
    impl InputEngine for StubEngine {
        fn get_key_state(&self, _key: Key) -> KeyState {
            KeyState::new(false, false, false)
        }
        fn get_axis_state(&self, _axis: Axis) -> AxisState {
            let mut s = AxisState::new();
            s.set_value(0.25);
            s
        }
    }

    fn make() -> SyntheticInputBridge {
        SyntheticInputBridge::new(Rc::new(RefCell::new(StubEngine)) as Rc<RefCell<dyn InputEngine>>)
    }

    #[test]
    fn press_down_then_release_reports_held_then_released() {
        let b = make();
        b.press_down(Key::F);
        let s = b.get_key_state(Key::F);
        assert!(s.is_down());
        assert!(s.pressed());

        b.end_frame();
        let s = b.get_key_state(Key::F);
        assert!(s.is_down(), "held key stays down across end_frame");
        assert!(!s.pressed());

        b.release(Key::F);
        let s = b.get_key_state(Key::F);
        assert!(s.released());
        b.end_frame();
        let s = b.get_key_state(Key::F);
        assert!(!s.is_down());
        assert!(!s.released());
    }

    #[test]
    fn tap_reports_one_frame_only() {
        let b = make();
        b.tap(Key::Space);
        let s = b.get_key_state(Key::Space);
        assert!(s.is_down() && s.pressed() && s.released());
        b.end_frame();
        let s = b.get_key_state(Key::Space);
        assert!(!s.is_down() && !s.pressed() && !s.released());
    }

    #[test]
    fn axis_override_takes_precedence_over_inner() {
        let b = make();
        assert_eq!(b.get_axis_state(Axis::LeftStickX).value(), 0.25);
        b.set_axis(Axis::LeftStickX, -1.0);
        assert_eq!(b.get_axis_state(Axis::LeftStickX).value(), -1.0);
        b.clear_axis(Axis::LeftStickX);
        assert_eq!(b.get_axis_state(Axis::LeftStickX).value(), 0.25);
    }
}
