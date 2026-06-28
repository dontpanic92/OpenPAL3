use crosscom::ComRc;
use radiance::application::Application;
use radiance::comdef::{IApplication, IApplicationExt, IApplicationLoaderComponent};
use radiance_editor::application::EditorApplicationLoader;
use shared::GameType;
use shared::video::register_opengb_video_decoders;
use yaobow_editor::config;
use yaobow_editor::directors::ScriptedWelcomePage;

fn main() {
    let logger = simple_logger::SimpleLogger::new();

    // workaround panic on Linux for 'Could not determine the UTC offset on this system'
    // see: https://github.com/borntyping/rust-simple_logger/issues/47
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
    let logger = logger.with_utc_timestamps();

    logger.init().unwrap();

    // Register video codec decoders (Bik via ffmpeg) so the editor's
    // `IPreviewerHub.open_video` can actually construct a stream;
    // without this `radiance::video::create_stream` finds no entry in
    // `VIDEO_DECODER_MAP` and clicking a `.bik` resource silently
    // returns null.
    register_opengb_video_decoders();

    // let mut line = String::new();
    // let stdin = std::io::stdin();
    // stdin.lock().read_line(&mut line).unwrap();

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        let _game = match args[1].as_str() {
            "--pal4" => GameType::PAL4,
            "--pal5" => GameType::PAL5,
            "--pal5q" => GameType::PAL5Q,
            "--swd5" => GameType::SWD5,
            "--swdhc" => GameType::SWDHC,
            "--swdcf" => GameType::SWDCF,
            "--gujian" => GameType::Gujian,
            "--gujian2" => GameType::Gujian2,
            &_ => GameType::PAL3,
        };
    }

    let cfg = shared::config::YaobowConfig::load();
    let engine_options = radiance::rendering::RenderingEngineOptions {
        scene_scale_mode: match cfg.scene_scale_mode() {
            shared::config::SceneScaleMode::Native => radiance::rendering::SceneScaleMode::Native,
            shared::config::SceneScaleMode::Logical => radiance::rendering::SceneScaleMode::Logical,
        },
        logical_extent: None,
    };
    let app = ComRc::<IApplication>::from_object(Application::with_options(engine_options));

    // imgui ini + theme need the engine but must land BEFORE the
    // loader's `on_loading` so ScriptedWelcomePage::create sees a
    // configured imgui context. Register as engine-ready callbacks;
    // they fire on the first run-loop tick after the first-resumed
    // bootstrap, ahead of any component on_loading.
    {
        let app2 = app.clone();
        let master_volume = cfg.master_volume();
        app.add_engine_ready_callback(Box::new(move || {
            app2.engine()
                .borrow()
                .audio_engine()
                .set_master_volume(master_volume);
            config::init_imgui_ini(&app2);
            config::init_theme(&app2);
        }));
    }

    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(EditorApplicationLoader::new(
            app.clone(),
            // Welcome-page construction reads the engine — defer it
            // to `on_loading` (post-resumed) via the factory closure.
            Box::new(|app| ScriptedWelcomePage::create(app)),
        )),
    );

    app.initialize();
    app.run();
}
