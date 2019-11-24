mod extensions;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
use windows::Platform;

use crate::constants;
use crate::rendering;
use extensions::ResultExtensions;

pub struct Application {
    rendering_engine: Box<dyn rendering::Engine>,
    platform: Platform,
}

impl Application {
    pub fn new() -> Self {
        Self { 
            rendering_engine: rendering::create_engine().unwrap_or_fail_fast(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            platform: Platform::new(),
        }
    }

    pub fn initialize(&self) {
        self.platform.initialize();
    }

    pub fn run(&self) {
        loop {
            if !self.platform.process_message()
            {
                break;
            }
        }
    }
}
