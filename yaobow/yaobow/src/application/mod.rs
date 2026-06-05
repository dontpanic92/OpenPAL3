pub mod yaobow_host_context;

use std::cell::RefCell;
use std::rc::Rc;

use agent_server::AgentServer;
use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{
        IApplication, IApplicationExt, IApplicationLoaderComponent, IComponentImpl, ISceneManager,
    },
    input::SyntheticInputBridge,
    scene::CoreScene,
};
use radiance_scripting::install_imgui_pump;
use shared::openpal4::agent::Pal4AgentBridge;
use shared::openpal4::launch::{install_global_log_sink, start_agent_server, AgentBootOptions};
use shared::{config::YaobowConfig, GameType};

use crate::comdef::yaobow_services::IYaobowHostContext;
use crate::script_source::YaobowScriptProject;

/// Single boot-option bundle for the yaobow application. Replaces
/// the previous family of `create_application_*` / `run_*` variants
/// with a single parameter object + builder methods.
///
/// Construct via [`BootOptions::title_page`] for the title director
/// or [`BootOptions::for_game`] / [`boot_for`] for direct-boot. Chain
/// `.with_asset_path(...)` and `.with_agent_opts(...)` as needed.
#[derive(Default)]
pub struct BootOptions {
    /// `None` → title page; `Some(game)` → direct boot into `game`.
    pub initial_game: Option<GameType>,
    /// Asset path override. `None` falls back to
    /// `config.asset_path_for(initial_game)` inside
    /// `YaobowApplicationLoader::on_loading`.
    pub asset_path: Option<String>,
    /// PAL4 agent-server boot options. Only meaningful when
    /// `initial_game == Some(GameType::PAL4)`.
    pub agent_opts: Option<AgentBootOptions>,
}

impl BootOptions {
    /// No initial game → title director is installed at boot.
    pub fn title_page() -> Self {
        Self::default()
    }

    /// Direct boot into `game` with default asset path (resolved by
    /// the loader via `YaobowConfig`). Use
    /// `.with_asset_path(...)` to override.
    pub fn for_game(game: GameType) -> Self {
        Self {
            initial_game: Some(game),
            asset_path: None,
            agent_opts: None,
        }
    }

    pub fn with_asset_path(mut self, asset_path: String) -> Self {
        self.asset_path = Some(asset_path);
        self
    }

    /// Variant that accepts an `Option<String>` so call-sites that
    /// pipe through [`resolve_asset_path`] don't need a `match`.
    pub fn with_asset_path_opt(mut self, asset_path: Option<String>) -> Self {
        if let Some(p) = asset_path {
            self.asset_path = Some(p);
        }
        self
    }

    pub fn with_agent_opts(mut self, opts: AgentBootOptions) -> Self {
        self.agent_opts = Some(opts);
        self
    }

    /// Variant that accepts `Option<AgentBootOptions>` so call-sites
    /// with optional agent boot don't need a `match`.
    pub fn with_agent_opts_opt(mut self, opts: Option<AgentBootOptions>) -> Self {
        if let Some(o) = opts {
            self.agent_opts = Some(o);
        }
        self
    }
}

/// The yaobow application loader (phase 2 — direct script handoff).
///
/// Responsibilities:
///  1. Install the `YaobowScriptProject` (which bootstraps the
///     `IYaobowHostContext` carrying the per-game services and the
///     rooted script app).
///  2. Install the imgui pump (so script-side directors' `render_im`
///     fires inside the imgui frame scope).
///  3. Configure `Pal4Service` with the agent bridge (if
///     `--pal4 --agent-port` was passed) and the scripted actor
///     controller factory.
///  4. Push the initial director: either the script-side title
///     director (default) or — for `--palX` CLI direct boot — the
///     per-game director constructed via `host.palX().create_director()`
///     directly (no title page flicker).
///
/// During normal play, per-game launches are dispatched entirely
/// from the script side (`title.p7`'s click handler) — no Rust
/// dispatcher, no `selected_game` slot, no `on_updating` polling.
pub struct YaobowApplicationLoader {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
    project: RefCell<Option<Rc<YaobowScriptProject>>>,
    initial_game: Option<GameType>,
    initial_asset_path: RefCell<Option<String>>,
    initial_agent_opts: RefCell<Option<AgentBootOptions>>,
    /// Live PAL4 agent-server HTTP listener. Held for the loader
    /// lifetime so the listener thread is joined exactly once at
    /// process exit.
    agent_server: RefCell<Option<AgentServer>>,
}

ComObject_YaobowApplicationLoader!(super::YaobowApplicationLoader);

impl IComponentImpl for YaobowApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title("妖弓 - Project Yaobow");

        let project = YaobowScriptProject::install(&self.app, self.config.clone());
        self.project.replace(Some(project.clone()));

        // Hook the scripted actor-controller factory into Pal4Service
        // now that the project exists. Done unconditionally so a
        // later title-page PAL4 selection also picks it up.
        let host_ctx = project.host_context();
        let pal4 = host_ctx.pal4();
        pal4.inner::<shared::openpal4::service::Pal4Service>()
            .set_actor_controller_factory(project.actor_controller_factory());

        let _ = install_imgui_pump(&self.app);

        let scene_manager = self.app.engine().borrow().scene_manager().clone();

        match self.initial_game {
            // CLI direct boot — skip the title director entirely and
            // route through the per-game service directly so there's
            // no one-frame title flicker before the swap.
            Some(game) => {
                let asset_path = self
                    .initial_asset_path
                    .borrow()
                    .clone()
                    .unwrap_or_else(|| self.config.borrow().asset_path_for(game).to_string());
                if let Err(err) = self.direct_boot(game, asset_path, &project, &scene_manager) {
                    log::warn!(
                        "direct-boot launch for {} failed: {err}; falling back to title",
                        game.app_name()
                    );
                    self.install_title_director(&scene_manager, &project);
                }
            }
            None => self.install_title_director(&scene_manager, &project),
        }
    }

    fn on_unloading(&self) {}

    /// Empty after phase 2 — per-game dispatch happens script-side
    /// via `title.p7`'s click handler returning the next director
    /// from its `update()`. No more `selected_game` slot polling.
    fn on_updating(&self, _delta_sec: f32) {}
}

impl YaobowApplicationLoader {
    pub fn new(app: ComRc<IApplication>) -> Self {
        Self {
            app,
            config: Rc::new(RefCell::new(YaobowConfig::load())),
            project: RefCell::new(None),
            initial_game: None,
            initial_asset_path: RefCell::new(None),
            initial_agent_opts: RefCell::new(None),
            agent_server: RefCell::new(None),
        }
    }

    pub fn new_with_initial_game(app: ComRc<IApplication>, game: GameType) -> Self {
        let mut loader = Self::new(app);
        loader.initial_game = Some(game);
        loader
    }

    fn install_title_director(
        &self,
        scene_manager: &ComRc<ISceneManager>,
        project: &Rc<YaobowScriptProject>,
    ) {
        let director = project
            .make_title_director_as_director()
            .expect("initial script director must be created");
        scene_manager.set_director(director);
        scene_manager.push_scene(CoreScene::create());
    }

    /// Direct-boot path for CLI flags. Calls into the matching
    /// per-game service (or, for PAL5, the script-side factory) and
    /// installs the returned director from `on_loading` — safe
    /// because no director update is in progress here.
    fn direct_boot(
        &self,
        game: GameType,
        asset_path: String,
        project: &Rc<YaobowScriptProject>,
        scene_manager: &ComRc<ISceneManager>,
    ) -> Result<(), String> {
        let host_ctx = project.host_context();

        let director = match game {
            GameType::PAL3 => host_ctx.pal3().create_director(&asset_path),
            GameType::PAL4 => {
                self.boot_pal4_agent_if_requested(&host_ctx)?;
                host_ctx.pal4().create_director(&asset_path)
            }
            GameType::PAL5 | GameType::PAL5Q => {
                let game_ordinal = ordinal_for_game(game);
                project.make_pal5_director(&asset_path, game_ordinal)?
            }
            GameType::SWD5 | GameType::SWDHC | GameType::SWDCF => {
                let game_ordinal = ordinal_for_game(game);
                host_ctx.swd5().create_director(&asset_path, game_ordinal)
            }
            _ => return Err(format!("unsupported game type: {}", game.app_name())),
        };

        scene_manager.set_director(director);
        Ok(())
    }

    /// Boot the PAL4 agent server (if `initial_agent_opts` is set)
    /// and install the bridge on `Pal4Service`. Called once at
    /// `on_loading` time when `--pal4 --agent-port` is in effect.
    fn boot_pal4_agent_if_requested(
        &self,
        host_ctx: &ComRc<IYaobowHostContext>,
    ) -> Result<(), String> {
        let opts = match self.initial_agent_opts.borrow_mut().take() {
            Some(opts) => opts,
            None => return Ok(()),
        };

        let real_input = self.app.engine().borrow().input_engine();
        let synth = Rc::new(RefCell::new(SyntheticInputBridge::new(real_input)));
        let bridge = Rc::new(Pal4AgentBridge::new(synth));

        let log_sink = install_global_log_sink();
        let server = start_agent_server(&opts, &bridge, log_sink)?;
        log::info!(
            "agent_server: listening on http://{} (PAL4)",
            server.local_addr()
        );
        *self.agent_server.borrow_mut() = Some(server);

        host_ctx
            .pal4()
            .inner::<shared::openpal4::service::Pal4Service>()
            .set_agent_bridge(bridge);
        Ok(())
    }
}

/// Convert a `GameType` to the same ordinal the script side uses
/// (via `radiance_scripting::services::game_registry`).
fn ordinal_for_game(game: GameType) -> i32 {
    for ord in 0..32 {
        if radiance_scripting::services::game_registry::ordinal_to_config_key(ord)
            == Some(game.config_key())
        {
            return ord;
        }
    }
    -1
}

pub fn create_application(opts: BootOptions) -> ComRc<IApplication> {
    let app = ComRc::<IApplication>::from_object(Application::new());
    let mut loader = match opts.initial_game {
        Some(game) => YaobowApplicationLoader::new_with_initial_game(app.clone(), game),
        None => YaobowApplicationLoader::new(app.clone()),
    };
    if let Some(p) = opts.asset_path {
        loader.initial_asset_path = RefCell::new(Some(p));
    }
    if let Some(a) = opts.agent_opts {
        loader.initial_agent_opts = RefCell::new(Some(a));
    }
    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(loader),
    );
    app
}

/// One-shot helper: build the application from `opts`, initialize the
/// engine, apply the runtime theme, and run the main loop. Every
/// per-game CLI entry (`run_openpal3`, `run_openpal4_with_agent`, …)
/// boils down to a single call to this with a populated
/// [`BootOptions`].
pub fn run_app(opts: BootOptions) {
    let app = create_application(opts);
    app.initialize();
    shared::theme_runtime::apply_runtime_theme(&app);
    app.run();
}

/// Resolve the asset path for `game`, honouring per-platform
/// fallbacks: desktop reads `YaobowConfig` and falls back to a
/// hardcoded dev path when the config slot is empty; Android / Vita
/// use platform-specific roots. Returns `None` when no asset path is
/// applicable (in which case the loader falls back to
/// `config.asset_path_for(game)` empty-string behaviour, which most
/// games accept by erroring at load time).
pub fn resolve_asset_path(game: GameType) -> Option<String> {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        let configured = YaobowConfig::load().asset_path_for(game).to_string();
        if !configured.is_empty() {
            return Some(configured);
        }
        // Per-game hardcoded fallback for dev workflows — preserves
        // the legacy `OpenPalXApplicationLoader::new` defaults.
        return match game {
            GameType::PAL4 => Some("F:\\PAL4_test".to_string()),
            GameType::SWD5 | GameType::SWDHC | GameType::SWDCF => {
                Some("F:\\SteamLibrary\\steamapps\\common\\SWDHC".to_string())
            }
            _ => None,
        };
    }
    #[cfg(target_os = "android")]
    {
        return match game {
            GameType::PAL3 => Some("/sdcard/Games/PAL3".to_string()),
            GameType::PAL4 => Some("/sdcard/Games/PAL4".to_string()),
            _ => None,
        };
    }
    #[cfg(vita)]
    {
        return match game {
            GameType::PAL3 => Some("ux0:games/PAL3".to_string()),
            GameType::PAL4 => Some("ux0:games/PAL4".to_string()),
            _ => None,
        };
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
        vita
    )))]
    {
        let _ = game;
        None
    }
}

/// Convenience: a [`BootOptions`] for `game` with the asset path
/// resolved via [`resolve_asset_path`]. Used by every CLI direct-boot
/// helper so they stay one-liners.
pub fn boot_for(game: GameType) -> BootOptions {
    BootOptions::for_game(game).with_asset_path_opt(resolve_asset_path(game))
}

pub fn run_title_selection() {
    run_app(BootOptions::title_page());
}

pub fn run_openpal5() {
    run_app(boot_for(GameType::PAL5));
}

pub fn run_openpal5q() {
    run_app(boot_for(GameType::PAL5Q));
}
