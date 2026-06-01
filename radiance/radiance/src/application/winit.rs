#[cfg(target_os = "android")]
use log::debug;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowAttributes};

pub type MessageCallback = Box<dyn Fn(&Event<()>)>;

pub struct Platform {
    event_loop: Cell<Option<EventLoop<()>>>,
    window: Rc<Window>,
    dpi_scale: f32,
    msg_callbacks: Rc<RefCell<Vec<MessageCallback>>>,
    quit_requested: Rc<Cell<bool>>,
}

impl Platform {
    pub fn new() -> Self {
        let event_loop = EventLoop::new().unwrap();
        let wa = WindowAttributes::default()
            .with_title("Radiance")
            .with_inner_size(LogicalSize::new(1280.0, 960.0))
            .with_resizable(true);
        let window = event_loop.create_window(wa).unwrap();

        Self {
            event_loop: Cell::new(Some(event_loop)),
            dpi_scale: window.scale_factor() as f32,
            msg_callbacks: Rc::new(RefCell::new(vec![])),
            window: Rc::new(window),
            quit_requested: Rc::new(Cell::new(false)),
        }
    }

    pub fn show_error_dialog(title: &str, msg: &str) {
        println!("title:{} msg:{}", title, msg);
    }

    pub fn initialize(&self) {}

    pub fn add_message_callback(&mut self, callback: MessageCallback) {
        self.msg_callbacks.borrow_mut().push(callback);
    }

    pub fn get_window(&self) -> &Rc<Window> {
        &self.window
    }

    pub fn run_event_loop<F1: 'static + FnMut()>(&self, mut update_engine: F1) {
        let window = self.window.clone();
        let msg_callbacks = self.msg_callbacks.clone();
        let event_loop = self.event_loop.take().unwrap();
        let quit_requested = self.quit_requested.clone();

        // `active` only flips on Android, where `Suspended` means the OS has
        // destroyed our rendering surface and we genuinely cannot draw. On
        // desktop (Linux/macOS) the game keeps simulating and rendering even
        // when another window has focus — losing focus does not pause us.
        #[cfg(target_os = "android")]
        let mut active = true;
        #[cfg(not(target_os = "android"))]
        let active = true;

        let _ = event_loop.run(move |event, window_target| {
            if quit_requested.get() {
                window_target.exit();
                return;
            }
            for cb in msg_callbacks.borrow().iter() {
                cb(&event);
            }
            match event {
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    window_target.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    if active {
                        update_engine();
                    }
                }
                #[cfg(target_os = "android")]
                Event::Suspended => {
                    debug!("Suspended");
                    active = false;
                }
                #[cfg(target_os = "android")]
                Event::Resumed => {
                    debug!("Resumed");
                    active = true;
                }
                _ => (),
            }
        });
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
