use super::extensions::ResultExtensions;
use super::Platform;
use crate::constants;
use crate::radiance;
use crate::radiance::CoreRadianceEngine;
use crate::radiance::RadianceEngine;
use crate::rendering;
use crate::rendering::VulkanRenderingEngine;

pub struct Application {
    radiance_engine: CoreRadianceEngine<VulkanRenderingEngine>,
    platform: Platform,
}

impl Application {
    pub fn new() -> Self {
        let platform = Platform::new();
        let window = rendering::Window {
            hwnd: platform.hwnd(),
        };
        Self {
            radiance_engine: radiance::create_radiance_engine::<VulkanRenderingEngine>(&window)
                .unwrap_or_fail_fast(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            platform: platform,
        }
    }

    pub fn initialize(&self) {
        self.platform.initialize();
    }

    pub fn run(&mut self) {
        self.radiance_engine.load_scene();

        loop {
            if !self.platform.process_message() {
                break;
            }

            self.radiance_engine.update();
        }

        self.radiance_engine.unload_scene();
    }
}
