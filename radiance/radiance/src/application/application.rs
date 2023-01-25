use crosscom::ComRc;
use dashmap::DashMap;
use uuid::Uuid;

use super::Platform;
use crate::comdef::{IApplicationImpl, IComponent, IComponentContainerImpl};
use crate::constants;
use crate::radiance;
use crate::radiance::CoreRadianceEngine;
use crate::ComObject_Application;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

pub struct Application {
    radiance_engine: Rc<RefCell<CoreRadianceEngine>>,
    platform: Rc<RefCell<Platform>>,
    components: DashMap<Uuid, ComRc<IComponent>>,
}

ComObject_Application!(super::Application);

impl IComponentContainerImpl for Application {
    fn add_component(&self, uuid: uuid::Uuid, component: ComRc<IComponent>) -> () {
        self.components.insert(uuid, component);
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components
            .get(&uuid)
            .and_then(|c| Some(c.value().clone()))
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components.remove(&uuid).and_then(|c| Some(c.1))
    }
}

impl IApplicationImpl for Application {
    fn initialize(&self) {
        self.platform.borrow_mut().initialize();

        for c in self.components.clone() {
            c.1.on_loading()
        }
    }

    fn run(&self) {
        let engine = self.radiance_engine.clone();
        let platform = self.platform.clone();

        let mut start_time = Instant::now();
        platform.borrow_mut().run_event_loop(move || {
            let end_time = Instant::now();
            let elapsed = end_time.duration_since(start_time).as_secs_f32();
            start_time = end_time;

            /*if elapsed < 1./120. {
                continue;
            }*/

            for c in self.components.clone() {
                c.1.on_updating(elapsed);
            }

            engine.borrow_mut().update(elapsed);
        });
    }

    fn set_title(&self, title: &str) {
        self.platform.borrow_mut().set_title(title);
    }

    fn engine(&self) -> Rc<RefCell<CoreRadianceEngine>> {
        self.radiance_engine.clone()
    }
}

impl Application {
    pub fn new() -> Self {
        set_panic_hook();
        let mut platform = Platform::new();
        Self {
            radiance_engine: Rc::new(RefCell::new(
                radiance::create_radiance_engine(&mut platform)
                    .expect(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            )),
            platform: Rc::new(RefCell::new(platform)),
            components: DashMap::new(),
        }
    }
}

fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let msg = format!("Radiance {}\n{:?}", panic_info, backtrace);
        Platform::show_error_dialog(crate::constants::STR_SORRY_DIALOG_TITLE, &msg);
    }));
}
