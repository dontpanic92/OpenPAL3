use super::extensions::ResultExtensions;
use super::Platform;
use crate::constants;
use crate::radiance;
use crate::radiance::CoreRadianceEngine;
use crate::rendering;
use crate::rendering::VulkanRenderingEngine;
use std::rc::Rc;
use std::time::Instant;
use std::cell::{RefMut, RefCell};

pub trait ApplicationCallbacks {
    define_callback_fn!(on_initialized, Application, ApplicationCallbacks);
    define_callback_fn!(on_updated, Application, ApplicationCallbacks, _delta_sec: f32);
}

mod private {
    pub struct EmptyCallbacks {}
    impl super::ApplicationCallbacks for EmptyCallbacks {}
} 
pub type DefaultApplication = Application<private::EmptyCallbacks>;

pub struct Application<TCallbacks: ApplicationCallbacks> {
    radiance_engine: CoreRadianceEngine<VulkanRenderingEngine>,
    platform: Platform,
    callbacks: Rc<RefCell<TCallbacks>>,
}

impl<TCallbacks: ApplicationCallbacks> Application<TCallbacks> {
    pub fn new(callbacks: RefCell<TCallbacks>) -> Self {
        let platform = Platform::new();
        let window = rendering::Window {
            hwnd: platform.hwnd(),
        };
        Self {
            radiance_engine: radiance::create_radiance_engine::<VulkanRenderingEngine>(&window)
                .unwrap_or_fail_fast(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            platform,
            callbacks: Rc::new(callbacks),
        }
    }

    pub fn engine_mut(&mut self) -> &mut CoreRadianceEngine<VulkanRenderingEngine> {
        &mut self.radiance_engine
    }

    pub fn callbacks_mut(&self) -> RefMut<TCallbacks> {
        self.callbacks.borrow_mut()
    }

    pub fn initialize(&mut self) {
        self.platform.initialize();
        callback!(self, on_initialized);
    }

    pub fn set_title(&mut self, title: &str) {
        self.platform.set_title(title);
    }

    pub fn run(&mut self) {
        let mut frame_start_time = Instant::now();
        loop {
            if !self.platform.process_message() {
                break;
            }

            self.radiance_engine.update();

            let frame_end_time = Instant::now();
            let elapsed = frame_end_time.duration_since(frame_start_time).as_secs_f32();
            frame_start_time = frame_end_time;
            callback!(self, on_updated, elapsed);
        }

        self.radiance_engine.unload_scene();
    }
}
