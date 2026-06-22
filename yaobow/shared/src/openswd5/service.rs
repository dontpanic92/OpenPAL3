//! SWD5-family launch service — phase-2 replacement for
//! `Swd5LaunchContext`. Mirrors the `Pal4Service` shape exactly: an
//! app-lifetime singleton exposed via `IYaobowHostContext.swd5()` that
//! returns a fully-wired `IDirector` for the requested game.
//!
//! `create_director(asset_path, game_ordinal)` accepts the per-game
//! ordinal because SWD5 / SWDHC / SWDCF share infrastructure but feed
//! the asset loader different `GameType` discriminators (texture
//! resolver branch, asset-table selection).
//!
//! ## Agent server
//!
//! When `--swd5 --agent-port` is passed, the loader installs an
//! [`AgentBridge`] via [`Swd5Service::set_agent_bridge`] *before* the
//! first `create_director`. The service then (1) hands the bridge's
//! synthetic-input overlay to the director so `/v1/input/*` reaches
//! the Lua VM, (2) plumbs the bridge into the director so pause / step
//! / fast-forward take effect, and (3) drains the agent command queue
//! once per frame via [`Swd5Service::pump_agent`].

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use packfs::init_virtual_fs;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use radiance::input::{InputEngine, SyntheticInputBridge};
use radiance::scene::CoreScene;

use crate::GameType;
use crate::agent_common::AgentBridge;
use crate::openswd5::agent::{Swd5DispatchCtx, dispatch_swd5_command};
use crate::openswd5::asset_loader::AssetLoader;
use crate::openswd5::comdef::{ISwd5Service, ISwd5ServiceImpl};
use crate::openswd5::director::OpenSWD5Director;

pub struct Swd5Service {
    app: ComRc<IApplication>,
    /// Agent-server bridge. `None` when no `--agent-port` flag was
    /// passed; `Some(_)` enables [`Self::pump_agent`] and makes every
    /// constructed `OpenSWD5Director` honour pause/step + see
    /// synthetic input.
    agent_bridge: RefCell<Option<Rc<AgentBridge>>>,
}

ComObject_Swd5Service!(super::Swd5Service);

impl Swd5Service {
    pub fn create(app: ComRc<IApplication>) -> ComRc<ISwd5Service> {
        ComRc::from_object(Self {
            app,
            agent_bridge: RefCell::new(None),
        })
    }

    /// Install the agent bridge so the next `create_director` plumbs
    /// synthetic input + pause/step gating into the new director.
    /// Called once at boot by `YaobowApplicationLoader` when
    /// `--swd5 --agent-port` is in effect.
    pub fn set_agent_bridge(&self, bridge: Rc<AgentBridge>) {
        *self.agent_bridge.borrow_mut() = Some(bridge);
    }

    /// Resolve the `Rc<RefCell<dyn InputEngine>>` to hand the next
    /// director: the agent's synthetic-input bridge when present (so
    /// `/v1/input/*` reaches the Lua VM), otherwise the real engine
    /// input.
    fn input_engine_for_director(&self) -> Rc<RefCell<dyn InputEngine>> {
        if let Some(bridge) = self.agent_bridge.borrow().as_ref() {
            // Rc<RefCell<SyntheticInputBridge>> coerces to
            // Rc<RefCell<dyn InputEngine>> at the binding site.
            let synth: Rc<RefCell<SyntheticInputBridge>> = bridge.input_bridge.clone();
            return synth;
        }
        self.app.engine().borrow().input_engine()
    }

    /// Drain the agent-server command queue, dispatch each command
    /// against SWD5 state, then publish frame telemetry and clear
    /// synthetic-input edges. Called once per frame by
    /// `YaobowApplicationLoader::on_updating` (before the engine
    /// tick), so commands land before the active director runs.
    ///
    /// No-op unless a bridge was installed (`--swd5 --agent-port`).
    pub fn pump_agent(&self, delta_sec: f32) {
        let bridge = match self.agent_bridge.borrow().as_ref() {
            Some(b) => b.clone(),
            None => return,
        };

        // Lazily wire the rendering engine so `/v1/screenshot` works.
        if bridge.rendering_engine.borrow().is_none() {
            let engine = self.app.engine().borrow().rendering_engine();
            bridge.set_rendering_engine(engine);
        }

        // Drain the queue once (single drainer for this game).
        let mut envelopes = Vec::new();
        if let Some(consumer) = bridge.consumer.borrow().as_ref() {
            consumer.drain(|env| envelopes.push(env));
        }

        if !envelopes.is_empty() {
            // Resolve the active director and clone out its context
            // handle for snapshot reads. SWD5 only ever installs an
            // `OpenSWD5Director` (no menu / title mode), so the
            // `inner` downcast is sound whenever a director exists.
            let scene_manager = self.app.engine().borrow().scene_manager().clone();
            let context = scene_manager
                .director()
                .map(|d| d.inner::<OpenSWD5Director>().context());

            for env in envelopes {
                let ctx = Swd5DispatchCtx {
                    bridge: &bridge,
                    context: context.clone(),
                };
                let response = dispatch_swd5_command(&ctx, env.command.clone());
                env.reply(response);
            }
        }

        // Telemetry: always advance frame counter + publish dt/fps.
        bridge.publish_frame_telemetry(delta_sec);

        // Clear synthetic-input edges. Done here (before the engine
        // tick) because the director's `update` runs *during*
        // `engine.update`, so taps injected this frame are observable
        // by the VM's input polls and cleared at the next pump.
        bridge.input_bridge.borrow().end_frame();
    }
}

impl ISwd5ServiceImpl for Swd5Service {
    fn create_director(
        &self,
        asset_path: &str,
        game_ordinal: std::os::raw::c_int,
    ) -> ComRc<IDirector> {
        let game =
            radiance_scripting::services::game_registry::ordinal_to_config_key(game_ordinal as i32)
                .and_then(GameType::from_config_key)
                .unwrap_or(GameType::SWDHC);

        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let component_factory = engine.rendering_component_factory();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager().clone();
        let ui = engine.ui_manager();
        drop(engine);

        // Synthetic-input overlay when the agent server is enabled,
        // otherwise the real engine input.
        let input_engine = self.input_engine_for_director();

        let asset_path = PathBuf::from(asset_path);
        let vfs = init_virtual_fs(asset_path.to_str().unwrap_or(""), None);
        let loader = AssetLoader::new(component_factory.clone(), Rc::new(vfs), game);

        // Push an empty initial scene so the Lua VM's first tick sees
        // a valid scene-manager state. Matches today's loader.
        scene_manager.push_scene(CoreScene::create());

        let director = OpenSWD5Director::new(
            loader,
            input_engine,
            scene_manager,
            audio_engine,
            component_factory,
            ui,
            self.agent_bridge.borrow().clone(),
        );

        ComRc::<IDirector>::from_object(director)
    }
}
