//! Active preview sessions registry + orbit camera state.
//!
//! Each open model tab has at most one `PreviewState`: a private scene
//! holding the loaded entity, an offscreen render target the imgui tab
//! body samples, and orbit-camera state driven by mouse input through
//! `IPreviewSession::tick_camera`. The host's `render_pending_previews`
//! walks the registry every frame and asks the rendering engine to draw
//! each live state's scene into its target before the imgui pass runs.
//!
//! `PreviewRegistry` holds `Weak` refs so dropping the script-side
//! `IPreviewSession` ComRc tears down the session naturally — no
//! explicit unregister step needed.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crosscom::ComRc;
use radiance::comdef::IScene;
use radiance::math::Vec3;
use radiance::rendering::{RenderTarget as EngineRenderTarget, RenderingEngine};

/// Per-session orbit-camera state. Distances are world units; angles
/// radians.
pub struct OrbitState {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl OrbitState {
    pub fn new(focus: Vec3, yaw: f32, pitch: f32, distance: f32) -> Self {
        Self {
            focus,
            yaw,
            pitch,
            distance,
        }
    }

    /// Resulting world-space camera position.
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(
            self.focus.x + self.distance * cp * sy,
            self.focus.y + self.distance * sp,
            self.focus.z + self.distance * cp * cy,
        )
    }
}

pub struct PreviewState {
    pub scene: ComRc<IScene>,
    /// Shared with the matching `ScriptedRenderTarget` ComRc so script
    /// resizes and engine renders see the same backing object.
    pub target: Rc<RefCell<Box<dyn EngineRenderTarget>>>,
    pub orbit: RefCell<OrbitState>,
    /// Set by `IPreviewSession::close` to short-circuit further renders;
    /// the registry skips dead states on the next walk.
    pub closed: Cell<bool>,
}

impl PreviewState {
    pub fn apply_camera(&self) {
        let orbit = self.orbit.borrow();
        let eye = orbit.eye();
        let camera = self.scene.camera();
        let mut camera = camera.borrow_mut();
        // Keep the projection's aspect ratio synchronized with the
        // target's current pixel extent so resizing the editor tab
        // doesn't squish the rendered model.
        let (tw, th) = self.target.borrow().extent();
        if tw > 0 && th > 0 {
            camera.set_aspect(tw as f32 / th as f32);
        }
        camera.transform_mut().set_position(&eye).look_at(&orbit.focus);
    }
}

/// Host-owned registry of live preview sessions.
#[derive(Default)]
pub struct PreviewRegistry {
    sessions: RefCell<Vec<Weak<PreviewState>>>,
}

impl PreviewRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, state: &Rc<PreviewState>) {
        self.sessions.borrow_mut().push(Rc::downgrade(state));
    }

    /// Drive each live state's offscreen pass via the rendering engine.
    /// Compacts the internal Vec by dropping dead weak refs.
    pub fn render_all(&self, engine: &mut dyn RenderingEngine) {
        let mut survivors: Vec<Weak<PreviewState>> = Vec::new();
        let snapshot: Vec<Weak<PreviewState>> = self.sessions.borrow().clone();
        for weak in snapshot {
            let Some(state) = weak.upgrade() else {
                continue;
            };
            if state.closed.get() {
                continue;
            }
            state.apply_camera();
            let mut target = state.target.borrow_mut();
            engine.render_scene_to_target(state.scene.clone(), target.as_mut());
            // Keep tracking — strong refs elsewhere keep it alive.
            survivors.push(Rc::downgrade(&state));
        }
        *self.sessions.borrow_mut() = survivors;
    }
}

/// Project orbit input from a single frame onto an `OrbitState`.
///
/// `buttons` bits: 0 = left (orbit), 1 = right (pan), 2 = middle.
/// `dx`/`dy` are pointer-pixel deltas (positive right / down).
/// `wheel` is positive when zooming in.
pub fn tick_orbit(state: &mut OrbitState, dx: f32, dy: f32, wheel: f32, buttons: i32) {
    // Tuneable: ~0.5deg per pixel for orbit, 0.1 world unit per pixel for
    // pan at default distance, 10% distance per wheel unit. All multiplied
    // by current distance for pan so far-away cameras pan faster.
    const ORBIT_RAD_PER_PIXEL: f32 = 0.005;
    const PAN_PER_PIXEL: f32 = 0.001;
    const ZOOM_FACTOR_PER_WHEEL: f32 = 0.9;
    // Clamp pitch just under +/-90deg to avoid gimbal flip.
    const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

    if buttons & 0b001 != 0 {
        state.yaw -= dx * ORBIT_RAD_PER_PIXEL;
        state.pitch = (state.pitch + dy * ORBIT_RAD_PER_PIXEL).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }
    if buttons & 0b010 != 0 {
        // Pan: move focus in the camera's right/up plane. Build a basis
        // from current yaw/pitch and translate `focus` accordingly. The
        // pan speed scales with distance so a far-away camera pans at
        // roughly the same on-screen rate.
        let (sy, cy) = state.yaw.sin_cos();
        let (sp, cp) = state.pitch.sin_cos();
        // Forward (from eye towards focus): -(cp*sy, sp, cp*cy)
        let fwd = Vec3::new(-cp * sy, -sp, -cp * cy);
        // World-up = (0,1,0); right = cross(fwd, up) ; up' = cross(right, fwd)
        let up = Vec3::new(0., 1., 0.);
        let right = Vec3::normalized(&Vec3::cross(&fwd, &up));
        let cam_up = Vec3::normalized(&Vec3::cross(&right, &fwd));
        let scale = PAN_PER_PIXEL * state.distance.max(1.0);
        state.focus = Vec3::new(
            state.focus.x - right.x * dx * scale + cam_up.x * dy * scale,
            state.focus.y - right.y * dx * scale + cam_up.y * dy * scale,
            state.focus.z - right.z * dx * scale + cam_up.z * dy * scale,
        );
    }
    if wheel.abs() > 0.0 {
        // Multiplicative zoom keeps the response stable across very
        // close and very far framings.
        let factor = ZOOM_FACTOR_PER_WHEEL.powf(wheel);
        state.distance = (state.distance * factor).max(0.1);
    }
}
