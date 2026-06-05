//! PAL4 launch service — phase-2 replacement for `Pal4LaunchContext`.
//!
//! App-lifetime singleton installed by `YaobowApplicationLoader` and
//! exposed via `IYaobowHostContext.pal4()`. `create_director(path)`
//! returns a fully-wired `IDirector` (= `OpenPAL4Director`) with:
//!  * its AssetLoader bound to the per-launch vfs
//!  * the agent bridge attached (if the loader pre-stocked one via
//!    [`Pal4Service::set_agent_bridge`])
//!  * the debug-overlay bundle attached (script-built via the
//!    `YaobowScriptProject`'s `make_pal4_debug_bundle`)
//!  * the actor controller factory attached (script-built)
//!
//! ## Ownership graph
//!
//! `Pal4Service` is held by `YaobowHostContext` which is interned into
//! the script via `foreign_box`. The script root receives the host
//! context as `box<IYaobowHostContext>`. `YaobowScriptProject` is held
//! as an engine service. To avoid a Rc cycle
//! (`YaobowScriptProject` → `ScriptHost` → `ComObjectTable` →
//! `YaobowHostContext` → `Pal4Service` → `YaobowScriptProject`), the
//! reference back to the project here is a [`Weak`].
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
use std::rc::{Rc, Weak};

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance::input::{InputEngine, SyntheticInputBridge};

use crate::openpal4::agent::Pal4AgentBridge;
use crate::openpal4::asset_loader::AssetLoader;
use crate::openpal4::comdef::{IPal4Service, IPal4ServiceImpl};
use crate::openpal4::director::OpenPAL4Director;
use crate::openpal4::scene::Pal4ActorControllerFactory;

/// Trait the script project exposes for `Pal4Service` to mint the
/// PAL4 debug overlay bundle during `create_director`. Decouples
/// `shared::openpal4::service` from `yaobow_lib::script_source` so
/// `shared` doesn't need to know about `YaobowScriptProject`.
///
/// `yaobow_lib::script_source::YaobowScriptProject` implements this
/// trait; the service holds it as `Weak<dyn Pal4ScriptHooks>` to
/// break the project↔host-context↔service Rc cycle.
///
/// The actor-controller factory is NOT part of this trait — it
/// needs `Rc<YaobowScriptProject>` to clone the project into each
/// minted controller, which doesn't fit the `&self` trait shape.
/// Instead, `YaobowApplicationLoader` calls
/// [`Pal4Service::set_actor_controller_factory`] explicitly after
/// the script project is installed.
pub trait Pal4ScriptHooks {
    fn make_pal4_debug_bundle(&self) -> crate::openpal4::director::Pal4DebugBundle;
}

pub struct Pal4Service {
    app: ComRc<IApplication>,
    agent_bridge: RefCell<Option<Rc<Pal4AgentBridge>>>,
    /// Weak ref to the yaobow script project (for minting the debug
    /// overlay). Set after construction via
    /// [`Pal4Service::set_script_hooks`] because the project itself
    /// depends on the host context (which holds this service)
    /// existing first. Wrapped in `Option` because `Weak::new()`
    /// cannot mint a `Weak<dyn Trait>` without a concrete starting
    /// value on stable Rust.
    script_hooks: RefCell<Option<Weak<dyn Pal4ScriptHooks>>>,
    /// Scripted actor-controller factory. Set explicitly by
    /// `YaobowApplicationLoader` after the script project is
    /// installed. Cloned into each constructed director.
    actor_controller_factory: RefCell<Option<Rc<dyn Pal4ActorControllerFactory>>>,
}

ComObject_Pal4Service!(super::Pal4Service);

impl Pal4Service {
    /// App-lifetime install. All four fields are late-bound via
    /// setters because their construction depends on the host
    /// context (which holds this service) existing first.
    pub fn create(app: ComRc<IApplication>) -> ComRc<IPal4Service> {
        ComRc::from_object(Self {
            app,
            agent_bridge: RefCell::new(None),
            script_hooks: RefCell::new(None),
            actor_controller_factory: RefCell::new(None),
        })
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

    /// Install the script-project hook. Called by
    /// `YaobowApplicationLoader::on_loading` after
    /// `YaobowScriptProject::install` returns.
    pub fn set_script_hooks(&self, hooks: Weak<dyn Pal4ScriptHooks>) {
        *self.script_hooks.borrow_mut() = Some(hooks);
    }

    /// Install the scripted actor-controller factory. Called by
    /// `YaobowApplicationLoader::on_loading` after the script project
    /// is installed.
    pub fn set_actor_controller_factory(&self, factory: Rc<dyn Pal4ActorControllerFactory>) {
        *self.actor_controller_factory.borrow_mut() = Some(factory);
    }
}

impl IPal4ServiceImpl for Pal4Service {
    fn create_director(&self, asset_path: &str) -> ComRc<IDirector> {
        let hooks = self
            .script_hooks
            .borrow()
            .as_ref()
            .and_then(|w| w.upgrade())
            .expect(
                "Pal4Service::create_director called before YaobowScriptProject was installed \
                 (or after it was dropped). The YaobowApplicationLoader must call \
                 Pal4Service::set_script_hooks after installing the script project.",
            );

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
        // agent, the real input handle is used unchanged.
        let input_engine: Rc<RefCell<dyn InputEngine>> = match &agent_bridge {
            Some(bridge) => {
                let synth: Rc<RefCell<SyntheticInputBridge>> = bridge.input_bridge.clone();
                synth
            }
            None => real_input,
        };

        let vfs = init_virtual_fs(asset_path, None);
        let loader = AssetLoader::new(component_factory.clone(), input_engine.clone(), vfs);

        let director = OpenPAL4Director::new(
            component_factory.clone(),
            loader,
            scene_manager,
            ui,
            input_engine,
            audio_engine,
            task_manager,
        );

        director.set_debug_bundle(hooks.make_pal4_debug_bundle());
        if let Some(factory) = self.actor_controller_factory.borrow().clone() {
            director.set_actor_controller_factory(factory);
        }

        if let Some(bridge) = agent_bridge {
            bridge.set_rendering_engine(rendering_engine);
            director.set_agent_bridge(bridge);
        }

        ComRc::<IDirector>::from_object(director)
    }
}
