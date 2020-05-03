use super::application::OpenGbApplication;
use super::config::OpenGbConfig;
use intercom::*;

com_library! {
    class Factory,
    interface Config,
    interface Application,
}

#[com_class(Config)]
pub struct Config(OpenGbConfig);

#[com_interface]
impl Config {
    pub fn new(config_name: &str, env_prefix: &str) -> Self { 
        Self {
            0: OpenGbConfig::load(config_name, env_prefix)
        }
    }
}

#[com_class(Factory)]
#[derive(Default)]
struct Factory;

#[com_interface]
impl Factory {
    pub fn load_opengb_config(&self, name: &str, env_prefix: &str) -> ComResult<ComRc<Config>> {
        Ok(ComRc::from(&ComBox::new(Config::new(name, env_prefix))))
    }

    pub fn create_application(&self, config: ComRc<Config>, app_name: &str) -> ComResult<ComRc<Application>> {
        Ok(ComRc::from(&ComBox::new(Application::new(config, app_name))))
    }

    pub fn echo(&mut self, value: i32) -> i32 {
        value
    }
}


#[com_class(Application)]
pub struct Application {
    app: crate::application::Application<OpenGbApplication>,
}

#[com_interface]
impl Application {
    pub fn new(config: ComRc<Config>, app_name: &str) -> Self { 
        Self {
            app: OpenGbApplication::create(&config.as_ref().0, app_name)
        }
    }

    pub fn initialize(&mut self) {
        self.app.initialize();
    }
    
    pub fn run(&mut self) {
        self.app.run();
    }
}
