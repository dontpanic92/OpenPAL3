use super::application::OpenGbApplication;
use super::config::OpenGbConfig;
use intercom::*;
use std::ops::Deref;

com_library! {
    class Factory,
    interface Config,
    interface IApplication,
    interface IApplicationExtension,
}

#[com_class(Config)]
pub struct Config(OpenGbConfig);

#[com_interface]
impl Config {
    pub fn new(config_name: &str, env_prefix: &str) -> Self {
        Self {
            0: OpenGbConfig::load(config_name, env_prefix),
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

    pub fn create_default_application(
        &self,
        config: ComRc<Config>,
        app_name: &str,
    ) -> ComResult<ComRc<dyn IApplication>> {
        Ok(ComRc::from(&ComBox::new(ComApplication::new(
            config, app_name,
        ))))
    }

    pub fn create_application(
        &self,
        ext: ComRc<dyn IApplicationExtension>,
    ) -> ComResult<ComRc<dyn IApplication>> {
        Ok(ComCustomApplication::create(ext))
    }

    pub fn echo(&mut self, value: i32) -> i32 {
        value
    }
}

#[com_interface]
pub trait IApplication {
    fn initialize(&mut self);
    fn run(&mut self);
}

#[com_class(IApplication)]
pub struct ComApplication {
    app: crate::application::Application<OpenGbApplication>,
}

impl ComApplication {
    pub fn new(config: ComRc<Config>, app_name: &str) -> Self {
        Self {
            app: OpenGbApplication::create(&config.as_ref().0, app_name),
        }
    }
}

impl IApplication for ComApplication {
    fn initialize(&mut self) {
        self.app.initialize();
    }

    fn run(&mut self) {
        self.app.run();
    }
}

#[com_interface]
pub trait IApplicationExtension {
    fn on_initialized(&self, app: Option<&ComItf<dyn IApplication>>);
    fn on_updating(&self, app: Option<&ComItf<dyn IApplication>>, delta_sec: f32);
}

pub struct ComApplicationExtension {
    pub ext: ComRc<dyn IApplicationExtension>,
    pub app: Option<raw::InterfacePtr<type_system::AutomationTypeSystem, ComCustomApplication>>,
}

impl crate::application::ApplicationExtension<ComApplicationExtension> for ComApplicationExtension {
    fn on_initialized(
        &mut self,
        app: &mut crate::application::Application<ComApplicationExtension>,
    ) {
        let itf = ComItf::maybe_new(self.app, None).unwrap();
        let o = ComItf::query_interface::<dyn IApplication>(&itf).unwrap();
        self.ext.on_initialized(Some(&o));
    }

    fn on_updating(
        &mut self,
        app: &mut crate::application::Application<ComApplicationExtension>,
        delta_sec: f32,
    ) {
    }
}

#[com_class(ComCustomApplication, IApplication)]
pub struct ComCustomApplication {
    app: crate::application::Application<ComApplicationExtension>,
}

#[com_interface]
impl ComCustomApplication {
    fn new(ext: ComRc<dyn IApplicationExtension>) -> Self {
        Self {
            app: crate::application::Application::new(ComApplicationExtension { ext, app: None }),
        }
    }

    pub fn create(ext: ComRc<dyn IApplicationExtension>) -> ComRc<dyn IApplication> {
        let app: ComRc<ComCustomApplication> = ComRc::from(&ComBox::new(Self::new(ext)));
        app.as_ref().deref().app.callbacks_mut().app = ComItf::ptr(app.as_ref());
        return ComItf::query_interface::<dyn IApplication>(&app).unwrap();
    }
}

impl IApplication for ComCustomApplication {
    fn initialize(&mut self) {
        self.app.initialize();
    }

    fn run(&mut self) {
        self.app.run();
    }
}
