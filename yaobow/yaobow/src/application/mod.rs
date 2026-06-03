pub mod yaobow_app_context;

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{
        IApplication, IApplicationExt, IApplicationLoaderComponent, IComponent, IComponentImpl,
    },
    scene::CoreScene,
};
use radiance_scripting::install_imgui_pump;
use shared::openpal5::context::Pal5Context;
use shared::{config::YaobowConfig, GameType};

use crate::script_source::YaobowScriptProject;
use crate::{
    openpal3::OpenPal3ApplicationLoader, openpal4::application::OpenPal4ApplicationLoader,
    openswd5::application::OpenSwd5ApplicationLoader,
};

pub struct YaobowApplicationLoader {
    app: ComRc<IApplication>,
    config: Rc<RefCell<YaobowConfig>>,
    selected_game: RefCell<Option<Rc<RefCell<Option<GameType>>>>>,
    project: RefCell<Option<Rc<YaobowScriptProject>>>,
    /// Optional pre-selected game. When `Some(...)` the loader skips
    /// the title director on first load and routes straight to the
    /// per-game launch path. Used by the `--pal3` / `--pal4` / `--pal5`
    /// CLI flags so direct boot doesn't flash the title page for a
    /// frame before swapping in the game director.
    initial_game: Option<GameType>,
}

ComObject_YaobowApplicationLoader!(super::YaobowApplicationLoader);

impl IComponentImpl for YaobowApplicationLoader {
    fn on_loading(&self) {
        self.app.set_title("妖弓 - Project Yaobow");

        let project = YaobowScriptProject::install(&self.app, self.config.clone());
        self.selected_game.replace(Some(project.selected_game()));
        self.project.replace(Some(project.clone()));

        let _ = install_imgui_pump(&self.app);

        let scene_manager = self.app.engine().borrow().scene_manager().clone();

        match self.initial_game {
            // CLI direct boot — skip the title director entirely and
            // route through the per-game launch path so there's no
            // one-frame title flicker before the swap.
            Some(game) => {
                let asset_path = self.config.borrow().asset_path_for(game).to_string();
                launch_game(game, self.app.clone(), asset_path, &project);
            }
            None => {
                let director = project
                    .make_title_director_as_director()
                    .expect("initial script director must be created");
                scene_manager.set_director(director);
                scene_manager.push_scene(CoreScene::create());
            }
        }
    }

    fn on_updating(&self, _delta_sec: f32) {
        let slot = self.selected_game.borrow();
        let Some(slot) = slot.as_ref() else {
            return;
        };
        if slot.borrow().is_none() {
            return;
        }

        let game = slot.borrow().unwrap();
        let asset_path = self.config.borrow().asset_path_for(game).to_string();

        // PAL5 is driven by the protosept orchestrator
        // (`yaobow/yaobow/scripts/openpal5.p7::launch`); every other
        // game still goes through the legacy `create_loader` path.
        let success = if game == GameType::PAL5 || game == GameType::PAL5Q {
            let project = self
                .project
                .borrow()
                .clone()
                .expect("YaobowScriptProject must be installed before per-game launch");
            match launch_openpal5_via_script(&project, self.app.clone(), game, asset_path) {
                Ok(()) => true,
                Err(err) => {
                    log::warn!(
                        "openpal5 launch script failed: {err}; staying on title director"
                    );
                    false
                }
            }
        } else {
            let loader = create_loader(game, self.app.clone(), asset_path)
                .query_interface::<IComponent>()
                .unwrap();
            loader.on_loading();
            true
        };

        // Clear the slot only on a successful launch so a failure
        // leaves the title director in place (the user can pick a
        // different game / fix the asset path / etc.).
        if success {
            slot.replace(None);
        }
    }

    fn on_unloading(&self) {}
}

impl YaobowApplicationLoader {
    pub fn new(app: ComRc<IApplication>) -> Self {
        Self {
            app,
            config: Rc::new(RefCell::new(YaobowConfig::load())),
            selected_game: RefCell::new(None),
            project: RefCell::new(None),
            initial_game: None,
        }
    }

    /// Variant used by the CLI direct-entry helpers (`--pal3`,
    /// `--pal4`, `--pal5`, …). Pre-selects `game` so the loader
    /// skips the title director on first load.
    pub fn new_with_initial_game(app: ComRc<IApplication>, game: GameType) -> Self {
        let mut loader = Self::new(app);
        loader.initial_game = Some(game);
        loader
    }
}

fn create_loader(
    game: GameType,
    app: ComRc<IApplication>,
    asset_path: String,
) -> ComRc<IApplicationLoaderComponent> {
    match game {
        GameType::PAL3 => OpenPal3ApplicationLoader::create(app, &asset_path),
        GameType::PAL4 => OpenPal4ApplicationLoader::create(app, asset_path),
        GameType::SWDHC => OpenSwd5ApplicationLoader::create(app, asset_path),
        // PAL5 is handled by the protosept-driven path in
        // `on_updating` / `launch_game`; reaching this arm would be a
        // routing bug rather than a missing implementation.
        _ => unimplemented!(),
    }
}

/// Per-game launch helper shared by the title-page (`on_updating`)
/// and the CLI direct-entry (`on_loading` initial_game) paths.
fn launch_game(
    game: GameType,
    app: ComRc<IApplication>,
    asset_path: String,
    project: &Rc<YaobowScriptProject>,
) {
    if game == GameType::PAL5 || game == GameType::PAL5Q {
        if let Err(err) = launch_openpal5_via_script(project, app, game, asset_path) {
            log::warn!("openpal5 direct-boot script failed: {err}");
        }
        return;
    }

    let loader = create_loader(game, app, asset_path)
        .query_interface::<IComponent>()
        .unwrap();
    loader.on_loading();
}

/// Constructs the PAL5 launch context and hands it off to the
/// protosept launch script via the installed `YaobowScriptProject`.
/// The Rust side stays tiny — the context only carries the
/// pal5-specific state (asset path + the rendering factory the
/// `AssetLoader` needs internally). Generic engine concerns
/// (window title, scene manager, camera, input) flow through the
/// canonical `IHostContext` services the script already has.
fn launch_openpal5_via_script(
    project: &Rc<YaobowScriptProject>,
    app: ComRc<IApplication>,
    game: GameType,
    asset_path: String,
) -> Result<(), String> {
    let factory = app.engine().borrow().rendering_component_factory();
    let ctx = Pal5Context::create(game, asset_path, factory);
    project.launch_pal5(ctx)
}

pub fn create_application() -> ComRc<IApplication> {
    let app = ComRc::<IApplication>::from_object(Application::new());
    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(YaobowApplicationLoader::new(app.clone())),
    );

    app
}

/// Builds an application pre-seeded with `game` so the CLI direct
/// entries (`--pal3`, `--pal4`, `--pal5`, …) skip the title page
/// without a one-frame flicker.
pub fn create_application_for_game(game: GameType) -> ComRc<IApplication> {
    let app = ComRc::<IApplication>::from_object(Application::new());
    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(YaobowApplicationLoader::new_with_initial_game(
            app.clone(),
            game,
        )),
    );

    app
}

pub fn run_title_selection() {
    let app = create_application();
    app.initialize();
    shared::theme_runtime::apply_runtime_theme(&app);
    app.run();
}

/// CLI direct-entry replacement for the legacy `run_openpal5` in
/// `yaobow::openpal5`. Boots the full yaobow application loader with
/// `initial_game = Some(GameType::PAL5)` so the title page is
/// skipped and the protosept-driven PAL5 launch fires immediately on
/// first `on_loading`.
pub fn run_openpal5() {
    run_pal5_family(GameType::PAL5);
}

/// CLI direct-entry for `--pal5q`. Mirrors `run_openpal5` but boots
/// as `GameType::PAL5Q` so `Pal5Context` threads the correct
/// `.pkg` decryption key (`GameType::pkg_key()`) into
/// `packfs::init_virtual_fs` and the per-game asset path / save dir
/// resolve via the right `config_key`.
pub fn run_openpal5q() {
    run_pal5_family(GameType::PAL5Q);
}

fn run_pal5_family(game: GameType) {
    debug_assert!(matches!(game, GameType::PAL5 | GameType::PAL5Q));
    let app = create_application_for_game(game);
    app.initialize();
    shared::theme_runtime::apply_runtime_theme(&app);
    app.run();
}
