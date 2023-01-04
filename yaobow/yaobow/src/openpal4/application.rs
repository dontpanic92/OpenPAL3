use std::path::PathBuf;

use radiance::application::{Application, ApplicationExtension};

pub struct OpenPal4Application {
    root_path: PathBuf,
    app_name: String,
}

impl ApplicationExtension<OpenPal4Application> for OpenPal4Application {
    fn on_initialized(&mut self, app: &mut Application<OpenPal4Application>) {
        let logger = simple_logger::SimpleLogger::new();
        // workaround panic on Linux for 'Could not determine the UTC offset on this system'
        // see: https://github.com/borntyping/rust-simple_logger/issues/47
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
        let logger = logger.with_utc_timestamps();
        logger.init().unwrap();

        app.set_title(&self.app_name);

        let input_engine = app.engine_mut().input_engine();
        let audio_engine = app.engine_mut().audio_engine();
    }

    fn on_updating(&mut self, _app: &mut Application<OpenPal4Application>, _delta_sec: f32) {}
}

impl OpenPal4Application {
    pub fn create(app_name: &str) -> Application<OpenPal4Application> {
        Application::new(Self::new(app_name))
    }

    fn new(app_name: &str) -> Self {
        OpenPal4Application {
            root_path: PathBuf::from(""),
            app_name: app_name.to_owned(),
        }
    }
}
