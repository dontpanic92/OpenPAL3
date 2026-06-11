use crosscom::ComRc;
use dashmap::DashMap;
use uuid::Uuid;

use super::Platform;
use crate::comdef::{IApplication, IApplicationImpl, IComponent, IComponentContainerImpl};
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

/// Closure registered via [`IApplicationExt::add_engine_ready_callback`]
/// and invoked exactly once, AFTER the engine has been created on
/// first `ApplicationHandler::resumed` but BEFORE component
/// `on_loading` fires. Used for app-side bootstrap that needs the
/// engine but must run before the loader component takes over (e.g.
/// imgui ini / theme setup). Closures typically capture a
/// `ComRc<IApplication>` clone to call back into the app.
pub type EngineReadyCallback = Box<dyn FnOnce()>;

pub struct Application {
    /// Engine slot, filled by the first-resumed `WindowReadyCallback`
    /// registered in [`Application::new`]. Before that hook fires
    /// [`Application::engine`] panics with a clear message. The inner
    /// `Rc<RefCell<...>>` is what gets handed back to callers — the
    /// outer `Option` is just the presence flag.
    radiance_engine: Rc<RefCell<Option<Rc<RefCell<CoreRadianceEngine>>>>>,
    platform: Rc<RefCell<Platform>>,
    components: Rc<DashMap<Uuid, AppComponentEntry>>,
    /// `true` once the post-engine-ready drain has fired (engine-ready
    /// callbacks + component on_loading). From that point onward
    /// `add_component` fires `on_loading` immediately. Matches today's
    /// semantics — only the moment of firing shifts.
    loaded: Cell<bool>,
    /// Set by [`Application::initialize`]. The drain is conditional
    /// on this flag — components that were registered without
    /// `initialize` ever being called are NOT loaded automatically.
    initialize_requested: Cell<bool>,
    /// Engine-ready callbacks awaiting the post-resumed drain. FIFO.
    /// After draining, subsequent
    /// [`IApplicationExt::add_engine_ready_callback`] calls fire the
    /// closure immediately.
    engine_ready_callbacks: Rc<RefCell<Vec<EngineReadyCallback>>>,
    /// Flipped to `true` inside the first-resumed `WindowReadyCallback`
    /// once the engine slot is filled. Read by
    /// `add_engine_ready_callback` to decide between deferring and
    /// firing immediately, and by the run-loop tick to know when to
    /// run the one-shot drain.
    engine_ready: Rc<Cell<bool>>,
}

ComObject_Application!(super::Application);

impl IComponentContainerImpl for Application {
    fn add_component(&self, uuid: uuid::Uuid, component: ComRc<IComponent>) -> () {
        // Fire on_loading immediately only after the initial drain.
        // Before the drain we just record the component — the drain
        // (or a later `initialize` if it runs post-engine-ready)
        // handles it.
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

        if self.initialize_requested.get() {
            return;
        }
        self.initialize_requested.set(true);

        // If the engine is already up (would only happen if
        // `initialize` is called from inside the run loop), drain
        // straight away. The common path is engine_ready=false at
        // this point, and the run-loop's per-tick drain hook picks
        // it up on the first frame after first-resumed.
        if self.engine_ready.get() {
            self.perform_drain();
        }
    }

    fn run(&self) {
        // Recover a `ComRc<IApplication>` for use inside the tick
        // closure. The CCW back-pointer in the macro-generated CCW
        // makes this O(1) and infallible. Keeping a `ComRc` alive
        // inside the tick is what lets us call back into
        // `Application` methods between frames.
        let app_rc = ComRc::<IApplication>::from_self(self);

        let mut start_time = Instant::now();
        let tick = move || {
            // Once-per-process: drain engine-ready callbacks +
            // pending on_loadings the first time engine_ready is
            // observed. Cheap no-op on subsequent ticks.
            let inner = app_rc.inner::<Application>();
            inner.perform_drain_if_ready();

            let end_time = Instant::now();
            let elapsed = end_time.duration_since(start_time).as_secs_f32();
            start_time = end_time;

            for kv in inner.components.iter() {
                kv.value().component.on_updating(elapsed);
            }

            if let Some(engine) = inner.radiance_engine.borrow().as_ref() {
                engine.borrow().update(elapsed);
            }
        };

        #[cfg(any(linux, macos, android))]
        {
            // Take the event loop + build the adapter in a tight scope so
            // the `Rc<RefCell<Platform>>` borrows are released BEFORE
            // `event_loop.run_app` blocks. This matters because the
            // first-resumed `WindowReadyCallback` re-borrows the platform
            // (`platform.borrow_mut()`) to call `create_radiance_engine`,
            // which registers additional input/imgui/lifecycle callbacks.
            // Holding an outer borrow here would trigger a
            // `RefCell already borrowed` panic.
            let (event_loop, mut adapter) = {
                let p = self.platform.borrow();
                let event_loop = p.take_event_loop();
                let adapter = p.build_app_handler(tick);
                (event_loop, adapter)
            };
            let _ = event_loop.run_app(&mut adapter);
        }

        #[cfg(windows)]
        {
            // The legacy Win32 Platform has no first-resumed concept —
            // the window already exists by the time `Platform::new`
            // returns. Bootstrap the engine inline so `engine_ready`
            // is true before the first tick, then drive the Win32
            // message pump.
            Self::bootstrap_engine(&self.platform, &self.radiance_engine, &self.engine_ready);
            self.platform.borrow().run_event_loop(tick);
        }

        // On platforms where the event loop returns cleanly (Vita,
        // Windows native), give the application a chance to fire
        // `on_unloading` on its loaded components before Drop tears
        // everything down. Platforms whose event loop never returns
        // (winit on desktop) rely on Drop instead.
        self.shutdown();
    }

    fn dpi_scale(&self) -> f32 {
        self.platform.borrow().dpi_scale()
    }

    fn request_exit(&self) {
        self.platform.borrow().request_exit();
    }
}

impl Application {
    /// Inherent counterpart to the formerly-IDL `set_title`. Access from
    /// a `ComRc<IApplication>` via the [`IApplicationExt`] trait.
    pub fn set_title(&self, title: &str) {
        self.platform.borrow().set_title(title);
    }

    /// Inherent counterpart to the formerly-IDL `engine`. Access from
    /// a `ComRc<IApplication>` via the [`IApplicationExt`] trait.
    ///
    /// Panics if called before the first `ApplicationHandler::resumed`
    /// — i.e. before the event loop has started and the engine has
    /// been bootstrapped from the just-created window. All known
    /// consumers run inside component `on_loading` (deferred to the
    /// post-resumed drain), inside service/director runtime methods
    /// (fired from the event loop), or inside `EngineReadyCallback`
    /// closures — all of which run AFTER the engine slot is filled.
    pub fn engine(&self) -> Rc<RefCell<CoreRadianceEngine>> {
        self.radiance_engine
            .borrow()
            .as_ref()
            .expect(
                "Application::engine() called before the engine is ready. \
                 The engine is created on the first `ApplicationHandler::resumed`. \
                 Eager engine access between `Application::new()` and the \
                 first frame must be moved into a component's `on_loading`, \
                 into an `IApplicationExt::add_engine_ready_callback` closure, \
                 or into a director factory invoked by the loader.",
            )
            .clone()
    }

    /// Append `cb` to the engine-ready queue (FIFO) or fire it
    /// immediately if the engine has already come up. Invoked from
    /// [`IApplicationExt::add_engine_ready_callback`].
    pub fn enqueue_engine_ready_callback(&self, cb: EngineReadyCallback) {
        if self.engine_ready.get() && self.loaded.get() {
            // Past the drain — fire immediately so callers don't
            // have to know whether they're pre- or post-resumed.
            cb();
        } else {
            self.engine_ready_callbacks.borrow_mut().push(cb);
        }
    }

    /// Per-tick guard: if the engine just became ready and we've
    /// asked to initialize, run the one-shot drain.
    fn perform_drain_if_ready(&self) {
        if self.loaded.get() || !self.engine_ready.get() || !self.initialize_requested.get() {
            return;
        }
        self.perform_drain();
    }

    /// Drain engine-ready callbacks (FIFO) followed by component
    /// `on_loading`s, exactly once. Idempotent — guarded by
    /// `self.loaded`. Engine-ready callbacks run BEFORE component
    /// on_loadings so app-side bootstrap (imgui ini/theme) lands
    /// before the loader installs scenes/directors.
    fn perform_drain(&self) {
        if self.loaded.get() {
            return;
        }
        self.drain_engine_ready_callbacks();
        self.drain_pending_on_loadings();
        self.loaded.set(true);
    }

    /// FIFO drain that's safe against re-entrant
    /// `add_engine_ready_callback` calls fired from inside a running
    /// callback. Steals the current Vec, runs each in order, and
    /// re-loops if any new callbacks were appended during the run.
    fn drain_engine_ready_callbacks(&self) {
        loop {
            let mut batch: Vec<EngineReadyCallback> =
                std::mem::take(&mut *self.engine_ready_callbacks.borrow_mut());
            if batch.is_empty() {
                break;
            }
            for cb in batch.drain(..) {
                cb();
            }
        }
    }

    /// Drain queued component `on_loading`s, exactly once. Idempotent
    /// at the per-entry level via the `loaded` flag in
    /// `AppComponentEntry`.
    fn drain_pending_on_loadings(&self) {
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
}

/// Extension trait that exposes `Application`'s engine-internal
/// accessors on a `ComRc<IApplication>` handle. These methods used to
/// live on the IDL via `[internal(), rust()]` shims; they are now
/// pure-Rust inherent methods on `Application`, surfaced here for
/// callers that only have a `ComRc<IApplication>`.
pub trait IApplicationExt {
    fn set_title(&self, title: &str);
    fn engine(&self) -> Rc<RefCell<CoreRadianceEngine>>;
    /// Register a closure that runs once, on the first run-loop tick
    /// after `ApplicationHandler::resumed` has bootstrapped the
    /// engine, BEFORE component `on_loading` fires. Used for
    /// app-side bootstrap that needs the engine but must run before
    /// the loader component takes over (e.g. imgui ini / theme
    /// setup, runtime-theme application). FIFO drain order. If the
    /// drain has already happened, the closure fires immediately.
    fn add_engine_ready_callback(&self, cb: EngineReadyCallback);
}

impl IApplicationExt for ComRc<crate::comdef::IApplication> {
    fn set_title(&self, title: &str) {
        self.inner::<Application>().set_title(title)
    }

    fn engine(&self) -> Rc<RefCell<CoreRadianceEngine>> {
        self.inner::<Application>().engine()
    }

    fn add_engine_ready_callback(&self, cb: EngineReadyCallback) {
        self.inner::<Application>()
            .enqueue_engine_ready_callback(cb);
    }
}

impl Application {
    pub fn new() -> Self {
        Self::set_panic_hook();
        let platform = Rc::new(RefCell::new(Platform::new()));
        let radiance_engine: Rc<RefCell<Option<Rc<RefCell<CoreRadianceEngine>>>>> =
            Rc::new(RefCell::new(None));
        let engine_ready = Rc::new(Cell::new(false));
        let engine_ready_callbacks: Rc<RefCell<Vec<EngineReadyCallback>>> =
            Rc::new(RefCell::new(vec![]));

        // First-resumed bootstrap (winit-only). The closure captures
        // the platform Rc and re-borrows it mutably here to call
        // `create_radiance_engine`, which reads
        // `platform.get_window()` (the live window the adapter just
        // created) and registers input/imgui/android-lifecycle
        // callbacks back onto the platform.
        //
        // `Application::run` is responsible for ensuring no OUTER
        // `platform.borrow()` is held when `event_loop.run_app`
        // blocks — otherwise the borrow_mut here would panic. See
        // `Application::run`'s scoped block.
        //
        // On Windows the legacy Win32 Platform has no first-resumed
        // hook; the window already exists by the time `Platform::new`
        // returns, so `Application::run` bootstraps the engine inline
        // via `bootstrap_engine` and skips this registration entirely.
        #[cfg(any(linux, macos, android))]
        {
            let platform_for_hook = platform.clone();
            let radiance_engine_for_hook = radiance_engine.clone();
            let engine_ready_for_hook = engine_ready.clone();
            platform
                .borrow_mut()
                .set_window_ready_callback(Box::new(move |_window| {
                    Self::bootstrap_engine(
                        &platform_for_hook,
                        &radiance_engine_for_hook,
                        &engine_ready_for_hook,
                    );
                    // engine_ready_callbacks + component on_loadings are
                    // drained by the run-loop tick — see
                    // `Application::perform_drain_if_ready`. The first
                    // tick happens before any redraw, so the drain is
                    // imperceptible to game/script code.
                }));
        }

        Self {
            radiance_engine,
            platform,
            components: Rc::new(DashMap::new()),
            loaded: Cell::new(false),
            initialize_requested: Cell::new(false),
            engine_ready_callbacks,
            engine_ready,
        }
    }

    /// Create the rendering engine for the live window and fill the
    /// `radiance_engine` slot, flipping `engine_ready` to `true`.
    /// Called from the winit `WindowReadyCallback` (after first
    /// resumed) on Linux/macOS/Android, and inline from
    /// `Application::run` on Windows (where the window exists from
    /// `Platform::new` onward).
    fn bootstrap_engine(
        platform: &Rc<RefCell<Platform>>,
        radiance_engine: &Rc<RefCell<Option<Rc<RefCell<CoreRadianceEngine>>>>>,
        engine_ready: &Rc<Cell<bool>>,
    ) {
        let engine = radiance::create_radiance_engine(&mut platform.borrow_mut())
            .expect(constants::STR_FAILED_CREATE_RENDERING_ENGINE);
        *radiance_engine.borrow_mut() = Some(Rc::new(RefCell::new(engine)));
        engine_ready.set(true);
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
