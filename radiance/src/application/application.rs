use super::extensions::ResultExtensions;
use super::Platform;

use crate::constants;
use crate::rendering;

pub struct Application {
    rendering_engine: Box<dyn rendering::Engine>,
    platform: Platform,
}

impl Application {
    pub fn new() -> Self {
        let platform = Platform::new();
        let window = rendering::Window { hwnd: platform.hwnd() };
        Self { 
            rendering_engine: rendering::create(&window).unwrap_or_fail_fast(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            platform: platform,
        }
    }

    pub fn initialize(&self) {
        self.platform.initialize();
    }

    pub fn run(&mut self) {
        loop {
            if !self.platform.process_message()
            {
                break;
            }

            self.rendering_engine.render();
        }
    }
}
