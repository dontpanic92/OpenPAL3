use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

pub type MessageCallback = Box<dyn Fn(&Window, &Event<()>)>;

pub struct Platform {
    event_loop: EventLoop<()>,
    window: Window,
    dpi_scale: f32,
    msg_callbacks: Vec<MessageCallback>,
}

impl Platform {
    pub fn new() -> Self {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Radiance")
            .with_inner_size(LogicalSize::new(1280.0, 960.0))
            .with_resizable(true)
            .build(&event_loop)
            .unwrap();
        Self {
            event_loop,
            dpi_scale: window.scale_factor() as f32,
            msg_callbacks: vec![],
            window,
        }
    }

    pub fn show_error_dialog(title: &str, msg: &str) {
        println!("title:{} msg:{}", title, msg);
    }

    pub fn initialize(&self) {}

    pub fn add_message_callback(&mut self, callback: MessageCallback) {
        self.msg_callbacks.push(callback);
    }

    pub fn get_window(&self) -> &Window {
        &self.window
    }

    pub fn run_event_loop<F1: 'static + FnMut(&Window, f32)>(self, mut update_engine: F1) {
        let Platform {
            window,
            msg_callbacks,
            ..
        } = self;
        let mut start_time = Instant::now();
        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::NewEvents(_) => {
                    // other application-specific logic
                }
                Event::MainEventsCleared => {
                    for cb in &msg_callbacks {
                        cb(&window, &event);
                    }

                    let end_time = Instant::now();
                    let elapsed = end_time.duration_since(start_time).as_secs_f32();
                    start_time = end_time;
                    update_engine(&window, elapsed);
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                event => {
                    // other application-specific event handling
                    for cb in &msg_callbacks {
                        cb(&window, &event);
                    }
                }
            }
        });
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    pub fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }
}
