use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;

use agent_server::{AgentLogSink, AgentServer, AgentServerConfig};
use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::input::{InputEngine, SyntheticInputBridge};
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationExt, IApplicationLoaderComponent, IComponentImpl},
};
use radiance_scripting::install_imgui_pump;
use shared::config::YaobowConfig;
use shared::openpal4::agent::Pal4AgentBridge;
use shared::openpal4::{asset_loader::AssetLoader, director::OpenPAL4Director};

use crate::script_source::YaobowScriptProject;

/// Boot-time options for the embedded agent server. `None` keeps the
/// classic windowed-only flow with no extra threads or input wrapping.
#[derive(Debug, Clone)]
pub struct AgentBootOptions {
    /// Port to bind. `0` selects an ephemeral port (mostly useful for
    /// tests that need to discover the address via [`AgentServer::local_addr`]).
    pub port: u16,
    /// Optional bind override. Defaults to `127.0.0.1` (loopback).
    pub bind_ip: Option<std::net::IpAddr>,
    /// Bearer token. Required for non-loopback binds.
    pub token: Option<String>,
}

impl AgentBootOptions {
    pub fn loopback(port: u16) -> Self {
        Self {
            port,
            bind_ip: None,
            token: None,
        }
    }
}

pub struct OpenPal4ApplicationLoader {
    app: ComRc<IApplication>,
    root_path: PathBuf,
    app_name: String,
    agent: Option<AgentBootOptions>,
    /// Storage for the live agent-server handle. Kept alive for the
    /// lifetime of the loader so the listener thread is joined cleanly
    /// when the binary exits.
    agent_server: RefCell<Option<AgentServer>>,
}

ComObject_OpenPal4ApplicationLoaderComponent!(super::OpenPal4ApplicationLoader);

impl IComponentImpl for OpenPal4ApplicationLoader {
    fn on_loading(&self) {
        self.app
            .set_title(&format!("{} - Project Yaobow", &self.app_name));

        let component_factory = self.app.engine().borrow().rendering_component_factory();
        let real_input = self.app.engine().borrow().input_engine();
        let task_manager = self.app.engine().borrow().task_manager();
        let audio_engine = self.app.engine().borrow().audio_engine();
        let scene_manager = self.app.engine().borrow().scene_manager().clone();
        let ui = self.app.engine().borrow().ui_manager();

        // When the agent server is enabled, wrap the engine's input
        // engine in a `SyntheticInputBridge` so commands posted via
        // `/v1/input/*` are observable by every consumer (scripts,
        // actor controllers, the director's own polls) without
        // forking the engine. The plain windowed flow keeps the real
        // input handle unchanged.
        let (input_engine, synth_handle) = match self.agent.as_ref() {
            Some(_) => {
                let synth = Rc::new(RefCell::new(SyntheticInputBridge::new(real_input.clone())));
                let as_engine: Rc<RefCell<dyn InputEngine>> = synth.clone();
                (as_engine, Some(synth))
            }
            None => (real_input, None),
        };

        let vfs = init_virtual_fs(self.root_path.to_str().unwrap(), None);
        let loader = AssetLoader::new(
            self.app.engine().borrow().rendering_component_factory(),
            input_engine.clone(),
            vfs,
        );

        // Create the PAL4 debug-overlay session before the director is
        // constructed; the resulting bundle is handed to the director
        // so its `render_im` can dispatch into the script-side overlay
        // each frame. `YaobowScriptProject::install` is idempotent —
        // if the title bootstrap already installed the project, this
        // call just returns the cached `Rc<YaobowScriptProject>`.
        let config = Rc::new(RefCell::new(YaobowConfig::load()));
        let project = YaobowScriptProject::install(&self.app, config);
        let debug = project.make_pal4_debug_bundle();
        let actor_controller_factory = project.actor_controller_factory();

        let director = OpenPAL4Director::new(
            component_factory.clone(),
            loader,
            scene_manager.clone(),
            ui,
            input_engine,
            audio_engine,
            task_manager,
        );
        director.set_debug_bundle(debug);
        director.set_actor_controller_factory(actor_controller_factory);

        // If the binary was launched with `--agent-port`, build the
        // shared bridge, install it on the director, then spawn the
        // HTTP listener thread. We do this before wrapping the
        // director in `ComRc<IDirector>` so the bridge can borrow
        // `&self` directly while still owned.
        if let Some(opts) = self.agent.as_ref() {
            let synth = synth_handle
                .clone()
                .expect("synthetic bridge must exist when agent is enabled");
            let bridge = Rc::new(Pal4AgentBridge::new(synth));
            // Hand the live rendering engine to the bridge so
            // `/v1/screenshot` can read back the last presented frame.
            bridge.set_rendering_engine(self.app.engine().borrow().rendering_engine());
            director.set_agent_bridge(bridge.clone());

            let log_sink = install_global_log_sink();
            match start_agent_server(opts, &bridge, log_sink) {
                Ok(server) => {
                    log::info!(
                        "agent_server: listening on http://{} (PAL4)",
                        server.local_addr()
                    );
                    *self.agent_server.borrow_mut() = Some(server);
                }
                Err(err) => {
                    log::error!("agent_server: failed to start ({err}); continuing without agent");
                }
            }
        }

        let director_com: ComRc<radiance::comdef::IDirector> = ComRc::from_object(director);
        scene_manager.set_director(director_com);

        // Install the engine-side imgui pump so
        // `OpenPAL4Director::render_im` fires inside the imgui frame
        // scope each tick. The texture cache is wired even though the
        // v1 debug overlay only emits text — keeps parity with the
        // editor's pump and future-proofs `ui.image(...)` from script.
        let _ = install_imgui_pump(&self.app);
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
}

impl OpenPal4ApplicationLoader {
    pub fn create_application(asset_path: String, app_name: &str) -> ComRc<IApplication> {
        let app = ComRc::<IApplication>::from_object(Application::new());
        app.add_component(
            IApplicationLoaderComponent::uuid(),
            ComRc::from_object(Self::new(app.clone(), asset_path, app_name, None)),
        );

        app
    }

    /// Variant of [`Self::create_application`] that boots the embedded
    /// agent server alongside the game.
    pub fn create_application_with_agent(
        asset_path: String,
        app_name: &str,
        agent: AgentBootOptions,
    ) -> ComRc<IApplication> {
        let app = ComRc::<IApplication>::from_object(Application::new());
        app.add_component(
            IApplicationLoaderComponent::uuid(),
            ComRc::from_object(Self::new(app.clone(), asset_path, app_name, Some(agent))),
        );
        app
    }

    pub fn create(
        app: ComRc<IApplication>,
        asset_path: String,
    ) -> ComRc<IApplicationLoaderComponent> {
        ComRc::from_object(Self::new(app.clone(), asset_path, "OpenPAL4", None))
    }

    fn new(
        app: ComRc<IApplication>,
        asset_path: String,
        app_name: &str,
        agent: Option<AgentBootOptions>,
    ) -> Self {
        let root_path = if cfg!(vita) {
            PathBuf::from("ux0:games/PAL4")
        } else if !asset_path.is_empty() {
            PathBuf::from(asset_path)
        } else {
            PathBuf::from("F:\\PAL4_test")
        };
        Self {
            app,
            root_path,
            app_name: app_name.to_owned(),
            agent,
            agent_server: RefCell::new(None),
        }
    }
}

/// Lazy-install a single global `AgentLogSink` so multiple boots in
/// the same process (e.g. integration tests) share the same ring
/// buffer instead of fighting over `log::set_logger`.
fn install_global_log_sink() -> Option<&'static AgentLogSink> {
    static SINK: OnceLock<&'static AgentLogSink> = OnceLock::new();
    if let Some(s) = SINK.get() {
        return Some(*s);
    }
    match AgentLogSink::new(4096).install() {
        Ok(s) => {
            let _ = SINK.set(s);
            Some(s)
        }
        Err(_) => {
            // The host already installed `simple_logger`; the agent
            // log endpoint will return an empty page in that case.
            log::warn!("agent_server: global logger already installed; /v1/log/tail will be empty");
            None
        }
    }
}

fn start_agent_server(
    opts: &AgentBootOptions,
    bridge: &Rc<Pal4AgentBridge>,
    log_sink: Option<&'static AgentLogSink>,
) -> Result<AgentServer, String> {
    let mut config = AgentServerConfig::loopback(opts.port);
    if let Some(ip) = opts.bind_ip {
        config = config.with_bind(std::net::SocketAddr::new(ip, opts.port))?;
    }
    if let Some(token) = opts.token.clone() {
        config = config.with_token(token);
    }
    AgentServer::start(config, &bridge.queue, log_sink)
}
