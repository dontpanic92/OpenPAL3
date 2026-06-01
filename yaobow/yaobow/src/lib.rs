#![allow(unused_variables)]
#![allow(dead_code)]

#[macro_use]
pub mod comdef {
    include!(concat!(env!("OUT_DIR"), "/yaobow_comdef.rs"));

    // Mirror radiance_scripting's services namespace so cross-crate uses of
    // `radiance_scripting::ComObject_*!` macros (which expand `use crate as
    // radiance_scripting`) can resolve `radiance_scripting::comdef::services::*`
    // through this crate.
    pub mod services {
        pub use radiance_scripting::comdef::services::*;
    }

    // Generated comdef for the title-page foreign service surface
    // (`crosscom/idl/yaobow_services.idl`). Paired with the p7 binding
    // exposed via `script_source::YAOBOW_SERVICES_P7`.
    #[macro_use]
    pub mod yaobow_services {
        include!(concat!(env!("OUT_DIR"), "/yaobow_services_comdef.rs"));
    }
}

pub mod script_source {
    //! p7 script bundle for the yaobow main app. `app.p7` is the only
    //! root source; title and PAL4 debug scripts are sibling modules
    //! reached through the rooted `YaobowScriptApp` object.

    use std::cell::RefCell;
    use std::rc::Rc;

    use crosscom::ComRc;
    use p7::interpreter::context::Data;
    use radiance::comdef::{
        IApplication, IApplicationExt, ICameraControl, IDirector, IEntity, IImmediateDirector,
        IRayCaster,
    };
    use radiance::radiance::CoreRadianceEngine;
    use radiance_scripting::comdef::services::IInputService;
    use radiance_scripting::{
        with_services, wrap_director, RuntimeAccess, RuntimeHandle, ScriptDirectorHandle,
        ScriptHost,
    };
    use shared::openpal4::actor_controller_script::wrap_actor_controller;
    use shared::openpal4::comdef::pal4_debug::{IPal4DebugContext, IPal4DebugOverlay};
    use shared::openpal4::comdef::{
        IPal4ActorAnimationController, IPal4ActorController, IPal4GameContext,
    };
    use shared::openpal4::pal4_debug::wrap_overlay;
    use shared::openpal4::scene::Pal4ActorControllerFactory;
    use shared::GameType;

    use crate::application::yaobow_app_context::YaobowAppContext;
    use radiance_scripting::comdef::services::IHostContext;

    /// p7 binding source for `yaobow_services.idl`. Register it with
    /// `ScriptHost::add_binding("yaobow_services", YAOBOW_SERVICES_P7)`
    /// before loading any yaobow script that imports the module.
    pub const YAOBOW_SERVICES_P7: &str =
        include_str!(concat!(env!("OUT_DIR"), "/yaobow_services.p7"));

    pub const APP_P7: &str = include_str!("../scripts/app.p7");
    pub const PAL4_DEBUG_P7: &str = shared::openpal4::pal4_debug::PAL4_DEBUG_P7;
    pub const ACTOR_CONTROLLER_P7: &str =
        shared::openpal4::actor_controller_script::ACTOR_CONTROLLER_P7;

    #[derive(Clone, Copy)]
    pub struct ScriptModule {
        pub name: &'static str,
        pub source: &'static str,
    }

    pub struct ScriptPackage {
        pub root_name: &'static str,
        pub root_source: &'static str,
        pub idl_bindings: &'static [ScriptModule],
        pub modules: &'static [ScriptModule],
    }

    pub const IDL_BINDINGS: &[ScriptModule] = &[
        ScriptModule {
            name: "yaobow_services",
            source: YAOBOW_SERVICES_P7,
        },
        ScriptModule {
            name: "pal4_debug",
            source: PAL4_DEBUG_P7,
        },
        ScriptModule {
            name: "openpal4",
            source: shared::openpal4::actor_controller_script::OPENPAL4_P7,
        },
    ];

    /// Sibling modules referenced by `app.p7`. The first element is
    /// the module path (as it appears in `import` statements), the
    /// second is the p7 source.
    pub const SIBLING_MODULES: &[ScriptModule] = &[
        ScriptModule {
            name: "title_consts",
            source: include_str!("../scripts/title_consts.p7"),
        },
        ScriptModule {
            name: "title",
            source: include_str!("../scripts/title.p7"),
        },
        ScriptModule {
            name: "pal4_debug_overlay",
            source: include_str!("../scripts/pal4_debug_overlay.p7"),
        },
        ScriptModule {
            name: "actor_controller",
            source: ACTOR_CONTROLLER_P7,
        },
    ];

    pub const YAOBOW_PACKAGE: ScriptPackage = ScriptPackage {
        root_name: "app",
        root_source: APP_P7,
        idl_bindings: IDL_BINDINGS,
        modules: SIBLING_MODULES,
    };

    /// Registers every sibling module with `host` via `add_binding`.
    /// After this, callers load `APP_P7` to compile the app root.
    /// Bindings survive `ScriptHost::reload`, but a host that fully
    /// recreates its `ScriptHost` must call this again.
    pub fn register_yaobow_modules(host: &ScriptHost) {
        for module in YAOBOW_PACKAGE.modules {
            host.add_binding(module.name, module.source);
        }
    }

    pub fn register_yaobow_project(host: &ScriptHost) {
        validate_package(&YAOBOW_PACKAGE).expect("yaobow script package manifest must be valid");
        for binding in YAOBOW_PACKAGE.idl_bindings {
            host.add_binding(binding.name, binding.source);
        }
        register_yaobow_modules(host);
    }

    pub fn ensure_yaobow_project_loaded(host: &ScriptHost) {
        register_yaobow_project(host);
        if !host.has_function("init") {
            host.load_source(YAOBOW_PACKAGE.root_source)
                .expect("yaobow app script project must load successfully");
        }
    }

    pub fn validate_package(package: &ScriptPackage) -> Result<(), String> {
        if package.root_name.is_empty() {
            return Err("script package root name must not be empty".to_string());
        }

        let all_groups = [package.idl_bindings, package.modules];
        for (group_idx, group) in all_groups.iter().enumerate() {
            for (idx, module) in group.iter().enumerate() {
                if module.name.is_empty() {
                    return Err(format!(
                        "script package module at group {group_idx} index {idx} has empty name"
                    ));
                }
            }
        }

        for left in package.idl_bindings {
            for right in package.modules {
                if left.name == right.name {
                    return Err(format!("duplicate script module '{}'", left.name));
                }
            }
        }

        for group in all_groups {
            for i in 0..group.len() {
                for j in (i + 1)..group.len() {
                    if group[i].name == group[j].name {
                        return Err(format!("duplicate script module '{}'", group[i].name));
                    }
                }
            }
        }

        Ok(())
    }

    pub struct YaobowScriptProject {
        host: Rc<ScriptHost>,
        app: ScriptDirectorHandle,
        selected_game: Rc<RefCell<Option<GameType>>>,
    }

    impl YaobowScriptProject {
        /// App-lifetime install. Constructs the `YaobowAppContext` and
        /// the `selected_game` slot internally, so callers no longer
        /// build their own context. Idempotent — every subsequent call
        /// from any feature (PAL3/PAL4/…) returns the same project.
        pub fn install(
            app: &ComRc<IApplication>,
            config: Rc<RefCell<shared::config::YaobowConfig>>,
        ) -> Rc<Self> {
            let engine_rc = app.engine();
            let engine = engine_rc.borrow();
            Self::install_inner(&engine, || {
                let selected_game: Rc<RefCell<Option<GameType>>> = Rc::new(RefCell::new(None));
                let app_ctx = YaobowAppContext::create(app.clone(), selected_game.clone(), config);
                (app_ctx, selected_game)
            })
        }

        /// Lower-level installer used by tests and integration paths
        /// that build a custom `IHostContext` (e.g. with stub
        /// services). The supplied `selected_game` slot is exposed via
        /// [`Self::selected_game`].
        pub fn install_with_context(
            engine: &CoreRadianceEngine,
            app_ctx: ComRc<IHostContext>,
            selected_game: Rc<RefCell<Option<GameType>>>,
        ) -> Rc<Self> {
            Self::install_inner(engine, || (app_ctx, selected_game))
        }

        fn install_inner<F>(engine: &CoreRadianceEngine, build_ctx: F) -> Rc<Self>
        where
            F: FnOnce() -> (ComRc<IHostContext>, Rc<RefCell<Option<GameType>>>),
        {
            engine.get_or_insert_service(|| {
                let host = ScriptHost::install(engine);
                ensure_yaobow_project_loaded(&host);
                let (app_ctx, selected_game) = build_ctx();
                let app_ctx_id = host.intern(app_ctx);
                let app_ctx_box = host
                    .foreign_box(
                        "radiance_scripting.comdef.services.IHostContext",
                        app_ctx_id,
                    )
                    .expect("IHostContext foreign box must construct");
                let app_data = host
                    .call_returning_data("init", vec![app_ctx_box])
                    .expect("yaobow app script init must succeed");
                let app = host.root(app_data);
                Self {
                    host,
                    app,
                    selected_game,
                }
            })
        }

        pub fn host(&self) -> Rc<ScriptHost> {
            self.host.clone()
        }

        /// The shared `Rc<RefCell<Option<GameType>>>` slot. Title-side
        /// `IAppService.open_game(ordinal)` writes this slot; the
        /// `YaobowApplicationLoader::on_updating` poll reads and
        /// clears it on the next tick to swap the active per-game
        /// loader.
        pub fn selected_game(&self) -> Rc<RefCell<Option<GameType>>> {
            self.selected_game.clone()
        }

        pub fn make_title_director(&self) -> Result<ComRc<IImmediateDirector>, String> {
            // Fat CCW supports QI for every interface the script
            // struct conforms to; wrap as IDirector and QI to
            // IImmediateDirector.
            let d = self.make_title_director_as_director()?;
            d.query_interface::<IImmediateDirector>()
                .ok_or_else(|| "title director did not implement IImmediateDirector".to_string())
        }

        pub fn make_title_director_as_director(&self) -> Result<ComRc<IDirector>, String> {
            let director = self.call_app_method("make_title_director", Vec::new())?;
            wrap_director(&self.runtime_handle(), director).map_err(|err| format!("{err:?}"))
        }

        pub fn make_pal4_debug_overlay(
            &self,
            ctx: ComRc<IPal4DebugContext>,
        ) -> Result<ComRc<IPal4DebugOverlay>, String> {
            let ctx_id = self.host.intern(ctx);
            let ctx_box = self
                .host
                .foreign_box(
                    "shared.openpal4.comdef.pal4_debug.IPal4DebugContext",
                    ctx_id,
                )
                .map_err(|err| format!("{err:?}"))?;
            let overlay = self.call_app_method("make_pal4_debug_overlay", vec![ctx_box])?;
            wrap_overlay(&self.runtime_handle(), overlay).map_err(|err| format!("{err:?}"))
        }

        /// Mint a scripted party-wide `IPal4ActorController` covering
        /// all four party members. Calls
        /// `app.make_actor_controller(game_ctx, input, entity_0..3,
        /// anim_0..3, camera, ray_caster)` — interns each ComRc,
        /// invokes the script method, reverse-wraps the returned p7 box
        /// back into a `ComRc<IPal4ActorController>`.
        pub fn make_actor_controller(
            &self,
            game_ctx: ComRc<IPal4GameContext>,
            input: ComRc<IInputService>,
            entities: [ComRc<IEntity>; 4],
            anims: [ComRc<IPal4ActorAnimationController>; 4],
            camera: ComRc<ICameraControl>,
            ray_caster: ComRc<IRayCaster>,
        ) -> Result<ComRc<IPal4ActorController>, String> {
            let game_ctx_box =
                self.intern_box("shared.openpal4.comdef.IPal4GameContext", game_ctx)?;
            let input_box =
                self.intern_box("radiance_scripting.comdef.services.IInputService", input)?;
            let [e0, e1, e2, e3] = entities;
            let entity_0_box = self.intern_box("radiance.comdef.IEntity", e0)?;
            let entity_1_box = self.intern_box("radiance.comdef.IEntity", e1)?;
            let entity_2_box = self.intern_box("radiance.comdef.IEntity", e2)?;
            let entity_3_box = self.intern_box("radiance.comdef.IEntity", e3)?;
            let [a0, a1, a2, a3] = anims;
            let anim_0_box =
                self.intern_box("shared.openpal4.comdef.IPal4ActorAnimationController", a0)?;
            let anim_1_box =
                self.intern_box("shared.openpal4.comdef.IPal4ActorAnimationController", a1)?;
            let anim_2_box =
                self.intern_box("shared.openpal4.comdef.IPal4ActorAnimationController", a2)?;
            let anim_3_box =
                self.intern_box("shared.openpal4.comdef.IPal4ActorAnimationController", a3)?;
            let camera_box = self.intern_box("radiance.comdef.ICameraControl", camera)?;
            let ray_caster_box = self.intern_box("radiance.comdef.IRayCaster", ray_caster)?;
            let controller = self.call_app_method(
                "make_actor_controller",
                vec![
                    game_ctx_box,
                    input_box,
                    entity_0_box,
                    entity_1_box,
                    entity_2_box,
                    entity_3_box,
                    anim_0_box,
                    anim_1_box,
                    anim_2_box,
                    anim_3_box,
                    camera_box,
                    ray_caster_box,
                ],
            )?;
            wrap_actor_controller(&self.runtime_handle(), controller)
                .map_err(|err| format!("{err:?}"))
        }

        /// Intern a `ComRc<I>` and wrap it as a script foreign-box
        /// argument. The `type_tag` must match the IDL-derived module
        /// path used in the script bridge (e.g.
        /// `radiance.comdef.IEntity`).
        fn intern_box<I>(
            &self,
            type_tag: &str,
            rc: ComRc<I>,
        ) -> Result<p7::interpreter::context::Data, String>
        where
            I: ::crosscom::ComInterface + 'static,
        {
            let id = self.host.intern(rc);
            self.host
                .foreign_box(type_tag, id)
                .map_err(|err| format!("{err:?}"))
        }

        /// Builds an `Rc<dyn Pal4ActorControllerFactory>` suitable for
        /// `Pal4AppContext::set_actor_controller_factory`. The factory
        /// keeps a strong `Rc<YaobowScriptProject>` so the script host
        /// stays installed for the controllers' lifetime.
        pub fn actor_controller_factory(self: &Rc<Self>) -> Rc<dyn Pal4ActorControllerFactory> {
            Rc::new(YaobowActorControllerFactory {
                project: self.clone(),
            })
        }

        /// One-shot helper for PAL4: builds a fresh debug session
        /// (Rust-side context + state) and asks the script app to
        /// create the overlay against it. Returns the bundle the
        /// `OpenPAL4Director` keeps and dispatches each frame.
        pub fn make_pal4_debug_bundle(&self) -> shared::openpal4::director::Pal4DebugBundle {
            let session = shared::openpal4::pal4_debug::create_debug_session();
            let overlay = self
                .make_pal4_debug_overlay(session.context.clone())
                .expect("pal4_debug overlay creation must succeed");
            shared::openpal4::director::Pal4DebugBundle {
                overlay,
                overlay_ctx: session.context,
                debug_state: session.state,
            }
        }

        fn call_app_method(&self, method_name: &str, args: Vec<Data>) -> Result<Data, String> {
            let app = self.host.deref_handle(self.app).ok_or_else(|| {
                "yaobow script app root was invalidated by ScriptHost::reload".to_string()
            })?;
            self.host
                .call_method_returning_data(app, method_name, args)
                .map_err(|err| format!("{err:?}"))
        }

        fn runtime_handle(&self) -> RuntimeHandle {
            let mut out = None;
            <ScriptHost as RuntimeAccess>::with_ctx(&self.host, &mut |_ctx| {
                let h = with_services(|s| s.runtime_handle())
                    .expect("with_services inside RuntimeAccess scope");
                out = Some(h);
            });
            out.expect("RuntimeAccess::with_ctx ran body")
        }
    }

    /// `Pal4ActorControllerFactory` impl that defers to a
    /// `YaobowScriptProject`. Held as `Rc<dyn …>` by `Pal4AppContext`
    /// so the script host stays alive for every minted controller.
    struct YaobowActorControllerFactory {
        project: Rc<YaobowScriptProject>,
    }

    impl Pal4ActorControllerFactory for YaobowActorControllerFactory {
        fn make_actor_controller(
            &self,
            game_ctx: ComRc<IPal4GameContext>,
            input: ComRc<IInputService>,
            entities: [ComRc<IEntity>; 4],
            anims: [ComRc<IPal4ActorAnimationController>; 4],
            camera: ComRc<ICameraControl>,
            ray_caster: ComRc<IRayCaster>,
        ) -> ComRc<IPal4ActorController> {
            self.project
                .make_actor_controller(game_ctx, input, entities, anims, camera, ray_caster)
                .expect("scripted Pal4PartyController creation must succeed")
        }
    }
}

pub mod application;
pub mod opengujian;
pub mod openpal3;
pub mod openpal4;
pub mod openpal5;
pub mod openswd5;

pub use application::run_title_selection;
pub use opengujian::run_opengujian;
pub use openpal3::run_openpal3;
pub use openpal4::application::AgentBootOptions as Pal4AgentBootOptions;
pub use openpal4::{run_openpal4, run_openpal4_with_agent};
pub use openpal5::run_openpal5;
pub use openswd5::run_openswd5;

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
pub fn android_entry() {
    openpal3::run_openpal3();
}
