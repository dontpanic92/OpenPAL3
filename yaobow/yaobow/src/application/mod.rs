pub mod yaobow_host_context;

use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;

use fileformats::{binrw::BinRead, npc::NpcInfoFile};

use agent_server::AgentServer;
use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{
        IApplication, IApplicationExt, IApplicationLoaderComponent, IComponentImpl, IDirector,
        ISceneManager,
    },
    input::SyntheticInputBridge,
    scene::CoreScene,
};
use radiance_scripting::install_imgui_pump;
use shared::openpal4::agent::Pal4AgentBridge;
use shared::openpal4::launch::{AgentBootOptions, install_global_log_sink, start_agent_server};
use shared::{GameType, config::YaobowConfig};

pub type Pal4AgentBootOptions = AgentBootOptions;

use crate::comdef::yaobow_services::{IYaobowHostContext, IYaobowScriptApp};
use crate::script_source::install_script_factory;

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
///  1. Call `install_script_factory` (which builds the per-game
///     services + `IYaobowHostContext`, bootstraps + reverse-wraps the
///     script app, and installs the PAL4 `IPal4ScriptFactory` on
///     `Pal4Service`); the loader holds the returned factory +
///     host-context handles.
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
    /// The reverse-wrapped script app factory (`make_title_director`,
    /// `make_pal5_director`). Held for the loader lifetime so the script
    /// box stays rooted. Set in `on_loading`.
    factory: RefCell<Option<ComRc<IYaobowScriptApp>>>,
    /// The canonical host context, used to reach the per-game services.
    /// Set in `on_loading`.
    host_context: RefCell<Option<ComRc<IYaobowHostContext>>>,
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

        // Bootstrap the script root. This also QIs the script's PAL4
        // factory surface and installs it on `Pal4Service`.
        let (factory, host_context) = install_script_factory(&self.app, self.config.clone());
        self.factory.replace(Some(factory.clone()));
        self.host_context.replace(Some(host_context.clone()));

        // Hook the imgui texture cache into Pal4Service.
        let pal4 = host_context.pal4();
        let texture_cache = install_imgui_pump(&self.app);
        pal4.inner::<shared::openpal4::service::Pal4Service>()
            .set_texture_cache(texture_cache);

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
                if let Err(err) =
                    self.direct_boot(game, asset_path, &factory, &host_context, &scene_manager)
                {
                    log::warn!(
                        "direct-boot launch for {} failed: {err}; falling back to title",
                        game.app_name()
                    );
                    self.install_title_director(&scene_manager, &factory);
                }
            }
            None => self.install_title_director(&scene_manager, &factory),
        }
    }

    fn on_unloading(&self) {
        // Break the strong reference cycle (Pal4Service → script factory
        // → script box → host context → Pal4Service) at teardown, then
        // drop the loader's own handles so the whole graph releases.
        if let Some(host_context) = self.host_context.borrow().as_ref() {
            host_context
                .pal4()
                .inner::<shared::openpal4::service::Pal4Service>()
                .clear_script_factory();
        }
        self.factory.replace(None);
        self.host_context.replace(None);
    }

    /// Drives the PAL4 app-lifetime **single agent dispatcher** each
    /// frame: it is the sole drainer of the agent command queue in every
    /// mode, delegating to the active `OpenPAL4Director` in story mode
    /// (resolved from the `SceneManager`) and answering the mode-agnostic
    /// subset otherwise. No-op unless a PAL4 agent bridge was installed
    /// (`--pal4 --agent-port`). Runs before `engine.update` so commands
    /// land before the active director ticks its VM this frame.
    fn on_updating(&self, delta_sec: f32) {
        if let Some(host_context) = self.host_context.borrow().as_ref() {
            host_context
                .pal4()
                .inner::<shared::openpal4::service::Pal4Service>()
                .pump_agent(delta_sec);
        }
    }
}

impl YaobowApplicationLoader {
    pub fn new(app: ComRc<IApplication>) -> Self {
        Self {
            app,
            config: Rc::new(RefCell::new(YaobowConfig::load())),
            factory: RefCell::new(None),
            host_context: RefCell::new(None),
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
        factory: &ComRc<IYaobowScriptApp>,
    ) {
        let director = factory
            .make_title_director()
            .query_interface::<IDirector>()
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
        factory: &ComRc<IYaobowScriptApp>,
        host_ctx: &ComRc<IYaobowHostContext>,
        scene_manager: &ComRc<ISceneManager>,
    ) -> Result<(), String> {
        let director = match game {
            GameType::PAL3 => host_ctx.pal3().create_director(&asset_path),
            GameType::PAL4 => {
                self.boot_pal4_agent_if_requested(host_ctx)?;
                host_ctx.pal4().create_director(&asset_path)
            }
            GameType::PAL5 | GameType::PAL5Q => {
                let game_ordinal = ordinal_for_game(game);
                factory
                    .make_pal5_director(&asset_path, game_ordinal)
                    .ok_or_else(|| {
                        format!(
                            "make_pal5_director returned null (likely missing PAL5 assets at {asset_path})"
                        )
                    })?
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

    // The runtime theme reads the engine's imgui context, so it can't
    // run until first-resumed has bootstrapped the engine. Register
    // as an engine-ready callback — it fires on the first run-loop
    // tick after the bootstrap, before any component on_loading
    // (and hence before any rendering).
    {
        let app2 = app.clone();
        app.add_engine_ready_callback(Box::new(move || {
            shared::theme_runtime::apply_runtime_theme(&app2);
        }));
    }

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

pub fn run_openpal4() {
    run_openpal4_with_agent(None);
}

pub fn run_openpal4_with_agent(agent: Option<AgentBootOptions>) {
    run_app(boot_for(GameType::PAL4).with_agent_opts_opt(agent));
}

pub fn run_openswd5() {
    run_app(boot_for(GameType::SWDHC));
}

pub fn run_opengujian() {
    let data = std::fs::read("F:\\PAL4\\gamedata\\scenedata\\scenedata\\q01\\N01\\npcInfo.npc")
        .expect("Gujian NPC info path must exist on this machine");
    let cam = NpcInfoFile::read(&mut Cursor::new(data));
    println!("cam: {:?}", cam);
}
