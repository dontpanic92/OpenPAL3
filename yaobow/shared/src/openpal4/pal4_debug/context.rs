//! Host-side ComObject backing `IPal4DebugContext`.
//!
//! The owning [`OpenPAL4Director`](crate::openpal4::director::OpenPAL4Director)
//! refreshes the snapshot once per `render` before invoking the
//! script overlay. The `Pal4DebugContext` ComObject delegates every IDL
//! getter to the shared [`Pal4DebugState`], so the host keeps a typed
//! Rust handle for snapshot writes while the script sees a plain
//! `box<IPal4DebugContext>`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crosscom::ComRc;

use crate::openpal4::comdef::pal4_debug::{IPal4DebugContext, IPal4DebugContextImpl};

/// Frame-local snapshot copied from `Pal4VmContext` before each
/// script `render` call. Plain data — no engine handles — so the
/// script-visible context object stays cheap to update and is not
/// invalidated by scene transitions.
#[derive(Clone, Debug, Default)]
pub struct Pal4DebugSnapshot {
    pub scene_name: String,
    pub block_name: String,
    pub leader_index: i32,
    pub leader_pos: [f32; 3],
    pub delta_time: f32,
    pub fps: f32,
}

/// PAL4-debug-session state shared between the typed Rust handle
/// (used by the director's `render` to push per-frame data) and
/// the COM wrapper that the script sees as a
/// `box<IPal4DebugContext>`.
pub struct Pal4DebugState {
    scene_name: RefCell<String>,
    block_name: RefCell<String>,
    leader_index: Cell<i32>,
    leader_pos: Cell<[f32; 3]>,
    delta_time: Cell<f32>,
    fps: Cell<f32>,
    bsp_visible: Cell<bool>,
    nav_mesh_visible: Cell<bool>,
    fast_forward: Cell<bool>,
}

impl Pal4DebugState {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn set_snapshot(&self, snap: Pal4DebugSnapshot) {
        *self.scene_name.borrow_mut() = snap.scene_name;
        *self.block_name.borrow_mut() = snap.block_name;
        self.leader_index.set(snap.leader_index);
        self.leader_pos.set(snap.leader_pos);
        self.delta_time.set(snap.delta_time);
        self.fps.set(snap.fps);
    }

    /// `&self`-safe accessor for the BSP-visibility toggle. Read by
    /// the director each frame so it can fan the flag out to
    /// `Pal4VmContext::set_bsp_visible`.
    pub fn bsp_visible(&self) -> bool {
        self.bsp_visible.get()
    }

    /// Same idea as [`Pal4DebugState::bsp_visible`], for the floor +
    /// wall (nav-mesh) overlay geometry.
    pub fn nav_mesh_visible(&self) -> bool {
        self.nav_mesh_visible.get()
    }

    /// `&self`-safe accessor for the plot fast-forward toggle. Read by
    /// the director each frame so it can fan the flag out to
    /// `Pal4VmContext::set_fast_forward`.
    pub fn fast_forward(&self) -> bool {
        self.fast_forward.get()
    }
}

impl Default for Pal4DebugState {
    fn default() -> Self {
        Self {
            scene_name: RefCell::new(String::new()),
            block_name: RefCell::new(String::new()),
            leader_index: Cell::new(0),
            leader_pos: Cell::new([0.0; 3]),
            delta_time: Cell::new(0.0),
            fps: Cell::new(0.0),
            // BSP renders by default (matches pre-toggle behaviour);
            // nav-mesh is hidden until the developer flips it on.
            bsp_visible: Cell::new(true),
            nav_mesh_visible: Cell::new(false),
            // Fast-forward is opt-in: the plot plays at normal speed
            // until the developer flips it on.
            fast_forward: Cell::new(false),
        }
    }
}

/// COM wrapper: every interface call delegates to the shared inner.
pub struct Pal4DebugContext {
    state: Rc<Pal4DebugState>,
}

ComObject_Pal4DebugContext!(super::Pal4DebugContext);

impl Pal4DebugContext {
    pub fn new(state: Rc<Pal4DebugState>) -> Self {
        Self { state }
    }
}

impl IPal4DebugContextImpl for Pal4DebugContext {
    fn scene_name(&self) -> &str {
        // SAFETY: the generated FFI thunk copies the returned bytes into
        // a freshly-allocated C buffer before this scope unwinds, so
        // extending the `Ref<String>` borrow to `&self` is sound for the
        // duration of the synchronous getter return path. The
        // `RefCell` is only mutated by `set_snapshot`, which is never
        // re-entered from inside a getter call.
        let r = self.state.scene_name.borrow();
        unsafe { std::mem::transmute::<&str, &str>(r.as_str()) }
    }

    fn block_name(&self) -> &str {
        let r = self.state.block_name.borrow();
        unsafe { std::mem::transmute::<&str, &str>(r.as_str()) }
    }

    fn leader_index(&self) -> std::os::raw::c_int {
        self.state.leader_index.get()
    }

    fn leader_pos_x(&self) -> f32 {
        self.state.leader_pos.get()[0]
    }

    fn leader_pos_y(&self) -> f32 {
        self.state.leader_pos.get()[1]
    }

    fn leader_pos_z(&self) -> f32 {
        self.state.leader_pos.get()[2]
    }

    fn delta_time(&self) -> f32 {
        self.state.delta_time.get()
    }

    fn fps(&self) -> f32 {
        self.state.fps.get()
    }

    fn bsp_visible(&self) -> bool {
        self.state.bsp_visible.get()
    }

    fn set_bsp_visible(&self, v: bool) {
        self.state.bsp_visible.set(v);
    }

    fn nav_mesh_visible(&self) -> bool {
        self.state.nav_mesh_visible.get()
    }

    fn set_nav_mesh_visible(&self, v: bool) {
        self.state.nav_mesh_visible.set(v);
    }

    fn fast_forward(&self) -> bool {
        self.state.fast_forward.get()
    }

    fn set_fast_forward(&self, v: bool) {
        self.state.fast_forward.set(v);
    }
}

/// PAL4 debug overlay session boundary: native code updates `state`,
/// script sees `context`.
pub struct Pal4DebugSession {
    pub state: Rc<Pal4DebugState>,
    pub context: ComRc<IPal4DebugContext>,
}

pub fn create_debug_session() -> Pal4DebugSession {
    let state = Pal4DebugState::new();
    let context = ComRc::from_object(Pal4DebugContext::new(state.clone()));
    Pal4DebugSession { state, context }
}
