use log::debug;
use std::cell::{RefCell, Cell};
use std::rc::Rc;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

pub type MessageCallback = Box<dyn Fn(&Event<()>)>;

pub struct Platform {
    event_loop: Cell<Option<EventLoop<()>>>,
    window: Rc<Window>,
    dpi_scale: f32,
    msg_callbacks: Rc<RefCell<Vec<MessageCallback>>>,
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
            event_loop: Cell::new(Some(event_loop)),
            dpi_scale: window.scale_factor() as f32,
            msg_callbacks: Rc::new(RefCell::new(vec![])),
            window: Rc::new(window),
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
        let mut active = true;
        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::RedrawRequested(_) => {}
                Event::RedrawEventsCleared => {}
                Event::MainEventsCleared => {
                    // needed for imgui got notified to prepare frame
                    for cb in msg_callbacks.borrow().iter() {
                        cb(&event);
                    }

                    if active {
                        update_engine();
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                event => {
                    // debug!("Event: {:?}", event);
                    for cb in msg_callbacks.borrow().iter() {
                        cb(&event);
                    }
                    // other application-specific event handling
                    match event {
                        Event::Suspended => {
                            debug!("Suspended");
                            active = false;
                        }
                        Event::Resumed => {
                            debug!("Resumed");
                            active = true;
                        }
                        Event::WindowEvent {
                            event: WindowEvent::Focused(focused),
                            ..
                        } => {
                            active = focused;
                        }
                        _ => (),
                    }
                }
            }
        });
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }
}
