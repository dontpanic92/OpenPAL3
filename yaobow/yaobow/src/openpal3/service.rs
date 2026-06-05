//! PAL3 launch service — phase-2 replacement for direct
//! `OpenPal3ApplicationLoader` construction.
//!
//! `IPal3Service` interface lives in `shared::openpal3::comdef` (from
//! `crosscom/idl/openpal3.idl`) so it can be exposed symmetrically via
//! `IYaobowHostContext.pal3()`. The conforming `class Pal3Service` is
//! declared in `yaobow_services.idl` so the `ComObject_Pal3Service!`
//! macro generates in `yaobow_lib` — where the PAL3 director
//! construction (`MainMenuDirector`, `OpenPal3DebugLayer`,
//! `sce_proc_hooks`) currently lives. This split avoids moving four
//! PAL3-specific files into `shared` purely for the migration.

use std::path::PathBuf;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IApplication, IApplicationExt, IDirector};
use shared::openpal3::asset_manager::AssetManager;
use shared::openpal3::comdef::{IPal3Service, IPal3ServiceImpl};

use crate::openpal3::debug_layer::OpenPal3DebugLayer;
use crate::openpal3::main_menu_director::MainMenuDirector;

pub struct Pal3Service {
    app: ComRc<IApplication>,
}

ComObject_Pal3Service!(super::Pal3Service);

impl Pal3Service {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IPal3Service> {
        ComRc::from_object(Self { app })
    }
}

impl IPal3ServiceImpl for Pal3Service {
    fn create_director(&self, asset_path: &str) -> ComRc<IDirector> {
        let engine_rc = self.app.engine();
        let engine = engine_rc.borrow();
        let input_engine = engine.input_engine();
        let audio_engine = engine.audio_engine();
        let scene_manager = engine.scene_manager();
        let ui = engine.ui_manager();
        let component_factory = engine.rendering_component_factory();
        drop(engine);

        let root_path = PathBuf::from(asset_path);
        let vfs = packfs::init_virtual_fs(&root_path, None);
        let asset_mgr = Rc::new(AssetManager::new(component_factory, Rc::new(vfs)));

        // Installing the debug layer mutates engine state but doesn't
        // trigger a director swap, so it's safe to call here even when
        // create_director is invoked from inside title.update.
        let debug_layer =
            OpenPal3DebugLayer::new(input_engine.clone(), scene_manager.clone(), ui.clone());
        self.app
            .engine()
            .borrow_mut()
            .set_debug_layer(Box::new(debug_layer));

        let director = MainMenuDirector::new(
            asset_mgr.clone(),
            audio_engine,
            input_engine,
            scene_manager,
            ui,
        );

        ComRc::<IDirector>::from_object(director)
    }
}
