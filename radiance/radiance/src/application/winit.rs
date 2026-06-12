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

/// One-shot callback invoked the first time the OS gives us a
/// concrete window (i.e. on the first `ApplicationHandler::resumed`).
/// Used by `Application` to defer engine construction (Vulkan surface,
/// input/imgui wiring) until a real `Window` exists.
pub type WindowReadyCallback = Box<dyn FnOnce(&Rc<Window>)>;

pub struct Platform {
    event_loop: Cell<Option<EventLoop<()>>>,
    /// Attributes used to materialise the `Window` on first resumed.
    /// Taken (Option::take) by the `ApplicationHandler` on first
    /// resumed so subsequent resumes don't re-create the window.
    window_attributes: Cell<Option<WindowAttributes>>,
    /// Window slot, filled on first resumed. Shared with the
    /// `PlatformAppHandler` adapter so the adapter can write into it
    /// while `Platform` (and external callers via `get_window`) read
    /// from it.
    window: Rc<RefCell<Option<Rc<Window>>>>,
    /// HiDPI scale. Defaults to 1.0; overwritten on first resumed
    /// from `window.scale_factor()`. Shared so the adapter can update
    /// it.
    dpi_scale: Rc<Cell<f32>>,
    /// One-shot first-resumed callback used to bootstrap the engine.
    window_ready: Rc<RefCell<Option<WindowReadyCallback>>>,
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

        Self {
            event_loop: Cell::new(Some(event_loop)),
            window_attributes: Cell::new(Some(wa)),
            window: Rc::new(RefCell::new(None)),
            dpi_scale: Rc::new(Cell::new(1.0)),
            window_ready: Rc::new(RefCell::new(None)),
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

    /// Register the one-shot callback invoked on first resumed once
    /// the window exists. Replaces any prior registration.
    pub fn set_window_ready_callback(&mut self, callback: WindowReadyCallback) {
        *self.window_ready.borrow_mut() = Some(callback);
    }

    /// Returns the live window. **Panics** if called before the
    /// `WindowReadyCallback` has fired — i.e. before
    /// `ApplicationHandler::resumed` runs for the first time. The
    /// engine bootstrap path inside the `window_ready` callback is
    /// the only intended caller.
    pub fn get_window(&self) -> Rc<Window> {
        self.window
            .borrow()
            .as_ref()
            .expect(
                "Platform::get_window() called before the window exists; \
                 only callable after the WindowReadyCallback fires (i.e. \
                 from inside or after first ApplicationHandler::resumed)",
            )
            .clone()
    }

    pub fn run_event_loop<F1: 'static + FnMut()>(&self, update_engine: F1) {
        let event_loop = self.take_event_loop();
        let mut adapter = self.build_app_handler(update_engine);
        let _ = event_loop.run_app(&mut adapter);
    }

    /// Take the EventLoop out for ownership. Must be paired with a
    /// subsequent `build_app_handler` call (or with another
    /// `EventLoop::run_app(&mut handler)` from a caller-provided
    /// handler) and `EventLoop::run_app(&mut adapter)`. Splitting
    /// this from `run_event_loop` lets callers release any
    /// `Rc<RefCell<Platform>>` borrow they hold before
    /// `event_loop.run_app` blocks — important because the
    /// first-resumed `WindowReadyCallback` typically re-borrows the
    /// platform to register additional callbacks (input, imgui,
    /// android surface).
    pub fn take_event_loop(&self) -> EventLoop<()> {
        self.event_loop
            .take()
            .expect("Platform::take_event_loop called twice or after run")
    }

    /// Build the `ApplicationHandler` adapter that bridges winit
    /// events back into Platform's per-kind callback registries and
    /// the per-frame `update_engine` closure. The returned handler
    /// shares the Platform's slots via `Rc<RefCell<…>>`, so the
    /// caller may freely `borrow_mut` the Platform from inside any
    /// callback fired by the handler (typical for the first-resumed
    /// engine bootstrap).
    pub fn build_app_handler<F1: 'static + FnMut()>(
        &self,
        update_engine: F1,
    ) -> PlatformAppHandler {
        PlatformAppHandler {
            window: self.window.clone(),
            window_attributes: self.window_attributes.take(),
            dpi_scale: self.dpi_scale.clone(),
            window_ready: self.window_ready.clone(),
            window_callbacks: self.window_callbacks.clone(),
            device_callbacks: self.device_callbacks.clone(),
            about_to_wait_callbacks: self.about_to_wait_callbacks.clone(),
            lifecycle_callbacks: self.lifecycle_callbacks.clone(),
            quit_requested: self.quit_requested.clone(),
            update_engine: Box::new(update_engine),
            // `active` only flips on Android, where `Suspended` means the OS
            // has destroyed our rendering surface and we genuinely cannot
            // draw. On desktop (Linux/macOS) the game keeps simulating and
            // rendering even when another window has focus. We start
            // `active = false` and let the first `resumed` flip it on every
            // platform; winit guarantees `resumed` fires before any window
            // event on supported targets.
            active: false,
        }
    }

    /// Current HiDPI scale. Returns `1.0` until the first resumed has
    /// fired and the real `Window::scale_factor()` has been cached.
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale.get()
    }

    /// Logical pixel extent of the window's inner client area. Returns
    /// `None` before the first `resumed` (no window yet). On winit,
    /// physical inner_size is divided by `scale_factor` to drop HiDPI
    /// scaling — e.g. a 1280×960 window on a 2× Retina returns
    /// `(1280, 960)`, while the underlying surface extent is 2560×1920.
    pub fn logical_inner_extent(&self) -> Option<(u32, u32)> {
        let window = self.window.borrow();
        let window = window.as_ref()?;
        let scale = self.dpi_scale.get().max(0.0001) as f64;
        let physical = window.inner_size();
        let w = ((physical.width as f64) / scale).round() as u32;
        let h = ((physical.height as f64) / scale).round() as u32;
        Some((w.max(1), h.max(1)))
    }

    /// Set the window title. No-ops gracefully when called before the
    /// window exists (no current caller does, but the panic-free
    /// behaviour avoids surprises in tests / future callers).
    pub fn set_title(&self, title: &str) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_title(title);
        }
    }

    pub fn request_exit(&self) {
        self.quit_requested.set(true);
    }
}

/// Bridges winit 0.30's `ApplicationHandler` trait back to the engine's
/// per-kind callback registries + per-frame `update_engine` closure.
///
/// Built by [`Platform::build_app_handler`]; the only intended call
/// site is `event_loop.run_app(&mut adapter)` after the
/// caller has released any outer `Platform` borrow.
pub struct PlatformAppHandler {
    window: Rc<RefCell<Option<Rc<Window>>>>,
    window_attributes: Option<WindowAttributes>,
    dpi_scale: Rc<Cell<f32>>,
    window_ready: Rc<RefCell<Option<WindowReadyCallback>>>,
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

        // On the FIRST resumed: take the stored window attributes,
        // create the real window via the new `ActiveEventLoop::create_window`,
        // cache the dpi_scale, fill the window slot, and fire the
        // one-shot `window_ready` callback (which is where the engine
        // bootstrap lives). On subsequent resumes (Android), the
        // attributes are already taken so we skip straight to the
        // lifecycle-callback dispatch — Android's surface-recreate
        // logic lives in the lifecycle callback registered by
        // `create_radiance_engine`.
        if let Some(attrs) = self.window_attributes.take() {
            let window = event_loop
                .create_window(attrs)
                .expect("ActiveEventLoop::create_window failed on first resumed");
            self.dpi_scale.set(window.scale_factor() as f32);
            let window_rc = Rc::new(window);
            *self.window.borrow_mut() = Some(window_rc.clone());
            if let Some(cb) = self.window_ready.borrow_mut().take() {
                cb(&window_rc);
            }
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
        if let Some(window) = self.window.borrow().as_ref() {
            window.request_redraw();
        }
    }
}
