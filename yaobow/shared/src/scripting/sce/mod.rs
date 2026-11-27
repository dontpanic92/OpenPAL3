use std::{
    any::Any,
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crosscom::ComRc;
use imgui::Ui;
use radiance::{
    audio::AudioEngine,
    comdef::{IEntityExt, ISceneExt, ISceneManager},
    input::InputEngine,
    math::{Mat44, Vec3},
    radiance::UiManager,
};
use radiance_scripting::UiManagerImmediateExt;

use crate::openpal3::{
    asset_manager::AssetManager,
    comdef::{IPal3DialogRenderer, IPal3StatusRenderer},
    directors::SceneManagerExtensions,
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
    fn blocks_script(&self) -> bool {
        false
    }

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

#[cfg(test)]
mod native_feedback_tests {
    use super::*;

    #[test]
    fn damage_number_uses_native_anchor_motion_and_horizontal_offset() {
        let projected = project_damage_number(
            &Mat44::new_identity(),
            Vec3::new(0., -64., 0.),
            0.,
            800.,
            600.,
        )
        .unwrap();
        assert_eq!(projected, (360., 300.));

        let risen = project_damage_number(
            &Mat44::new_identity(),
            Vec3::new(0., -64., 0.),
            0.5,
            800.,
            600.,
        )
        .unwrap();
        assert_eq!(risen, (360., -2700.));
    }

    #[test]
    fn damage_number_fades_for_native_two_and_a_half_seconds() {
        assert_eq!(damage_number_alpha(0.), 1.);
        assert!((damage_number_alpha(1.5) - 0.4).abs() < f32::EPSILON);
        assert_eq!(damage_number_alpha(2.5), 0.);
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
    menu_state: ComRc<crate::openpal3::comdef::IPal3MenuState>,
    mechanism_notice: Option<(String, f32)>,
    reward_notices: Vec<RewardNotice>,
    hazard_damage_feedback: Vec<HazardDamageFeedback>,
}

#[derive(Clone, Debug)]
struct RewardNotice {
    text: String,
    elapsed: f32,
    row: i32,
}

#[derive(Clone, Debug)]
struct HazardDamageFeedback {
    delta: i32,
    elapsed: f32,
}

fn project_damage_number(
    view_projection: &Mat44,
    role_position: Vec3,
    elapsed: f32,
    width: f32,
    height: f32,
) -> Option<(f32, f32)> {
    let position = Vec3::new(
        role_position.x,
        role_position.y + 64. + elapsed * 20.,
        role_position.z,
    );
    let m = view_projection.floats();
    let clip_x = m[0][0] * position.x + m[0][1] * position.y + m[0][2] * position.z + m[0][3];
    let clip_y = m[1][0] * position.x + m[1][1] * position.y + m[1][2] * position.z + m[1][3];
    let clip_w = m[3][0] * position.x + m[3][1] * position.y + m[3][2] * position.z + m[3][3];
    if !clip_w.is_finite() || clip_w <= 0. {
        return None;
    }

    let ndc_x = clip_x / clip_w;
    let ndc_y = clip_y / clip_w;
    Some(((ndc_x * 0.5 + 0.45) * width, (1. - ndc_y) * 0.5 * height))
}

fn damage_number_alpha(elapsed: f32) -> f32 {
    (1. - elapsed * 0.4).clamp(0., 1.)
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
        let menu_state = global_state.menu_state();

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
            menu_state,
            mechanism_notice: None,
            reward_notices: vec![],
            hazard_damage_feedback: vec![],
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

    pub fn advance_dialog_page(&self) -> bool {
        self.dialog_renderer.advance_page()
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

    pub fn render_loading(&self, progress: f32, delta_sec: f32) -> bool {
        self.ui
            .with_ui_host(|ui_host| {
                self.status_renderer
                    .render_loading(ui_host.clone(), progress, delta_sec)
            })
            .unwrap_or(true)
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
    pub fn render_status(&mut self, delta_sec: f32, scene_manager: ComRc<ISceneManager>) {
        let renderer = self.status_renderer.clone();
        let menu = self.menu_state.clone();
        let menu = menu.inner::<crate::openpal3::menu_state::Pal3MenuState>();
        let role_position = scene_manager
            .get_resolved_role(self, -1)
            .map(|role| role.transform().borrow().position());
        let return_proc = crate::openpal3::menu_state::EARTH_PEARL_PROC;
        let can_call_return = self.asset_mgr.game() == crate::GameType::PAL3
            && !self.context.is_running()
            && self.context.has_proc(return_proc);
        if let Some(scene) = scene_manager.scn_scene() {
            let scene = scene.inner::<crate::openpal3::scene::ScnScene>();
            menu.set_earth_pearl_context(scene.scene_type(), scene.name(), can_call_return);
            let bee_proc = if !self.context.is_running() {
                self.context
                    .proc_id_by_name(&format!("_ylf_{}_{}", scene.name(), scene.sub_name()))
            } else {
                None
            };
            let has_bee = scene.scene_type() == 2 && bee_proc.is_some() && scene.prepare_bee();
            menu.set_bee_context(scene.scene_type(), bee_proc, has_bee);
            menu.set_treasure_mouse_context(
                !self.context.is_running()
                    && role_position.is_some()
                    && scene.prepare_treasure_mouse(),
            );
        } else {
            menu.set_earth_pearl_context(0, "", false);
            menu.set_bee_context(0, None, false);
            menu.set_treasure_mouse_context(false);
        }
        let view_projection = scene_manager.scene().map(|scene| {
            let camera = scene.camera();
            Mat44::multiplied(
                camera.projection_matrix(),
                &Mat44::inversed(&camera.transform().matrix()),
            )
        });
        self.ui.with_ui_host(|ui_host| {
            renderer.render_status(ui_host.clone(), delta_sec, self.menu_state.clone());
            if let Some((text, _)) = &self.mechanism_notice {
                renderer.render_notice(ui_host.clone(), text);
            }
            for notice in &self.reward_notices {
                renderer.render_reward_notice(
                    ui_host.clone(),
                    &notice.text,
                    notice.elapsed,
                    notice.row,
                );
            }
            if let (Some(position), Some(view_projection)) = (role_position, view_projection) {
                let width = ui_host.display_size_x() as f32;
                let height = ui_host.display_size_y() as f32;
                for feedback in &self.hazard_damage_feedback {
                    if let Some((x, y)) = project_damage_number(
                        &view_projection,
                        position,
                        feedback.elapsed,
                        width,
                        height,
                    ) {
                        ui_host.text_at_size(
                            x,
                            y,
                            32.,
                            20. / 255.,
                            20. / 255.,
                            230. / 255.,
                            damage_number_alpha(feedback.elapsed),
                            &feedback.delta.to_string(),
                        );
                    }
                }
            }
        });
        if menu.take_earth_pearl_request() {
            self.call_proc(return_proc);
            self.global_state.set_adv_input_enabled(false);
            renderer.set_menu_open(false);
        }
        if let Some(proc_id) = menu.take_bee_request() {
            self.call_proc(proc_id);
            self.global_state.set_adv_input_enabled(false);
            renderer.set_menu_open(false);
        }
        if menu.take_treasure_mouse_request() {
            if let (Some(scene), Some(position)) = (scene_manager.scn_scene(), role_position) {
                scene
                    .inner::<crate::openpal3::scene::ScnScene>()
                    .launch_treasure_mouse(position);
            }
            renderer.set_menu_open(false);
        }
        let sounds = self
            .menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .take_menu_sounds();
        for name in sounds {
            match self.asset_mgr.load_snd_data(name) {
                Ok(data) => {
                    let mut source = self.global_state.create_sound_source();
                    source.set_data(data, radiance::audio::Codec::Wav);
                    source.play(false);
                    self.global_state
                        .add_sound_source(Rc::new(RefCell::new(source)));
                }
                Err(error) => log::warn!("Cannot load PAL3 item-menu sound {name}: {error}"),
            }
        }
        let elapsed = delta_sec.max(0.);
        if let Some((_, remaining)) = &mut self.mechanism_notice {
            *remaining -= elapsed;
            if *remaining <= 0. {
                self.mechanism_notice = None;
            }
        }
        for feedback in &mut self.hazard_damage_feedback {
            feedback.elapsed += elapsed;
        }
        self.hazard_damage_feedback
            .retain(|feedback| feedback.elapsed < 2.5);
        for notice in &mut self.reward_notices {
            notice.elapsed += elapsed;
        }
        self.reward_notices.retain(|notice| notice.elapsed < 4.);
    }

    pub fn show_mechanism_notice(&mut self, text: String) {
        self.mechanism_notice = Some((text, 4.));
    }

    pub fn show_reward_notices(&mut self, notices: Vec<String>) {
        let first_row = self
            .reward_notices
            .last()
            .map_or(0, |notice| notice.row + 1);
        self.reward_notices
            .extend(
                notices
                    .into_iter()
                    .enumerate()
                    .map(|(index, text)| RewardNotice {
                        text,
                        elapsed: 0.,
                        row: first_row + index as i32,
                    }),
            );
    }

    pub fn show_hazard_damage(&mut self, delta: i32) {
        if delta < 0 {
            self.hazard_damage_feedback
                .push(HazardDamageFeedback { delta, elapsed: 0. });
        }
    }

    /// The scripted status indicator/menu renderer. Exposed so the agent
    /// server can open/close the in-game character-status (状态) menu
    /// without forging a synthetic avatar click.
    pub fn status_renderer(&self) -> ComRc<crate::openpal3::comdef::IPal3StatusRenderer> {
        self.status_renderer.clone()
    }

    pub fn take_menu_load(&self) -> Option<i32> {
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .take_requested_load()
    }

    #[cfg(not(target_os = "vita"))]
    pub fn install_menu_preview(
        &self,
        cache: Rc<RefCell<radiance_scripting::services::texture_cache::ImguiTextureCache>>,
    ) {
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .install_thumbnail_uploader(radiance_scripting::services::sprite::SpriteUploader::new(
                &cache.borrow(),
            ));
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .install_preview(crate::openpal3::menu_preview::MenuPreview::new(
                self.asset_mgr.clone(),
                cache,
            ));
    }

    pub fn render_menu_preview(&self, engine: &mut dyn radiance::rendering::RenderingEngine) {
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .render_preview(self.status_renderer.is_menu_open(), engine);
    }

    pub(crate) fn capture_save_thumbnail(
        &self,
        engine: &mut dyn radiance::rendering::RenderingEngine,
    ) {
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .capture_save_thumbnail(engine);
    }

    pub fn take_menu_exit(&self) -> bool {
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .take_requested_return_to_menu()
    }

    pub fn report_menu_load_failed(&self) {
        self.menu_state
            .inner::<crate::openpal3::menu_state::Pal3MenuState>()
            .report_load_failed();
    }
}
