use std::{
    any::Any,
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::{
    audio::AudioEngine, comdef::ISceneManager, input::InputEngine, radiance::UiManager,
};
use radiance_scripting::UiManagerImmediateExt;

use crate::openpal3::{
    asset_manager::AssetManager,
    comdef::{IPal3DialogRenderer, IPal3StatusRenderer},
    loaders::sce_loader::SceFile,
    states::global_state::GlobalState,
};

use self::vm::{SceExecutionContext, SceExecutionOptions};

pub mod commands;
pub mod vm;

/// The scripted PAL3 dialog-box renderer, threaded from `Pal3Service`
/// into the SCE command loop. The renderer is self-contained (it owns its
/// sprites, avatar and curtain state in p7); the host only forwards SCE
/// events and the per-frame draw calls. The imgui texture cache the draws
/// resolve through is engine-owned (reached via `UiManager::with_ui_host`),
/// so the renderer is passed straight through (no wrapper struct).

pub trait SceCommandDebug {
    fn debug(&self) -> String;
}

pub trait SceCommand: dyn_clone::DynClone + SceCommandDebug {
    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, _state: &mut SceState) {}

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool;
}

dyn_clone::clone_trait_object!(SceCommand);

impl<T: SceCommand + std::fmt::Debug> SceCommandDebug for T {
    fn debug(&self) -> String {
        format!("{:?}", self)
    }
}

pub struct SceState {
    asset_mgr: Rc<AssetManager>,
    global_state: GlobalState,
    context: SceExecutionContext,
    run_mode: i32,
    curtain: f32,
    /// Agent-server fast-forward flag, refreshed once per frame from
    /// [`AgentBridge::fast_forward`](crate::agent_common::AgentBridge)
    /// by `AdventureDirector` before each `SceVm::update`. SCE commands
    /// read it via [`SceState::fast_forward`] to skip input-blocked
    /// waits (dialog / movie) and collapse timed tweens to their final
    /// state in a single frame. Defaults to `false` (real-time).
    fast_forward: bool,
    ext: HashMap<String, Box<dyn Any>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    audio_engine: Rc<dyn AudioEngine>,

    // Scripted PAL3 dialog box: the SCE command loop forwards events to
    // the self-contained p7 `dialog_renderer` and drives its draw calls
    // via `UiManager::with_ui_host` — radiance's immediate-mode UI
    // composition primitive (the peer of the retained `UiLayerStack`;
    // both compose onto the same engine imgui frame through one shared
    // engine-owned texture cache). The dialog uses the immediate path
    // rather than a registered `IUiLayer` because the `dlg` SCE command
    // reads input and emits its draw in the *same* update tick (see
    // `commands/dlg.rs`), which a pre-update retained layer can't express
    // without a separate visibility state machine.
    ui: Rc<UiManager>,
    dialog_renderer: ComRc<IPal3DialogRenderer>,
    // Scripted top-right status indicator; drawn each frame the player
    // has control (no SCE running). Same immediate-mode draw path as the
    // dialog renderer.
    status_renderer: ComRc<crate::openpal3::comdef::IPal3StatusRenderer>,
}

impl SceState {
    pub fn new(
        input_engine: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        ui: Rc<UiManager>,
        asset_mgr: Rc<AssetManager>,
        sce: Rc<SceFile>,
        sce_name: String,
        global_state: GlobalState,
        options: Option<SceExecutionOptions>,
        dialog_renderer: ComRc<IPal3DialogRenderer>,
        status_renderer: ComRc<IPal3StatusRenderer>,
    ) -> Self {
        let ext = HashMap::<String, Box<dyn Any>>::new();

        Self {
            asset_mgr: asset_mgr.clone(),
            global_state,
            context: SceExecutionContext::new(sce, sce_name, options),
            run_mode: 1,
            curtain: 1.,
            fast_forward: false,
            ext,
            input_engine,
            audio_engine,
            ui,
            dialog_renderer,
            status_renderer,
        }
    }

    pub fn global_state(&self) -> &GlobalState {
        &self.global_state
    }

    pub fn global_state_mut(&mut self) -> &mut GlobalState {
        &mut self.global_state
    }

    pub fn context_mut(&mut self) -> &mut SceExecutionContext {
        &mut self.context
    }

    pub fn context(&self) -> &SceExecutionContext {
        &self.context
    }

    pub fn input(&self) -> Ref<'_, dyn InputEngine> {
        self.input_engine.borrow()
    }

    pub fn audio_engine(&self) -> &Rc<dyn AudioEngine> {
        &self.audio_engine
    }

    pub fn run_mode(&self) -> i32 {
        self.run_mode
    }

    pub fn set_run_mode(&mut self, run_mode: i32) {
        self.run_mode = run_mode;
    }

    /// Whether the agent-server fast-forward mode is active this frame.
    /// SCE commands consult this to skip input-blocked waits (dialog /
    /// movie) and to collapse timed tweens to their final state.
    pub fn fast_forward(&self) -> bool {
        self.fast_forward
    }

    /// Set by `AdventureDirector` once per frame from the agent bridge.
    pub fn set_fast_forward(&mut self, fast_forward: bool) {
        self.fast_forward = fast_forward;
    }

    pub fn curtain(&self) -> f32 {
        self.curtain
    }

    pub fn set_curtain(&mut self, curtain: f32) {
        let curtain = if curtain > 1. {
            1.
        } else if curtain < -1. {
            -1.
        } else {
            curtain
        };
        self.curtain = curtain;
    }

    pub fn ext_mut(&mut self) -> &mut HashMap<String, Box<dyn Any>> {
        &mut self.ext
    }

    pub fn asset_mgr(&self) -> &Rc<AssetManager> {
        &self.asset_mgr
    }

    pub fn get_next_cmd(&mut self) -> Option<Box<dyn SceCommand>> {
        self.context.get_next_cmd(&mut self.global_state)
    }

    pub fn call_proc(&mut self, proc_id: u32) {
        self.context.call_proc(proc_id, &mut self.global_state);
    }

    pub fn try_call_proc_by_name(&mut self, proc_name: &str) {
        self.context
            .try_call_proc_by_name(proc_name, &mut self.global_state);
    }

    /// Compose the scripted dialog panel for this frame: drive the p7
    /// `render_dialog` with the markup-bearing `text` inside a mid-frame
    /// imgui scope. Mirrors the legacy `DialogBox::draw`.
    pub fn render_dialog(&mut self, text: &str, delta_sec: f32) {
        let renderer = self.dialog_renderer.clone();
        self.ui.with_ui_host(|ui_host| {
            renderer.render_dialog(ui_host.clone(), text, delta_sec);
        });
    }

    /// Paint the scene-transition curtain/fade for this frame from the
    /// current `curtain` value. Mirrors the legacy
    /// `DialogBox::fade_window` driven by `SceVm::draw_curtain`.
    pub fn render_curtain(&mut self) {
        let curtain = self.curtain;
        self.dialog_renderer.set_curtain(curtain);
        let renderer = self.dialog_renderer.clone();
        self.ui.with_ui_host(|ui_host| {
            renderer.render_curtain(ui_host.clone());
        });
    }

    /// Set the dialog avatar/portrait (PAL3 `dlgface`). The p7 renderer
    /// loads the portrait sprite; called outside any `with_ui_host`
    /// scope so the texture-cache upload doesn't re-enter a held borrow.
    pub fn set_dialog_avatar(&self, role: &str, face: &str, side: i32) {
        self.dialog_renderer.set_avatar(role, face, side);
    }

    /// Clear the dialog avatar/portrait (end of a `dlg` beat).
    pub fn clear_dialog_avatar(&self) {
        self.dialog_renderer.clear_avatar();
    }

    /// Compose the top-right status indicator for this frame. Called only
    /// when the player has control (no SCE proc running).
    pub fn render_status(&mut self, delta_sec: f32) {
        let renderer = self.status_renderer.clone();
        self.ui.with_ui_host(|ui_host| {
            renderer.render_status(ui_host.clone(), delta_sec);
        });
    }
}
