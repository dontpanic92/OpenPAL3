use std::{cell::RefCell, rc::Rc};

use imgui::{Context, Io};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use winit::dpi::PhysicalPosition;
use winit::event::{
    DeviceId, ElementState, Event, ModifiersState, MouseButton, Touch, TouchPhase, WindowEvent,
};
use winit::window::Window;

use crate::application::Platform;

pub struct ImguiPlatform {
    context: Rc<RefCell<Context>>,
    winit_platform: WinitPlatform,
    window: Rc<Window>,
}

impl ImguiPlatform {
    pub fn new(context: Rc<RefCell<Context>>, platform: &mut Platform) -> Rc<RefCell<Self>> {
        let mut winit_platform = WinitPlatform::init(&mut context.as_ref().borrow_mut());
        let window = platform.get_window().clone();
        winit_platform.attach_window(
            context.as_ref().borrow_mut().io_mut(),
            &window,
            HiDpiMode::Locked(1.0),
        );

        let imgui_platform = Rc::new(RefCell::new(Self {
            context: context.clone(),
            winit_platform,
            window,
        }));

        let imgui_platform_clone = imgui_platform.clone();
        platform.add_message_callback(Box::new(move |event| {
            imgui_platform_clone
                .as_ref()
                .borrow_mut()
                .handle_event(&event);
        }));

        imgui_platform
    }

    pub fn new_frame(&self) {
        self.update_display_size(&self.window);
        self.update_cursor_shape();
        self.update_cursor_pos();
    }

    fn prepare_frame(&self, io: &mut Io) {
        self.winit_platform
            .prepare_frame(io, &self.window)
            .expect("Failed to prepare frame");
        self.window.request_redraw();
    }

    fn handle_event(&mut self, event: &Event<()>) {
        let mut context = self.context.as_ref().borrow_mut();
        let io = context.io_mut();
        match event {
            Event::MainEventsCleared => {
                self.prepare_frame(io);
            }
            Event::RedrawRequested(_) => {}
            // interprete touch events as mouse input
            Event::WindowEvent {
                event:
                    WindowEvent::Touch(Touch {
                        phase,
                        location: PhysicalPosition { x, y },
                        id: 0,
                        ..
                    }),
                window_id,
            } => {
                io.mouse_pos = [*x as f32, *y as f32];
                let state = match *phase {
                    TouchPhase::Started => ElementState::Pressed,
                    TouchPhase::Moved => ElementState::Pressed,
                    TouchPhase::Ended => ElementState::Released,
                    TouchPhase::Cancelled => ElementState::Released,
                };
                let mouse_input: Event<()> = Event::WindowEvent {
                    event: WindowEvent::MouseInput {
                        device_id: unsafe { DeviceId::dummy() },
                        state,
                        button: MouseButton::Left,
                        modifiers: ModifiersState::empty(),
                    },
                    window_id: *window_id,
                };
                self.winit_platform
                    .handle_event(io, &self.window, &mouse_input);
            }
            event => self.winit_platform.handle_event(io, &self.window, event),
        }
    }

    fn update_display_size(&self, window: &Window) {
        let mut context = self.context.as_ref().borrow_mut();
        let size = window.inner_size();
        context.io_mut().display_size = [size.width as f32, size.height as f32];
    }

    fn update_cursor_shape(&self) {}

    fn update_cursor_pos(&self) {}
}
