//! PAL4 launch service — phase-2 replacement for `Pal4LaunchContext`.
//!
//! App-lifetime singleton installed by `YaobowApplicationLoader` and
//! exposed via `IYaobowHostContext.pal4()`. `create_director(path)`
//! returns a fully-wired `IDirector` (= `OpenPAL4Director`) with:
//!  * its AssetLoader bound to the per-launch vfs
//!  * the agent bridge attached (if the loader pre-stocked one via
//!    [`Pal4Service::set_agent_bridge`])
//!  * the debug-overlay bundle attached (assembled here around the
//!    script-built overlay from [`Pal4Service::set_script_factory`])
//!  * the actor controller factory attached (the same script factory)
//!
//! ## Ownership graph
//!
//! `Pal4Service` is held by `YaobowHostContext`, which is interned into
//! the script via `foreign_box`. The single `script_factory`
//! (`ComRc<IPal4ScriptFactory>`, QI'd from the reverse-wrapped app
//! root) is the script's PAL4 factory surface. Holding it forms a
//! strong cycle (`Pal4Service` → script CCW → script box → host context
//! → `Pal4Service`); `YaobowApplicationLoader::on_unloading` calls
//! [`Pal4Service::clear_script_factory`] to break it at teardown.
//!
//! ## Agent server lifetime
//!
//! `Pal4Service` only holds the [`Pal4AgentBridge`] handle. The
//! companion `AgentServer` HTTP listener lives on
//! `YaobowApplicationLoader` (Rust-side, RefCell<Option<AgentServer>>)
//! for the app lifetime — joined deterministically at process exit.
//! This separation was flagged as load-bearing by phase-1's
//! rubber-duck (finding A).

use std::cell::RefCell;
use std::rc::Rc;

use agent_server::protocol::{
    AgentCommand, AgentError, AgentResponse, AxisInputParams, KeyAction, KeyInputParams,
    PerfMetric, PerfMetricsResponse, ScreenshotResponse, StateSnapshot, StepTimeParams,
};
use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::audio::Codec;
use radiance::comdef::{IApplication, IApplicationExt, IDirector, IScene};
use radiance::input::{Axis, InputEngine, Key, SyntheticInputBridge};
use radiance_scripting::comdef::services::{IAudioSource, IUiLayoutHandle};
use radiance_scripting::services::ImguiTextureCache;
use radiance_scripting::services::audio::AudioSource as ScriptAudioSource;

use crate::loaders::cegui::layout as cegui_layout;
use crate::loaders::cegui::ui_layout_handle::UiLayoutHandle;
use crate::openpal4::agent::Pal4AgentBridge;
use crate::openpal4::asset_loader::AssetLoader;
use crate::openpal4::comdef::{
    IOpenPAL4Director, IPal4ScriptFactory, IPal4Service, IPal4ServiceImpl,
};
use crate::openpal4::director::{OpenPAL4Director, Pal4DebugBundle};
use crate::openpal4::modes::{
    self, Pal4ModeFactory, Pal4ModeIntent, Pal4ModeKind, Pal4ModeRegistry,
};
use crate::openpal4::pal4_debug::create_debug_session;
use crate::openpal4::session::Pal4Session;
use crate::openpal4::states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState};
use common::store_ext::StoreExt2;

/// PAL4 save namespace lives in
/// `openpal4::states::persistent_state::PAL4_APP_NAME` so the service
/// and `Pal4VmContext` agree on the slot directory.

pub struct Pal4Service {
    app: ComRc<IApplication>,
    agent_bridge: RefCell<Option<Rc<Pal4AgentBridge>>>,
    /// The script app's PAL4 factory (`make_actor_controller`,
    /// `make_pal4_start_menu`, `make_pal4_debug_overlay`). Set after
    /// construction by `YaobowApplicationLoader` (QI'd from the
    /// reverse-wrapped app factory) because the script root itself
    /// depends on the host context (which holds this service) existing
    /// first. Cloned into each constructed story director for actor
    /// controllers; called directly for the start menu + debug overlay.
    script_factory: RefCell<Option<ComRc<IPal4ScriptFactory>>>,

    /// App-lifetime `AssetLoader` for PAL4. Built lazily by
    /// `loader_for` on the first `create_director` (mounting the vfs and
    /// picking the real vs. agent-synthetic input engine), then reused
    /// for the whole process: the single asset front-door for menu-time
    /// loads (`open_layout`, `load_menu_scene`, `load_music`) and handed
    /// straight to the story director by `build_story_director`. The
    /// PAL4 asset path is invariant for a launch (the menu captures it
    /// once and passes the same string to every story-director factory),
    /// and `AssetLoader` is immutable after construction, so a single
    /// instance is mounted once and never remounted.
    launch_loader: RefCell<Option<Rc<AssetLoader>>>,

    /// Imgui texture cache used by `open_layout` to upload imageset
    /// atlases. Set once at boot by `YaobowApplicationLoader` after
    /// `install_imgui_pump` returns the cache. `open_layout` returns
    /// `None` when this slot is empty (e.g. on a build target with
    /// no imgui pump).
    texture_cache: RefCell<Option<Rc<RefCell<ImguiTextureCache>>>>,

    /// Reusable scratch buffer backing `save_slot_summary`'s `&str`
    /// return. The accessor refreshes it then returns a pointer into
    /// it; codegen copies into a CString immediately.
    summary_scratch: RefCell<String>,

    /// The PAL4 mode-factory registry — the single extension point for
    /// the game-mode graph. Pre-populated with the built-in start-menu
    /// and story factories; `route` dispatches every
    /// [`Pal4ModeIntent`](crate::openpal4::modes::Pal4ModeIntent)
    /// through it. Future modes (e.g. battle) register a factory here
    /// instead of editing the router. Wrapped in `RefCell` so
    /// [`Pal4Service::register_mode`] can extend it after construction.
    mode_registry: RefCell<Pal4ModeRegistry>,

    /// App-lifetime owner of the playthrough [`Pal4Session`], shared by
    /// `Rc` handle. Cloned into every story director's context (so the
    /// session survives mode switches) and reachable from app-lifetime
    /// code such as the agent dispatcher. `reset_session` starts a fresh
    /// playthrough (New Game / return to title).
    session: Rc<RefCell<Pal4Session>>,

    /// The PAL4 asset path captured on the first `loader_for`. Lets the
    /// app-lifetime mode-control commands (`/v1/menu/new_game` etc.)
    /// rebuild the story director without the agent supplying a path.
    launch_asset_path: RefCell<Option<String>>,
}

ComObject_Pal4Service!(super::Pal4Service);

impl Pal4Service {
    /// App-lifetime install. All slots are late-bound via setters
    /// because their construction depends on the host context (which
    /// holds this service) existing first, or on the imgui pump
    /// being installed.
    pub fn create(app: ComRc<IApplication>) -> ComRc<IPal4Service> {
        ComRc::from_object(Self {
            app,
            agent_bridge: RefCell::new(None),
            script_factory: RefCell::new(None),
            launch_loader: RefCell::new(None),
            texture_cache: RefCell::new(None),
            summary_scratch: RefCell::new(String::new()),
            mode_registry: RefCell::new(Pal4ModeRegistry::with_builtins()),
            session: Rc::new(RefCell::new(Pal4Session::new())),
            launch_asset_path: RefCell::new(None),
        })
    }

    /// Clone the shared session handle. Used by `build_story_director`
    /// to seed the director's context, and (Phase 8) by the app-lifetime
    /// agent dispatcher to read/write session state without a director.
    pub fn session_handle(&self) -> Rc<RefCell<Pal4Session>> {
        self.session.clone()
    }

    /// Start a fresh playthrough — replace the shared session in place
    /// so every existing handle observes the reset. Called when a new
    /// playthrough begins (New Game) or the player returns to title.
    pub fn reset_session(&self) {
        *self.session.borrow_mut() = Pal4Session::new();
    }

    /// Install the imgui texture cache so `open_layout` can upload
    /// imageset atlases. Called once at boot by
    /// `YaobowApplicationLoader::on_loading` with the cache returned
    /// by `install_imgui_pump`. Idempotent — replaces a previous
    /// cache (in tests / hot-reload scenarios).
    pub fn set_texture_cache(&self, cache: Rc<RefCell<ImguiTextureCache>>) {
        *self.texture_cache.borrow_mut() = Some(cache);
    }

    /// Install the agent bridge. Called by `YaobowApplicationLoader`
    /// during `on_loading` when the binary was started with
    /// `--pal4 --agent-port`. Idempotent — re-installation replaces
    /// the previous bridge; a warn is logged when this happens since
    /// double-install would indicate a sequencing bug.
    pub fn set_agent_bridge(&self, bridge: Rc<Pal4AgentBridge>) {
        let mut slot = self.agent_bridge.borrow_mut();
        if slot.is_some() {
            log::warn!("Pal4Service::set_agent_bridge called twice; replacing previous bridge");
        }
        *slot = Some(bridge);
    }

    /// Install the script app's PAL4 factory (QI'd from the
    /// reverse-wrapped app root). Called by
    /// `YaobowApplicationLoader::on_loading` after the script root is
    /// installed. Used for the start menu, debug overlay, and actor
    /// controllers.
    pub fn set_script_factory(&self, factory: ComRc<IPal4ScriptFactory>) {
        *self.script_factory.borrow_mut() = Some(factory);
    }

    /// Drop the held script factory, breaking the
    /// service↔host-context↔script-CCW reference cycle at teardown.
    /// Called by `YaobowApplicationLoader::on_unloading`.
    pub fn clear_script_factory(&self) {
        *self.script_factory.borrow_mut() = None;
    }

    /// Assemble a fresh PAL4 debug bundle: a Rust-side debug session
    /// (context + state) plus the script-built overlay against it.
    /// Returns `None` when no script factory is installed.
    fn build_debug_bundle(&self) -> Option<Pal4DebugBundle> {
        let factory = self.script_factory.borrow().clone()?;
        let session = create_debug_session();
        let overlay = factory.make_pal4_debug_overlay(session.context.clone());
        Some(Pal4DebugBundle {
            overlay,
            overlay_ctx: session.context,
            debug_state: session.state,
        })
    }

    /// Returns the app-lifetime PAL4 `AssetLoader`, mounting + caching
    /// it on first use. Every subsequent call returns the same cached
    /// `Rc<AssetLoader>` regardless of `asset_path` (which is invariant
    /// across a PAL4 launch).
    ///
    /// This is the single asset front-door for a PAL4 launch: the
    /// start menu reads through it (`open_layout`, `load_menu_scene`,
    /// `load_music`) and the story director receives the very same
    /// `Rc<AssetLoader>` — no second mount, no ownership handoff.
    ///
    /// The loader's input engine is the agent's `SyntheticInputBridge`
    /// when an agent bridge is installed (so `/v1/input/*` commands are
    /// observable by every consumer), otherwise the real engine input.
    /// The agent bridge is installed at boot, before any
    /// `create_director`, so this choice is stable across a launch.
    fn loader_for(&self, asset_path: &str) -> Rc<AssetLoader> {
        if let Some(loader) = self.launch_loader.borrow().as_ref() {
            return loader.clone();
        }

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let component_factory = engine.rendering_component_factory();
        let real_input = engine.input_engine();
        drop(engine);

        let input_engine: Rc<RefCell<dyn InputEngine>> = match self.agent_bridge.borrow().as_ref() {
            Some(bridge) => bridge.input_bridge.clone(),
            None => real_input,
        };

        let vfs = init_virtual_fs(asset_path, None);
        let loader = AssetLoader::new(component_factory, input_engine, vfs);
        *self.launch_loader.borrow_mut() = Some(loader.clone());
        // Remember the launch asset path so app-lifetime mode-control
        // commands (`/v1/menu/new_game` etc.) can rebuild the story
        // director without the agent having to supply a path.
        *self.launch_asset_path.borrow_mut() = Some(asset_path.to_string());
        loader
    }

    /// App-lifetime **single agent dispatcher**. Driven once per frame
    /// by `YaobowApplicationLoader::on_updating` (which runs *before*
    /// the active director's `update`), it is the **sole drainer** of
    /// the agent command queue in every mode.
    ///
    /// Routing: it looks up the currently-installed director from the
    /// `SceneManager` and, if it is an `OpenPAL4Director` (story mode,
    /// detected via `query_interface::<IOpenPAL4Director>()`), delegates
    /// each envelope to the director's full, VM-backed
    /// [`OpenPAL4Director::handle_agent_envelope`] — including the
    /// cross-frame `fire_trigger { wait_until_idle }` settle, which the
    /// director still replies to from its post-VM `poll_pending_fires`.
    /// Otherwise (start menu / title / any non-story mode) it answers
    /// the mode-agnostic subset itself (`GetState` with a minimal "no
    /// active playthrough" snapshot, `Screenshot` of the 3D backdrop)
    /// and rejects gameplay commands with a clear `not_implemented`.
    ///
    /// In story mode the per-frame agent bookkeeping (frame counter /
    /// fps / synthetic-input edges) is done by the director's
    /// `end_agent_frame`; in non-story modes this method does it.
    pub fn pump_agent(&self, delta_sec: f32) {
        let bridge = match self.agent_bridge.borrow().as_ref() {
            Some(b) => b.clone(),
            None => return,
        };

        // Lazily wire the rendering engine so `/v1/screenshot` works
        // before any story director has set it on the bridge.
        if bridge.rendering_engine.borrow().is_none() {
            let engine = self.app.engine().borrow().rendering_engine();
            bridge.set_rendering_engine(engine);
        }

        // Drain the queue once (single drainer for all modes).
        let mut envelopes = Vec::new();
        if let Some(consumer) = bridge.consumer.borrow().as_ref() {
            consumer.drain(|env| envelopes.push(env));
        }

        let scene_manager = self.app.engine().borrow().scene_manager().clone();

        for env in envelopes {
            if Self::is_bridge_command(&env.command) {
                // App-lifetime / bridge-only commands (input, time
                // control, screenshot, perf, log) are handled here in
                // *both* modes — they never need the VM, so there is one
                // implementation and no menu/story asymmetry.
                let response = self.dispatch_bridge_command(&bridge, env.command.clone());
                env.reply(response);
            } else if Self::is_session_command(&env.command) {
                // Session-only commands (dialog / world-map choice
                // buffering) write straight to the shared session via
                // interior mutability — no director or VM required.
                // This lets an agent pre-stage a choice even if no
                // story director is currently active (e.g. queuing
                // the first dialog reply while the start-menu is up).
                let response = self.dispatch_session_command(env.command.clone());
                env.reply(response);
            } else if Self::is_mode_control_command(&env.command) {
                // Mode-control commands (new_game / load / exit) install
                // the next director via the `SceneManager`. Handled here
                // because the dispatcher — not any director — owns the
                // mode graph; lets the agent drive transitions directly.
                let response = self.dispatch_mode_control(&scene_manager, env.command.clone());
                env.reply(response);
            } else if let Some(story) = Self::active_story_director(&scene_manager) {
                // VM / scene commands: delegate to the active story
                // director (it owns the reply, including the deferred
                // `fire_trigger { wait_until_idle }` settle). Resolved
                // fresh per command so a preceding mode-control command
                // in the same batch is observed.
                story.inner::<OpenPAL4Director>().handle_agent_envelope(env);
            } else {
                // VM / scene command with no active playthrough: answer
                // the menu subset (`GetState` minimal) or reject.
                let response = Self::dispatch_menu_command(&bridge, env.command.clone());
                env.reply(response);
            }
        }

        // Per-frame agent telemetry (frame counter + dt/fps) is generic
        // and published here once per frame for **every** mode.
        bridge.publish_frame_telemetry(delta_sec);

        // The synthetic-input edge clear must run *after* the active
        // mode polls input for the frame. In story mode the director's
        // `end_agent_frame` does it (after its VM tick, which happens in
        // `engine.update`, i.e. after this `on_updating`); here we do it
        // only when no story director is installed (re-resolved, since a
        // mode-control command this frame may have just installed one).
        if Self::active_story_director(&scene_manager).is_none() {
            bridge.input_bridge.borrow().end_frame();
        }
    }

    /// Resolve the currently-installed director and downcast to the
    /// concrete story director, or `None` when the active mode is not a
    /// story director (start menu / title / none).
    fn active_story_director(
        scene_manager: &ComRc<radiance::comdef::ISceneManager>,
    ) -> Option<ComRc<IOpenPAL4Director>> {
        scene_manager
            .director()
            .and_then(|d| d.query_interface::<IOpenPAL4Director>())
    }

    /// Whether `command` drives a mode transition (handled by the
    /// dispatcher via the `SceneManager`, in any mode).
    fn is_mode_control_command(command: &AgentCommand) -> bool {
        matches!(
            command,
            AgentCommand::EnterNewGame | AgentCommand::EnterLoadGame(_) | AgentCommand::ExitGame
        )
    }

    /// Install the next director (or quit) for a mode-control command.
    /// Uses the launch asset path captured by `loader_for`, so the agent
    /// supplies no path. `set_director` runs the outgoing director's
    /// `deactivate` synchronously; the dispatcher holds no engine borrow
    /// here (the `scene_manager` handle was cloned out earlier).
    fn dispatch_mode_control(
        &self,
        scene_manager: &ComRc<radiance::comdef::ISceneManager>,
        command: AgentCommand,
    ) -> AgentResponse {
        if let AgentCommand::ExitGame = command {
            self.app.request_exit();
            return AgentResponse::Ok;
        }

        let asset_path = match self.launch_asset_path.borrow().clone() {
            Some(p) => p,
            None => {
                return AgentResponse::err(AgentError::internal(
                    "PAL4 launch asset path not set yet (no playthrough has been mounted)",
                ));
            }
        };

        let intent = match command {
            AgentCommand::EnterNewGame => Pal4ModeIntent::Story { asset_path },
            AgentCommand::EnterLoadGame(params) => Pal4ModeIntent::StoryFromSave {
                asset_path,
                slot: params.slot,
            },
            _ => {
                return AgentResponse::err(AgentError::internal(
                    "dispatch_mode_control called with a non-mode-control command",
                ));
            }
        };

        let director = modes::route(self, intent);
        scene_manager.set_director(director);
        AgentResponse::Ok
    }

    /// Commands that need only the app-lifetime [`Pal4AgentBridge`] (or
    /// global state) and never the VM / scene — handled by the
    /// dispatcher itself in every mode.
    fn is_bridge_command(command: &AgentCommand) -> bool {
        matches!(
            command,
            AgentCommand::KeyInput(_)
                | AgentCommand::AxisInput(_)
                | AgentCommand::PauseTime
                | AgentCommand::ResumeTime
                | AgentCommand::StepTime(_)
                | AgentCommand::Screenshot
                | AgentCommand::LogTail(_)
                | AgentCommand::GetPerfMetrics
        )
    }

    /// Commands that write into the shared playthrough session
    /// (`Pal4SessionTransient`) via interior mutability — no VM or
    /// director needed.
    fn is_session_command(command: &AgentCommand) -> bool {
        matches!(
            command,
            AgentCommand::ChooseDialog(_) | AgentCommand::ChooseWorldMap(_)
        )
    }

    /// Switchboard for [`Self::is_session_command`]. Writes go
    /// directly to `self.session` (shared `Rc<RefCell>`); the next
    /// active story director observes them through the same handle.
    fn dispatch_session_command(&self, command: AgentCommand) -> AgentResponse {
        match command {
            AgentCommand::ChooseDialog(params) => {
                self.session.borrow().buffer_dialog_choice(params.index);
                AgentResponse::Ok
            }
            AgentCommand::ChooseWorldMap(params) => {
                self.session
                    .borrow()
                    .buffer_world_map_choice(params.scene, params.block);
                AgentResponse::Ok
            }
            _ => unreachable!("dispatch_session_command called with non-session command"),
        }
    }

    /// Switchboard for the [`Self::is_bridge_command`] set. These all
    /// operate purely on the agent bridge (synthetic input, pause/step
    /// cells, rendering engine) or global `radiance::perf`, so they live
    /// here rather than in the story director.
    fn dispatch_bridge_command(
        &self,
        bridge: &Pal4AgentBridge,
        command: AgentCommand,
    ) -> AgentResponse {
        match command {
            AgentCommand::KeyInput(params) => Self::handle_key_input(bridge, params),
            AgentCommand::AxisInput(params) => Self::handle_axis_input(bridge, params),
            AgentCommand::PauseTime => Self::handle_pause(bridge, true),
            AgentCommand::ResumeTime => Self::handle_pause(bridge, false),
            AgentCommand::StepTime(params) => Self::handle_step(bridge, params),
            AgentCommand::Screenshot => Self::dispatch_screenshot(bridge),
            AgentCommand::GetPerfMetrics => Self::handle_get_perf_metrics(),
            AgentCommand::LogTail(_) => AgentResponse::err(AgentError::internal(
                "log_tail must not be queued; served by transport",
            )),
            _ => AgentResponse::err(AgentError::internal(
                "dispatch_bridge_command called with a non-bridge command",
            )),
        }
    }

    /// Menu / non-story switchboard for VM-needing commands: answers the
    /// mode-agnostic `GetState` with a minimal "no active playthrough"
    /// snapshot and rejects everything else.
    fn dispatch_menu_command(bridge: &Pal4AgentBridge, command: AgentCommand) -> AgentResponse {
        match command {
            AgentCommand::GetState => AgentResponse::State(StateSnapshot {
                frame: bridge.frame.get(),
                paused: bridge.paused.get(),
                fps: bridge.fps_display.get(),
                dt: bridge.dt_display.get(),
                script_running: false,
                movie_playing: false,
                ..Default::default()
            }),
            _ => AgentResponse::err(AgentError::not_implemented(
                "command not available outside story mode (no active PAL4 playthrough)",
            )),
        }
    }

    fn handle_key_input(bridge: &Pal4AgentBridge, params: KeyInputParams) -> AgentResponse {
        let Some(key) = Key::from_name(&params.key) else {
            return AgentResponse::err(AgentError::bad_request(format!(
                "unknown key name: {}",
                params.key
            )));
        };
        let synthetic = bridge.input_bridge.borrow();
        match params.action {
            KeyAction::Down => synthetic.press_down(key),
            KeyAction::Up => synthetic.release(key),
            KeyAction::Tap => synthetic.tap(key),
        }
        AgentResponse::Ok
    }

    fn handle_axis_input(bridge: &Pal4AgentBridge, params: AxisInputParams) -> AgentResponse {
        let Some(axis) = Axis::from_name(&params.axis) else {
            return AgentResponse::err(AgentError::bad_request(format!(
                "unknown axis name: {}",
                params.axis
            )));
        };
        bridge.input_bridge.borrow().set_axis(axis, params.value);
        AgentResponse::Ok
    }

    fn handle_pause(bridge: &Pal4AgentBridge, paused: bool) -> AgentResponse {
        bridge.paused.set(paused);
        if !paused {
            // Resuming clears any leftover step budget so the game
            // doesn't double-tick.
            bridge.requested_steps.set(0);
        }
        AgentResponse::Ok
    }

    fn handle_step(bridge: &Pal4AgentBridge, params: StepTimeParams) -> AgentResponse {
        if !bridge.paused.get() {
            return AgentResponse::err(AgentError::conflict(
                "must pause time before requesting fixed-step frames",
            ));
        }
        if params.frames == 0 {
            return AgentResponse::Ok;
        }
        bridge
            .requested_steps
            .set(bridge.requested_steps.get().saturating_add(params.frames));
        bridge.requested_dt.set(params.dt.unwrap_or(0.0).max(0.0));
        AgentResponse::Ok
    }

    fn handle_get_perf_metrics() -> AgentResponse {
        // `radiance::perf::snapshot()` reads the thread_local metrics
        // registry, so this must run on the game thread — which is where
        // the dispatcher pumps. When perf is disabled at boot the
        // snapshot is empty; the response's `enabled` field lets the
        // agent distinguish that from "enabled but nothing recorded yet".
        let entries = radiance::perf::snapshot();
        let metrics = entries
            .into_iter()
            .map(|(name, snapshot)| match snapshot {
                radiance::perf::MetricSnapshot::Timing {
                    calls,
                    avg_ns,
                    max_ns,
                } => PerfMetric::Timing {
                    name: name.to_string(),
                    calls,
                    avg_ns,
                    max_ns,
                },
                radiance::perf::MetricSnapshot::Counter { frame, total } => PerfMetric::Counter {
                    name: name.to_string(),
                    frame,
                    total,
                },
                radiance::perf::MetricSnapshot::Gauge { last, max } => PerfMetric::Gauge {
                    name: name.to_string(),
                    last,
                    max,
                },
            })
            .collect();
        AgentResponse::PerfMetrics(PerfMetricsResponse {
            enabled: radiance::perf::enabled(),
            metrics,
        })
    }

    fn dispatch_screenshot(bridge: &Pal4AgentBridge) -> AgentResponse {
        let engine = match bridge.rendering_engine.borrow().clone() {
            Some(e) => e,
            None => return AgentResponse::Screenshot(ScreenshotResponse::default()),
        };
        match engine.borrow_mut().capture_last_frame() {
            Some(frame) => AgentResponse::Screenshot(ScreenshotResponse {
                width: frame.width,
                height: frame.height,
                encoded: true,
                rgba: frame.rgba,
            }),
            None => AgentResponse::Screenshot(ScreenshotResponse::default()),
        }
    }
}

impl IPal4ServiceImpl for Pal4Service {
    fn create_director(&self, asset_path: &str) -> ComRc<IDirector> {
        modes::route(
            self,
            Pal4ModeIntent::StartMenu {
                asset_path: asset_path.to_string(),
            },
        )
    }

    fn enter_new_game(&self, asset_path: &str) -> ComRc<IDirector> {
        modes::route(
            self,
            Pal4ModeIntent::Story {
                asset_path: asset_path.to_string(),
            },
        )
    }

    fn enter_load_game(&self, asset_path: &str, slot: i32) -> ComRc<IDirector> {
        modes::route(
            self,
            Pal4ModeIntent::StoryFromSave {
                asset_path: asset_path.to_string(),
                slot,
            },
        )
    }

    fn save_slot_count(&self) -> i32 {
        Pal4PersistentState::SLOT_COUNT
    }

    fn save_slot_exists(&self, slot: i32) -> bool {
        Pal4PersistentState::peek(PAL4_APP_NAME, slot).is_some()
    }

    fn save_slot_summary(&self, slot: i32) -> &str {
        // Full, display-ready row label for the load screen's slot
        // list. ASCII only — the PAL4 menu imgui font has no CJK
        // glyphs. Populated slots show their scene/quest summary;
        // empty slots are explicitly marked so the row still renders.
        let label = match Pal4PersistentState::peek(PAL4_APP_NAME, slot) {
            Some(state) => format!("Slot {} - {}", slot, state.summary()),
            None => format!("Slot {} - (empty)", slot),
        };
        *self.summary_scratch.borrow_mut() = label;
        unsafe { (*self.summary_scratch.as_ptr()).as_str() }
    }
    fn open_layout(&self, vfs_path: &str) -> Option<ComRc<IUiLayoutHandle>> {
        let cache = self.texture_cache.borrow().clone()?;
        let loader = self.launch_loader.borrow().as_ref()?.clone();
        let vfs = loader.vfs();
        // Sanity-check: the file should look like a CEGUI layout
        // before we incur the (cheap) parse + atlas upload cost.
        if let Ok(bytes) = vfs.read_to_end(vfs_path) {
            if !cegui_layout::looks_like_gui_layout(&bytes) {
                log::warn!(
                    "Pal4Service::open_layout({vfs_path}): file does not look like <GUILayout>"
                );
                return None;
            }
        }
        UiLayoutHandle::try_create(vfs, vfs_path, cache)
    }

    fn load_menu_scene(&self) -> Option<ComRc<IScene>> {
        let loader = self.launch_loader.borrow().as_ref()?.clone();
        loader.load_menu_scene()
    }

    fn load_music(&self, music_name: &str) -> Option<ComRc<IAudioSource>> {
        // Delegate the `/gamedata/Music/<name>.smp` read + XXTEA
        // decrypt to the single asset front-door so the path / decode
        // lives once (the in-game BGM path uses the same
        // `AssetLoader::load_music`).
        let loader = self.launch_loader.borrow().as_ref()?.clone();
        let decrypted = match loader.load_music(music_name) {
            Ok(d) => d,
            Err(err) => {
                log::warn!("Pal4Service::load_music({music_name}): {err:#}");
                return None;
            }
        };

        let engine_rc = self.app.engine();
        let audio_engine = engine_rc.borrow().audio_engine();
        drop(engine_rc);

        let mut source = audio_engine.create_source();
        source.set_data(decrypted, Codec::Mp3);
        Some(ScriptAudioSource::create(source))
    }
}

impl Pal4Service {
    /// Register (or replace) the director factory for `kind` in the
    /// mode registry. The boot-time extension hook for new PAL4 game
    /// modes (e.g. a future battle director): call this once instead of
    /// editing the router or adding a bespoke service method.
    pub fn register_mode(&self, kind: Pal4ModeKind, factory: Pal4ModeFactory) {
        self.mode_registry.borrow_mut().register(kind, factory);
    }

    /// Dispatch a [`Pal4ModeIntent`] through the mode registry to build
    /// the concrete director. Called by [`modes::route`]. Falls back to
    /// a fresh story director if the intent's kind has no registered
    /// factory — which can only happen if the built-ins were
    /// deliberately replaced, so it is logged loudly.
    pub(crate) fn build_mode(&self, intent: Pal4ModeIntent) -> ComRc<IDirector> {
        let asset_path = match &intent {
            Pal4ModeIntent::StartMenu { asset_path }
            | Pal4ModeIntent::Story { asset_path }
            | Pal4ModeIntent::StoryFromSave { asset_path, .. } => asset_path.clone(),
        };
        let kind = intent.kind();
        match self.mode_registry.borrow().build(self, intent) {
            Some(director) => director,
            None => {
                log::warn!(
                    "Pal4Service::build_mode: no factory registered for {:?}; \
                     falling back to a fresh story director",
                    kind
                );
                ComRc::<IDirector>::from_object(self.build_story_director(&asset_path))
            }
        }
    }

    /// Build the scripted PAL4 start-menu director. Mounts the
    /// per-launch asset loader first so the script-side menu can call
    /// `host.pal4().open_layout("/gamedata/ui/...")`, then asks the
    /// script project's `make_pal4_start_menu` hook to build it. Falls
    /// back to a fresh story director when the scripted menu can't be
    /// built (e.g. PAL4 assets missing at this path). Called by the
    /// mode router for [`Pal4ModeIntent::StartMenu`].
    pub(crate) fn build_start_menu(&self, asset_path: &str) -> ComRc<IDirector> {
        // Mount the per-launch asset loader now so the script-side
        // menu can call `host.pal4().open_layout("/gamedata/ui/...")`.
        let _ = self.loader_for(asset_path);

        let factory = self.script_factory.borrow().clone().expect(
            "Pal4Service::build_start_menu called before the script factory was installed \
             (or after it was cleared). YaobowApplicationLoader must call \
             Pal4Service::set_script_factory after installing the script root.",
        );

        // The menu struct conforms to both `IImmediateDirector` and
        // `IDirector`; QI the immediate variant to the director the
        // scene manager expects. On a null/failed menu the fat CCW
        // still QIs, but we fall back to a story director defensively.
        let menu = factory.make_pal4_start_menu(asset_path);
        match menu.query_interface::<IDirector>() {
            Some(director) => director,
            None => {
                log::warn!(
                    "Pal4Service::build_start_menu: scripted start menu did not expose IDirector \
                     for {}; falling back to story director",
                    asset_path
                );
                ComRc::<IDirector>::from_object(self.build_story_director(asset_path))
            }
        }
    }

    /// Construct (but do not wrap) the full PAL4 story director: asset
    /// loader, AngelScript VM, agent bridge, debug bundle, and actor
    /// controller factory. Called by the mode router for
    /// [`Pal4ModeIntent::Story`] / [`Pal4ModeIntent::StoryFromSave`];
    /// the latter sets a pending load slot on the returned director
    /// before wrapping it.
    pub(crate) fn build_story_director(&self, asset_path: &str) -> OpenPAL4Director {
        // Every menu → story entry begins a fresh playthrough: reset the
        // shared session so a New Game starts clean (and a Load Game then
        // overwrites it via the director's pending-load on first update).
        // Without this, a second playthrough in the same process would
        // inherit the previous one's session state.
        self.reset_session();

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();

        let component_factory = engine.rendering_component_factory();
        let real_input = engine.input_engine();
        let task_manager = engine.task_manager();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        let rendering_engine = engine.rendering_engine();
        drop(engine);

        let agent_bridge = self.agent_bridge.borrow().clone();

        // Agent mode wraps the engine input so commands posted via
        // `/v1/input/*` are observable by every consumer (scripts,
        // actor controllers, the director's own polls). Without an
        // agent, the real input handle is used unchanged. This mirrors
        // the choice `loader_for` makes for the AssetLoader below.
        let input_engine: Rc<RefCell<dyn InputEngine>> = match &agent_bridge {
            Some(bridge) => {
                let synth: Rc<RefCell<SyntheticInputBridge>> = bridge.input_bridge.clone();
                synth
            }
            None => real_input,
        };

        // Reuse the single app-lifetime AssetLoader the start menu (or
        // our own `create_director`) already mounted — the director and
        // the menu share one asset front-door instead of remounting a
        // second `MiniFs`.
        let loader = self.loader_for(asset_path);

        let director = OpenPAL4Director::new(
            component_factory.clone(),
            loader,
            scene_manager,
            ui,
            input_engine,
            audio_engine,
            task_manager,
            self.session_handle(),
        );

        if let Some(bundle) = self.build_debug_bundle() {
            director.set_debug_bundle(bundle);
        }
        if let Some(factory) = self.script_factory.borrow().clone() {
            director.set_actor_controller_factory(factory.clone());
            // The loading overlay is built per-launch (it captures
            // the host context for lazy `open_layout`) and survives
            // for the director's lifetime. Mirrors the debug-bundle
            // path: silently degrade to the legacy synchronous
            // `load_scene` flow if no script factory is installed
            // (e.g. headless test harness).
            let overlay = factory.make_pal4_loading_overlay();
            director.set_loading_overlay(overlay);
        }

        if let Some(bridge) = agent_bridge {
            bridge.set_rendering_engine(rendering_engine);
            director.set_agent_bridge(bridge);
        }

        director
    }
}
