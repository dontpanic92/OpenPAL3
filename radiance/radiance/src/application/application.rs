use crosscom::ComRc;
use dashmap::DashMap;
use uuid::Uuid;

use super::Platform;
use crate::comdef::{IApplicationImpl, IComponent, IComponentContainerImpl};
use crate::constants;
use crate::radiance;
use crate::radiance::CoreRadianceEngine;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

struct AppComponentEntry {
    component: ComRc<IComponent>,
    loaded: bool,
}

pub struct Application {
    radiance_engine: Rc<RefCell<CoreRadianceEngine>>,
    platform: Rc<RefCell<Platform>>,
    components: Rc<DashMap<Uuid, AppComponentEntry>>,
    loaded: Cell<bool>,
}

ComObject_Application!(super::Application);

impl IComponentContainerImpl for Application {
    fn add_component(&self, uuid: uuid::Uuid, component: ComRc<IComponent>) -> () {
        // Mirror Entity/Scene: fire on_loading immediately if the
        // application is already initialised; otherwise defer to
        // `initialize`.
        let fire_now = self.loaded.get();
        if fire_now {
            component.on_loading();
        }
        self.components.insert(
            uuid,
            AppComponentEntry {
                component,
                loaded: fire_now,
            },
        );
    }

    fn get_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        self.components.get(&uuid).map(|e| e.component.clone())
    }

    fn remove_component(&self, uuid: uuid::Uuid) -> Option<ComRc<IComponent>> {
        let entry = self.components.remove(&uuid).map(|(_, e)| e);
        if let Some(e) = entry {
            if e.loaded {
                e.component.on_unloading();
            }
            Some(e.component)
        } else {
            None
        }
    }
}

impl IApplicationImpl for Application {
    fn initialize(&self) {
        self.platform.borrow_mut().initialize();

        if self.loaded.get() {
            return;
        }
        self.loaded.set(true);

        let uuids: Vec<Uuid> = self.components.iter().map(|kv| *kv.key()).collect();
        for uuid in uuids {
            let component = {
                let mut entry = match self.components.get_mut(&uuid) {
                    Some(e) => e,
                    None => continue,
                };
                if entry.loaded {
                    continue;
                }
                entry.loaded = true;
                entry.component.clone()
            };
            component.on_loading();
        }
    }

    fn run(&self) {
        let engine = self.radiance_engine.clone();
        let platform = self.platform.clone();
        let components = self.components.clone();

        let mut start_time = Instant::now();
        platform.borrow().run_event_loop(move || {
            let end_time = Instant::now();
            let elapsed = end_time.duration_since(start_time).as_secs_f32();
            start_time = end_time;

            /*if elapsed < 1./120. {
                continue;
            }*/

            for kv in components.iter() {
                kv.value().component.on_updating(elapsed);
            }

            engine.borrow().update(elapsed);
        });

        // On platforms where `run_event_loop` returns cleanly (Vita,
        // Windows native), give the application a chance to fire
        // `on_unloading` on its loaded components before Drop tears
        // everything down. Platforms whose event loop never returns
        // (winit on desktop) rely on Drop instead.
        self.shutdown();
    }

    fn set_title(&self, title: &str) {
        self.platform.borrow().set_title(title);
    }

    fn engine(&self) -> Rc<RefCell<CoreRadianceEngine>> {
        self.radiance_engine.clone()
    }

    fn dpi_scale(&self) -> f32 {
        self.platform.borrow().dpi_scale()
    }
}

impl Application {
    pub fn new() -> Self {
        Self::set_panic_hook();
        let mut platform = Platform::new();
        Self {
            radiance_engine: Rc::new(RefCell::new(
                radiance::create_radiance_engine(&mut platform)
                    .expect(constants::STR_FAILED_CREATE_RENDERING_ENGINE),
            )),
            platform: Rc::new(RefCell::new(platform)),
            components: Rc::new(DashMap::new()),
            loaded: Cell::new(false),
        }
    }

    /// Fire `on_unloading` on every component that received
    /// `on_loading`, exactly once. Idempotent and safe to call from
    /// both the run-loop exit path and `Drop`.
    pub fn shutdown(&self) {
        if !self.loaded.get() {
            return;
        }
        self.loaded.set(false);

        let uuids: Vec<Uuid> = self.components.iter().map(|kv| *kv.key()).collect();
        let mut to_unload: Vec<ComRc<IComponent>> = Vec::new();
        for uuid in uuids {
            if let Some((_, entry)) = self.components.remove(&uuid) {
                if entry.loaded {
                    to_unload.push(entry.component);
                }
            }
        }
        for c in to_unload {
            c.on_unloading();
        }
    }

    pub fn set_panic_hook() {
        std::panic::set_hook(Box::new(|panic_info| {
            let backtrace = backtrace::Backtrace::new();
            let msg = format!("Radiance {}\n{:?}", panic_info, backtrace);
            log::error!("{}", &msg);
            Platform::show_error_dialog(crate::constants::STR_SORRY_DIALOG_TITLE, &msg);
        }));
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.shutdown();
    }
}
