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
    // bundled into the yaobow script package at build time and
    // surfaced through `script_source::package()`.
    #[macro_use]
    pub mod yaobow_services {
        include!(concat!(env!("OUT_DIR"), "/yaobow_services_comdef.rs"));
    }
}

/// Auto-generated script bridges. The local `yaobow_services` bridge
/// is just the `register_yaobow_script_app_proto` / `wrap_yaobow_script_app`
/// pair (the IDL-derived ProtoSpec metadata + the single reverse-wrap
/// entry point used by `install_inner`); it is `include!`d from
/// `OUT_DIR`. Cross-IDL deps are re-exported under the paths the codegen
/// generates for imported IDL stems
/// (`crate::script_bridges::<idl_stem>::<fn>`).
pub mod script_bridges {
    pub mod yaobow_services {
        include!(concat!(env!("OUT_DIR"), "/yaobow_services_bridge.rs"));
    }

    pub mod radiance {
        pub use radiance_scripting::script_bridges::radiance::*;
    }

    pub mod crosscom {
        pub use radiance_scripting::script_bridges::crosscom::*;
    }

    pub mod openpal3 {
        pub use shared::script_bridges::openpal3::*;
    }

    pub mod openpal4 {
        pub use shared::script_bridges::openpal4::*;
    }

    pub mod pal4_debug {
        pub use shared::script_bridges::pal4_debug::*;
    }
}

pub mod script_source {
    //! p7 script bundle for the yaobow main app. `app.p7` is the only
    //! root source; title and PAL4 debug scripts are sibling modules
    //! reached through the reverse-wrapped `YaobowScriptApp` object.
    //!
    //! `app.p7` conforms a single struct to the `IYaobowScriptApp`
    //! proto (`crosscom/idl/yaobow_services.idl`). Rust reverse-wraps it
    //! into a `ComRc<IYaobowScriptApp>` and calls its `make_*` factory
    //! methods through the COM vtable — no typed client, no per-method
    //! marshalling glue.

    use std::cell::RefCell;
    use std::rc::Rc;

    use crosscom::ComRc;
    use radiance::comdef::{IApplication, IApplicationExt};
    use radiance_scripting::{ScriptHost, bootstrap_script_root_from_path};

    use crate::application::yaobow_host_context::{
        YAOBOW_HOST_CONTEXT_TYPE_TAG, YaobowHostContext,
    };
    use crate::comdef::yaobow_services::{IYaobowHostContext, IYaobowScriptApp};
    use crate::openpal3::Pal3Service;

    /// In-binary `.ypk` produced by `build.rs` from `scripts/*.p7` +
    /// the codegen-derived `yaobow_services.p7`. Mounted at `/yaobow/`
    /// on the script `AssetManager` by [`mount_scripts`].
    const YAOBOW_YPK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/yaobow.ypk"));

    /// Mounts this crate's `yaobow.ypk` at `/yaobow/` on the script
    /// `AssetManager`, so scripts can `import yaobow.title;`,
    /// `import yaobow.yaobow_services;`, etc. and the app root
    /// resolves at `/yaobow/app.p7`.
    pub fn mount_scripts(assets: &radiance::asset::AssetManager) {
        assets
            .mount_ypk_bytes("/yaobow", YAOBOW_YPK)
            .expect("yaobow.ypk must mount");
    }

    /// Construct the dedicated script `AssetManager` used by the
    /// yaobow binary: engine bindings + every contributing crate's
    /// ypk mounted under its prefix.
    pub fn install_script_assets() -> Rc<radiance::asset::AssetManager> {
        let assets = radiance::asset::AssetManager::new();
        radiance_scripting::mount_engine_bindings(&assets);
        radiance_scripting::mount_scripts(&assets);
        shared::mount_scripts(&assets);
        mount_scripts(&assets);
        // yaobow_editor mounts only when the editor binary is in use;
        // the title-selector / per-game launcher path doesn't need it.
        assets
    }

    /// Bootstrap the yaobow script root and return the two app-lifetime
    /// COM handles the loader holds for the whole process:
    ///   * `ComRc<IYaobowScriptApp>` — the reverse-wrapped app factory
    ///     (`make_title_director`, `make_pal5_director`). Holding it
    ///     keeps the script box rooted.
    ///   * `ComRc<IYaobowHostContext>` — the canonical host context,
    ///     handed to the script's `init` and used by the loader to reach
    ///     the per-game services during direct boot.
    ///
    /// Side effect: the script struct also conforms to
    /// `openpal4.IPal4ScriptFactory`; we register that ProtoSpec, QI the
    /// factory to it, and stash it on `Pal4Service::set_script_factory`
    /// so the PAL4 launch path (start menu, debug overlay, actor
    /// controllers) dispatches straight through the COM vtable.
    ///
    /// Called once from `YaobowApplicationLoader::on_loading`.
    pub fn install_script_factory(
        app: &ComRc<IApplication>,
        config: Rc<RefCell<shared::config::YaobowConfig>>,
    ) -> (ComRc<IYaobowScriptApp>, ComRc<IYaobowHostContext>) {
        let engine_rc = app.engine();
        let engine = engine_rc.borrow();

        // Build the per-game services + the host context handed to `init`.
        let pal3 = Pal3Service::create(app.clone());
        let pal4 = shared::openpal4::service::Pal4Service::create(app.clone());
        let pal5 = shared::openpal5::service::Pal5Service::create(
            app.clone(),
            engine.rendering_component_factory(),
        );
        let swd5 = shared::openswd5::service::Swd5Service::create(app.clone());
        let host_context =
            YaobowHostContext::create(app.clone(), config, pal3.clone(), pal4.clone(), pal5, swd5);

        let host = ScriptHost::install(&engine);
        // Install the dedicated script `AssetManager` so the VFS-backed
        // `ModuleProvider` can resolve every `import <crate>.<module>;`.
        host.set_script_assets(install_script_assets());
        let app_data = bootstrap_script_root_from_path(
            &host,
            "/yaobow/app.p7",
            host_context.clone(),
            YAOBOW_HOST_CONTEXT_TYPE_TAG,
            "init",
        )
        .expect("yaobow app script init must succeed");

        // The `app.p7` struct conforms to `IYaobowScriptApp`,
        // `openpal3.IPal3ScriptFactory`, and `openpal4.IPal4ScriptFactory`.
        // Register every conformed proto's ProtoSpec *before* wrapping
        // so the fat CCW gets a real QI slot for each.
        shared::script_bridges::openpal3::register_pal3_script_factory_proto();
        shared::script_bridges::openpal4::register_pal4_script_factory_proto();
        // Reverse-wrap the script app root as a real
        // `ComRc<IYaobowScriptApp>` (a proto-CCW). The CCW roots the
        // script box for its lifetime and unroots on final release.
        let factory = crate::script_bridges::yaobow_services::wrap_yaobow_script_app(
            &host.runtime_handle(),
            app_data,
        )
        .expect("reverse-wrap yaobow script app root must succeed");

        // Hand the PAL3 factory surface to Pal3Service (QI to the
        // shared `IPal3ScriptFactory` slot of the same fat CCW).
        let pal3_factory = factory
            .query_interface::<shared::openpal3::comdef::IPal3ScriptFactory>()
            .expect("script app must conform to IPal3ScriptFactory");
        pal3.inner::<crate::openpal3::Pal3Service>()
            .set_script_factory(pal3_factory);

        // Hand the PAL4 factory surface to Pal4Service (QI to the
        // shared `IPal4ScriptFactory` slot of the same fat CCW).
        let script_factory = factory
            .query_interface::<shared::openpal4::comdef::IPal4ScriptFactory>()
            .expect("script app must conform to IPal4ScriptFactory");
        pal4.inner::<shared::openpal4::service::Pal4Service>()
            .set_script_factory(script_factory);

        (factory, host_context)
    }
}

pub mod application;
pub mod openpal3;
pub mod openpal5;

pub use application::{
    BootOptions, Pal4AgentBootOptions, boot_for, create_application, resolve_asset_path, run_app,
    run_opengujian, run_openpal4, run_openpal4_with_agent, run_openpal5, run_openpal5q,
    run_openswd5, run_title_selection,
};
pub use openpal3::{run_openpal3, run_openpal3_with_agent};

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
pub fn android_entry() {
    openpal3::run_openpal3();
}
