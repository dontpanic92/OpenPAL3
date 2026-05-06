use crosscom::ComRc;
use radiance::application::Application;
use radiance::comdef::{IApplication, IApplicationLoaderComponent};
use radiance_editor::application::EditorApplicationLoader;
use shared::GameType;
use yaobow_editor::config;
use yaobow_editor::directors::ScriptedWelcomePage;

fn main() {
    let logger = simple_logger::SimpleLogger::new();

    // workaround panic on Linux for 'Could not determine the UTC offset on this system'
    // see: https://github.com/borntyping/rust-simple_logger/issues/47
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
    let logger = logger.with_utc_timestamps();

    logger.init().unwrap();

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

    let app = ComRc::<IApplication>::from_object(Application::new());
    app.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(EditorApplicationLoader::new(
            app.clone(),
            ScriptedWelcomePage::create(app.clone()),
        )),
    );

    config::init_imgui_ini(&app);

    app.initialize();
    app.run();
}
