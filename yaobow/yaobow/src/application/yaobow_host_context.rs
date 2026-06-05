//! Yaobow-specific extension of the canonical `IHostContext`.
//!
//! This module is the single home for everything related to the host
//! context the script side receives at `app.p7::init(ctx)`:
//!
//!   * [`YaobowHostContext`] — the real ComObject implementing
//!     `IYaobowHostContext` (and, via inheritance, `IHostContext`).
//!     Holds an inner `ComRc<IHostContext>` for the generic engine
//!     services and four per-game service handles
//!     (`pal3 / pal4 / pal5 / swd5`).
//!   * [`build_inner_host_context`] — private factory that constructs
//!     the generic inner `IHostContext` (audio / textures / vfs /
//!     input / app / config) from engine handles. Used only by
//!     `script_source::install`.
//!   * [`YaobowAppService`] — vestigial `IAppService` impl. After
//!     phase 2, only its `exit()` method is reached (from
//!     `title.p7`'s exit-button handler). `open_game` is a no-op:
//!     the title page now dispatches per-game launches via
//!     `host.palX().create_director()` directly. Kept because
//!     `IAppService` is required by `IHostContext::app()` and the
//!     editor's `AppService` shares the same interface.
//!   * [`YAOBOW_HOST_CONTEXT_TYPE_TAG`] — the IDL-derived type tag
//!     `bootstrap_script_root` uses to foreign-box the context for
//!     the script.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crosscom::ComRc;
use mini_fs::{LocalFs, MiniFs, ZipFs};
use radiance::comdef::{IApplication, IApplicationExt, IDirector, ISceneManager};
use radiance_scripting::comdef::services::{
    IAppService, IAppServiceImpl, IAudioService, IConfigService, IGameRegistry, IHostContext,
    IHostContextImpl, IInputService, IRandomService, ITextureService, IVfsService,
};
use radiance_scripting::services::HostContext;
use shared::config::YaobowConfig;
use shared::config_service::ConfigService;
use shared::openpal3::comdef::IPal3Service;
use shared::openpal4::comdef::IPal4Service;
use shared::openpal5::comdef::IPal5Service;
use shared::openswd5::comdef::ISwd5Service;

use crate::comdef::yaobow_services::{IYaobowHostContext, IYaobowHostContextImpl};

/// Type tag the script uses to recognise the host context handed to
/// `app.p7::init`. Mirrors the structure of the auto-generated tags
/// produced by `crosscom_protosept::interface_type_tag` from
/// `module(rust) yaobow::comdef::yaobow_services` + the interface
/// name `IYaobowHostContext`.
pub const YAOBOW_HOST_CONTEXT_TYPE_TAG: &str =
    "yaobow.comdef.yaobow_services.IYaobowHostContext";

// ---------------------------------------------------------------------------
// YaobowHostContext (the real ComObject)
// ---------------------------------------------------------------------------

pub struct YaobowHostContext {
    inner: ComRc<IHostContext>,
    pal3: ComRc<IPal3Service>,
    pal4: ComRc<IPal4Service>,
    pal5: ComRc<IPal5Service>,
    swd5: ComRc<ISwd5Service>,
}

ComObject_YaobowHostContext!(super::YaobowHostContext);

impl YaobowHostContext {
    /// Build the full yaobow host context. Internally constructs the
    /// generic inner `IHostContext` from engine handles + the
    /// `YaobowAppService`, then wraps it with the supplied per-game
    /// service handles. Called once from `script_source::install`.
    pub fn create(
        app: ComRc<IApplication>,
        config: Rc<RefCell<YaobowConfig>>,
        pal3: ComRc<IPal3Service>,
        pal4: ComRc<IPal4Service>,
        pal5: ComRc<IPal5Service>,
        swd5: ComRc<ISwd5Service>,
    ) -> ComRc<IYaobowHostContext> {
        let app_service = YaobowAppService::create(app.clone());
        let inner = build_inner_host_context(app, app_service, config);
        ComRc::from_object(Self {
            inner,
            pal3,
            pal4,
            pal5,
            swd5,
        })
    }
}

impl IHostContextImpl for YaobowHostContext {
    fn scene_manager(&self) -> ComRc<ISceneManager> {
        self.inner.scene_manager()
    }
    fn audio(&self) -> ComRc<IAudioService> {
        self.inner.audio()
    }
    fn textures(&self) -> ComRc<ITextureService> {
        self.inner.textures()
    }
    fn vfs(&self) -> ComRc<IVfsService> {
        self.inner.vfs()
    }
    fn input(&self) -> ComRc<IInputService> {
        self.inner.input()
    }
    fn games(&self) -> ComRc<IGameRegistry> {
        self.inner.games()
    }
    fn app(&self) -> ComRc<IAppService> {
        self.inner.app()
    }
    fn random(&self) -> ComRc<IRandomService> {
        self.inner.random()
    }
    fn config(&self) -> ComRc<IConfigService> {
        self.inner.config()
    }
}

impl IYaobowHostContextImpl for YaobowHostContext {
    fn pal3(&self) -> ComRc<IPal3Service> {
        self.pal3.clone()
    }
    fn pal4(&self) -> ComRc<IPal4Service> {
        self.pal4.clone()
    }
    fn pal5(&self) -> ComRc<IPal5Service> {
        self.pal5.clone()
    }
    fn swd5(&self) -> ComRc<ISwd5Service> {
        self.swd5.clone()
    }
}

// ---------------------------------------------------------------------------
// Inner host context builder (private)
// ---------------------------------------------------------------------------

/// Stitch the generic engine services + yaobow's app vfs +
/// `ConfigService` + `YaobowAppService` into the canonical
/// `radiance_scripting::HostContext`. Returns the inner
/// `ComRc<IHostContext>` that [`YaobowHostContext::create`] wraps
/// with per-game getters.
fn build_inner_host_context(
    app: ComRc<IApplication>,
    app_service: ComRc<IAppService>,
    config: Rc<RefCell<YaobowConfig>>,
) -> ComRc<IHostContext> {
    let engine_rc = app.engine();
    let engine = engine_rc.borrow();
    let vfs = load_app_vfs();
    let imgui_ctx = engine.ui_manager().imgui_context();
    let config_service = ConfigService::create_with_imgui(config, Some(imgui_ctx));

    HostContext::create(
        engine.scene_manager(),
        engine.audio_engine(),
        engine.rendering_component_factory(),
        vfs,
        engine.input_engine(),
        app_service,
        config_service,
    )
}

// ---------------------------------------------------------------------------
// YaobowAppService (vestigial; only `exit()` is reached after phase 2)
// ---------------------------------------------------------------------------

struct YaobowAppService {
    app: ComRc<IApplication>,
}

radiance_scripting::ComObject_AppService!(super::YaobowAppService);

impl YaobowAppService {
    fn create(app: ComRc<IApplication>) -> ComRc<IAppService> {
        ComRc::from_object(Self { app })
    }
}

impl IAppServiceImpl for YaobowAppService {
    fn open_game(&self, _ordinal: i32) -> Option<ComRc<IDirector>> {
        log::warn!(
            "YaobowAppService::open_game called — yaobow no longer dispatches via open_game. \
             The title page is expected to call host.palX().create_director() directly."
        );
        None
    }

    fn exit(&self) {
        self.app.request_exit();
    }

    fn set_title(&self, title: &str) {
        self.app.set_title(title);
    }
}

// ---------------------------------------------------------------------------
// App-vfs probing (yaobow asset zip locations)
// ---------------------------------------------------------------------------

/// Probes for the yaobow asset zip in the same order as the legacy
/// title director's `load_vfs`: bundled zip, dev tree, parent dev
/// tree, then the FHS install location. Returns an empty `MiniFs`
/// if none match.
fn load_app_vfs() -> Rc<MiniFs> {
    let mut vfs = MiniFs::new(false);
    let zip = PathBuf::from(ASSET_PATH);
    let local1 = PathBuf::from("./yaobow/yaobow-assets");
    let local2 = PathBuf::from("../yaobow-assets");
    let local3 = PathBuf::from("/usr/share/yaobow/yaobow-assets");

    if Path::exists(&zip) {
        let local = ZipFs::new(std::fs::File::open(zip).unwrap());
        vfs = vfs.mount(PathBuf::from("/"), local);
    } else if Path::exists(&local1) {
        let local = LocalFs::new(&local1);
        vfs = vfs.mount(PathBuf::from("/"), local);
    } else if Path::exists(&local2) {
        let local = LocalFs::new(&local2);
        vfs = vfs.mount(PathBuf::from("/"), local);
    } else if Path::exists(&local3) {
        let local = LocalFs::new(&local3);
        vfs = vfs.mount(PathBuf::from("/"), local);
    }
    Rc::new(vfs)
}

#[cfg(windows)]
const ASSET_PATH: &str = "./yaobow-assets.zip";
#[cfg(any(linux, macos))]
const ASSET_PATH: &str = "../shared/yaobow/yaobow-assets.zip";
#[cfg(vita)]
const ASSET_PATH: &str = "ux0:data/yaobow-assets.zip";
#[cfg(not(any(windows, linux, macos, vita)))]
const ASSET_PATH: &str = "./yaobow-assets.zip";
