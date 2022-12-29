use super::Platform;
use crate::constants;
use crate::radiance;
use crate::radiance::CoreRadianceEngine;
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::time::Instant;

pub trait ApplicationExtension<TImpl: ApplicationExtension<TImpl>> {
    define_ext_fn!(on_initialized, Application, TImpl);
    define_ext_fn!(on_updating, Application, TImpl, _delta_sec: f32);
}

mod private {
    pub struct EmptyCallbacks {}
    impl super::ApplicationExtension<EmptyCallbacks> for EmptyCallbacks {}
}
pub type DefaultApplication = Application<private::EmptyCallbacks>;

pub struct Application<TExtension: 'static + ApplicationExtension<TExtension>> {
    radiance_engine: Rc<RefCell<CoreRadianceEngine>>,
    platform: Rc<RefCell<Platform>>,
    extension: Rc<RefCell<TExtension>>,
}

impl<TExtension: 'static + ApplicationExtension<TExtension>> Application<TExtension> {
    pub fn new(extension: TExtension) -> Self {
        set_panic_hook();
        let mut platform = Platform::new();
        Self {
            radiance_engine: Rc::new(RefCell::new(
                radiance::create_radiance_engine(&mut platform)
                    .expect(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            )),
            platform: Rc::new(RefCell::new(platform)),
            extension: Rc::new(RefCell::new(extension)),
        }
    }

    pub fn engine_mut(&mut self) -> RefMut<CoreRadianceEngine> {
        self.radiance_engine.borrow_mut()
    }

    pub fn callbacks_mut(&self) -> RefMut<TExtension> {
        self.extension.borrow_mut()
    }

    pub fn initialize(&mut self) {
        self.platform.borrow_mut().initialize();
        ext_call!(self, on_initialized);
    }

    pub fn set_title(&mut self, title: &str) {
        self.platform.borrow_mut().set_title(title);
    }

    pub fn run(mut self) {
        let engine = self.radiance_engine.clone();
        let extension = self.extension.clone();
        let platform = self.platform.clone();

        let mut start_time = Instant::now();
        platform.borrow_mut().run_event_loop(move || {
            let end_time = Instant::now();
            let elapsed = end_time.duration_since(start_time).as_secs_f32();
            start_time = end_time;

            /*if elapsed < 1./120. {
                continue;
            }*/

            let mut ext = extension.borrow_mut();
            ext.on_updating(&mut self, elapsed);
            // ext_call!(self, on_updating, elapsed);
            engine.borrow_mut().update(elapsed);
        });
    }
}

fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let msg = format!("Radiance {}\n{:?}", panic_info, backtrace);
        Platform::show_error_dialog(crate::constants::STR_SORRY_DIALOG_TITLE, &msg);
    }));
}
