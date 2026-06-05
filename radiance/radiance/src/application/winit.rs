#[cfg(target_os = "android")]
use log::debug;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

/// Lifecycle transitions surfaced by winit's `resumed`/`suspended`
/// hooks. On desktop both fire (resumed once at startup); on Android
/// they bracket every surface-destroy cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    Resumed,
    Suspended,
}

pub type WindowEventCallback = Box<dyn Fn(WindowId, &WindowEvent)>;
pub type DeviceEventCallback = Box<dyn Fn(DeviceId, &DeviceEvent)>;
pub type AboutToWaitCallback = Box<dyn Fn()>;
pub type LifecycleCallback = Box<dyn Fn(LifecycleEvent)>;

pub struct Platform {
    event_loop: Cell<Option<EventLoop<()>>>,
    window: Rc<Window>,
    dpi_scale: f32,
    window_callbacks: Rc<RefCell<Vec<WindowEventCallback>>>,
    device_callbacks: Rc<RefCell<Vec<DeviceEventCallback>>>,
    about_to_wait_callbacks: Rc<RefCell<Vec<AboutToWaitCallback>>>,
    lifecycle_callbacks: Rc<RefCell<Vec<LifecycleCallback>>>,
    quit_requested: Rc<Cell<bool>>,
}

impl Platform {
    pub fn new() -> Self {
        let event_loop = EventLoop::new().unwrap();
        let wa = WindowAttributes::default()
            .with_title("Radiance")
            .with_inner_size(LogicalSize::new(1280.0, 960.0))
            .with_resizable(true);
        // TODO(winit-migration): this still uses the deprecated
        // `EventLoop::create_window`. Fully migrating to
        // `ActiveEventLoop::create_window` (called from
        // `ApplicationHandler::resumed`) would require deferring engine
        // construction past `Application::new()`, which in turn requires
        // refactoring ~15 `application.engine()` callsites across
        // `radiance_editor`, `radiance_scripting`, `yaobow`,
        // `yaobow_editor`, and `shared/openpalX..openswd5` to access the
        // engine lazily (today they read it immediately after `new()` or
        // during component `on_loading` which fires from `initialize()`
        // before the event loop starts). Tracked as a follow-up; the
        // existing eager path still works correctly on desktop.
        let window = event_loop.create_window(wa).unwrap();

        Self {
            event_loop: Cell::new(Some(event_loop)),
            dpi_scale: window.scale_factor() as f32,
            window: Rc::new(window),
            window_callbacks: Rc::new(RefCell::new(vec![])),
            device_callbacks: Rc::new(RefCell::new(vec![])),
            about_to_wait_callbacks: Rc::new(RefCell::new(vec![])),
            lifecycle_callbacks: Rc::new(RefCell::new(vec![])),
            quit_requested: Rc::new(Cell::new(false)),
        }
    }

    pub fn show_error_dialog(title: &str, msg: &str) {
        println!("title:{} msg:{}", title, msg);
    }

    pub fn initialize(&self) {}

    pub fn add_window_event_callback(&mut self, callback: WindowEventCallback) {
        self.window_callbacks.borrow_mut().push(callback);
    }

    pub fn add_device_event_callback(&mut self, callback: DeviceEventCallback) {
        self.device_callbacks.borrow_mut().push(callback);
    }

    pub fn add_about_to_wait_callback(&mut self, callback: AboutToWaitCallback) {
        self.about_to_wait_callbacks.borrow_mut().push(callback);
    }

    pub fn add_lifecycle_callback(&mut self, callback: LifecycleCallback) {
        self.lifecycle_callbacks.borrow_mut().push(callback);
    }

    pub fn get_window(&self) -> &Rc<Window> {
        &self.window
    }

    pub fn run_event_loop<F1: 'static + FnMut()>(&self, update_engine: F1) {
        let event_loop = self.event_loop.take().unwrap();

        // `active` only flips on Android, where `Suspended` means the OS has
        // destroyed our rendering surface and we genuinely cannot draw. On
        // desktop (Linux/macOS) the game keeps simulating and rendering even
        // when another window has focus — losing focus does not pause us.
        // We start `active = false` and let the first `resumed` flip it on
        // every platform; winit guarantees `resumed` fires before any
        // window event on supported targets.
        let mut adapter = PlatformAppHandler {
            window: self.window.clone(),
            window_callbacks: self.window_callbacks.clone(),
            device_callbacks: self.device_callbacks.clone(),
            about_to_wait_callbacks: self.about_to_wait_callbacks.clone(),
            lifecycle_callbacks: self.lifecycle_callbacks.clone(),
            quit_requested: self.quit_requested.clone(),
            update_engine: Box::new(update_engine),
            active: false,
        };
        let _ = event_loop.run_app(&mut adapter);
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    pub fn request_exit(&self) {
        self.quit_requested.set(true);
    }
}

/// Bridges winit 0.30's `ApplicationHandler` trait back to the engine's
/// per-kind callback registries + per-frame `update_engine` closure.
struct PlatformAppHandler {
    window: Rc<Window>,
    window_callbacks: Rc<RefCell<Vec<WindowEventCallback>>>,
    device_callbacks: Rc<RefCell<Vec<DeviceEventCallback>>>,
    about_to_wait_callbacks: Rc<RefCell<Vec<AboutToWaitCallback>>>,
    lifecycle_callbacks: Rc<RefCell<Vec<LifecycleCallback>>>,
    quit_requested: Rc<Cell<bool>>,
    update_engine: Box<dyn FnMut()>,
    active: bool,
}

impl ApplicationHandler for PlatformAppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "android")]
        debug!("Resumed");
        if self.quit_requested.get() {
            event_loop.exit();
            return;
        }
        for cb in self.lifecycle_callbacks.borrow().iter() {
            cb(LifecycleEvent::Resumed);
        }
        self.active = true;
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "android")]
        debug!("Suspended");
        if self.quit_requested.get() {
            event_loop.exit();
            return;
        }
        for cb in self.lifecycle_callbacks.borrow().iter() {
            cb(LifecycleEvent::Suspended);
        }
        self.active = false;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.quit_requested.get() {
            event_loop.exit();
            return;
        }
        for cb in self.window_callbacks.borrow().iter() {
            cb(window_id, &event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if self.active {
                    (self.update_engine)();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.quit_requested.get() {
            event_loop.exit();
            return;
        }
        for cb in self.device_callbacks.borrow().iter() {
            cb(device_id, &event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit_requested.get() {
            event_loop.exit();
            return;
        }
        for cb in self.about_to_wait_callbacks.borrow().iter() {
            cb();
        }
        self.window.request_redraw();
    }
}
