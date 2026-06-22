//! PAL5 launch service — host-side singleton that builds the PAL5
//! story director and hosts the agent-server bridge.
//!
//! `IPal5Service` lives in `shared::openpal5::comdef` (from
//! `crosscom/idl/openpal5.idl`) so it can be exposed symmetrically via
//! `IYaobowHostContext.pal5()`. The conforming `class Pal5Service` is
//! declared in `yaobow_services.idl` so the `ComObject_Pal5Service!`
//! macro generates here in `yaobow_lib`, where the PAL5 story runtime
//! (`Pal5StoryDirector`, `Pal5ScriptContext`, agent dispatch) lives.
//! This mirrors `Pal3Service` and `Swd5Service`.
//!
//! Both the title-selector launch (`title.p7` → `host.pal5()
//! .create_director`) and the CLI direct-boot (`--pal5`) go through
//! `create_director`, so there is a single source of truth for PAL5
//! director construction.
//!
//! ## Agent server
//!
//! When `--pal5 --agent-port` is passed, the loader installs an
//! [`AgentBridge`] via [`Pal5Service::set_agent_bridge`] *before* the
//! first `create_director`. The service then (1) hands the bridge's
//! synthetic-input overlay to the director so `/v1/input/*` reaches the
//! Lua VM, (2) plumbs the bridge into the director so pause / step /
//! fast-forward take effect, and (3) drains the agent command queue
//! once per frame via [`Pal5Service::pump_agent`].

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};

use shared::GameType;
use shared::agent_common::AgentBridge;
use shared::openpal5::comdef::{IPal5Service, IPal5ServiceImpl};

use super::agent::{Pal5DispatchCtx, dispatch_pal5_command};
use super::director::Pal5StoryDirector;

pub struct Pal5Service {
    app: ComRc<IApplication>,
    /// Agent-server bridge. `None` when no `--agent-port` flag was
    /// passed; `Some(_)` enables [`Self::pump_agent`] and makes every
    /// constructed `Pal5StoryDirector` honour pause/step + see
    /// synthetic input.
    agent_bridge: RefCell<Option<Rc<AgentBridge>>>,
}

ComObject_Pal5Service!(super::Pal5Service);

impl Pal5Service {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IPal5Service> {
        ComRc::from_object(Self {
            app,
            agent_bridge: RefCell::new(None),
        })
    }

    /// Install the agent bridge so the next `create_director` plumbs
    /// synthetic input + pause/step gating into the new director.
    /// Called once at boot by `YaobowApplicationLoader` when
    /// `--pal5 --agent-port` is in effect.
    pub fn set_agent_bridge(&self, bridge: Rc<AgentBridge>) {
        *self.agent_bridge.borrow_mut() = Some(bridge);
    }

    /// Drain the agent-server command queue, dispatch each command
    /// against PAL5 state, then publish frame telemetry and clear
    /// synthetic-input edges. Called once per frame by
    /// `YaobowApplicationLoader::on_updating` (before the engine
    /// tick), so commands land before the active director runs.
    ///
    /// No-op unless a bridge was installed (`--pal5 --agent-port`).
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
            // handle for snapshot reads. PAL5 only ever installs a
            // `Pal5StoryDirector` (no menu / title mode); the QI to
            // `IPal5StoryDirector` keeps the `inner` downcast sound.
            let scene_manager = self.app.engine().borrow().scene_manager().clone();
            let context = scene_manager
                .director()
                .and_then(|d| d.query_interface::<crate::comdef::IPal5StoryDirector>())
                .map(|d| d.inner::<Pal5StoryDirector>().context());

            for env in envelopes {
                let ctx = Pal5DispatchCtx {
                    bridge: &bridge,
                    context: context.clone(),
                };
                let response = dispatch_pal5_command(&ctx, env.command.clone());
                env.reply(response);
            }
        }

        // Telemetry: always advance frame counter + publish dt/fps.
        bridge.publish_frame_telemetry(delta_sec);

        // Clear synthetic-input edges (before the engine tick; the
        // director's `update` runs during `engine.update`).
        bridge.input_bridge.borrow().end_frame();
    }
}

impl IPal5ServiceImpl for Pal5Service {
    fn create_director(
        &self,
        asset_path: &str,
        game_ordinal: std::os::raw::c_int,
    ) -> Option<ComRc<IDirector>> {
        let game =
            radiance_scripting::services::game_registry::ordinal_to_config_key(game_ordinal as i32)
                .and_then(GameType::from_config_key)
                .unwrap_or(GameType::PAL5);

        let bridge = self.agent_bridge.borrow().clone();
        super::create_story_director(self.app.clone(), asset_path, game, bridge)
    }
}
