use std::{cell::RefCell, collections::HashMap, ptr::null_mut, rc::Rc, time::Duration};

use imgui::{BackendFlags, ConfigFlags, Context, ImString, Key};
use winapi::{
    shared::{
        minwindef::{HINSTANCE, UINT},
        windef::{HWND, POINT, RECT},
    },
    um::winuser::{
        self, IDC_ARROW, IDC_HAND, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
        IDC_SIZENWSE, IDC_SIZEWE, MSG,
    },
};
use winuser::GetClientRect;

use crate::application::Platform;

pub struct ImguiPlatform {
    context: Rc<RefCell<Context>>,
    hinstance: HINSTANCE,
    hwnd: HWND,
    last_mouse_cursor: i32,
}

impl ImguiPlatform {
    pub fn new(context: Rc<RefCell<Context>>, platform: &mut Platform) -> Rc<RefCell<Self>> {
        Self::setup_platform(&mut context.borrow_mut());
        let imgui_platform = Rc::new(RefCell::new(Self {
            context,
            hinstance: platform.hinstance(),
            hwnd: platform.hwnd(),
            last_mouse_cursor: -1,
        }));

        let imgui_platform_clone = imgui_platform.clone();
        platform.add_message_callback(Box::new(move |msg| {
            imgui_platform_clone.borrow_mut().process_message(msg);
        }));

        imgui_platform
    }

    pub fn new_frame(&mut self, delta_sec: f32) {
        self.update_delta_time(delta_sec);
        self.update_display_size();
        self.update_cursor_shape();
        self.update_cursor_pos();
    }

    fn setup_platform(context: &mut Context) {
        context.set_platform_name(Some(ImString::from(format!(
            "radiance-imgui-windows {}",
            env!("CARGO_PKG_VERSION"),
        ))));

        let io = context.io_mut();
        io.display_size = [1024., 768.];
        io.backend_flags.insert(BackendFlags::HAS_MOUSE_CURSORS);
        io.backend_flags.insert(BackendFlags::HAS_SET_MOUSE_POS);
        io[Key::Tab] = winuser::VK_TAB as _;
        io[Key::LeftArrow] = winuser::VK_LEFT as _;
        io[Key::RightArrow] = winuser::VK_RIGHT as _;
        io[Key::UpArrow] = winuser::VK_UP as _;
        io[Key::DownArrow] = winuser::VK_DOWN as _;
        io[Key::Home] = winuser::VK_HOME as _;
        io[Key::End] = winuser::VK_END as _;
        io[Key::Insert] = winuser::VK_INSERT as _;
        io[Key::Delete] = winuser::VK_DELETE as _;
        io[Key::Backspace] = winuser::VK_BACK as _;
        io[Key::Space] = winuser::VK_SPACE as _;
        io[Key::Enter] = winuser::VK_RETURN as _;
        io[Key::Escape] = winuser::VK_ESCAPE as _;
        io[Key::A] = 'A' as _;
        io[Key::C] = 'C' as _;
        io[Key::V] = 'V' as _;
        io[Key::X] = 'X' as _;
        io[Key::Y] = 'Y' as _;
        io[Key::Z] = 'Z' as _;
    }

    fn process_message(&mut self, msg: &MSG) {
        if msg.hwnd == self.hwnd {
            self.process_message_internal(msg);
        }
    }

    fn update_delta_time(&mut self, delta_sec: f32) {
        let mut context = self.context.borrow_mut();
        let io = context.io_mut();
        io.update_delta_time(Duration::from_secs_f32(delta_sec));
    }

    fn update_display_size(&mut self) {
        let mut context = self.context.borrow_mut();

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe {
            GetClientRect(self.hwnd, &mut rect);
        }
        context.io_mut().display_size = [
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        ];
    }

    fn update_cursor_shape(&mut self) {
        let mut context = self.context.borrow_mut();

        let mouse_cursor = unsafe { imgui::sys::igGetMouseCursor() };
        if mouse_cursor == self.last_mouse_cursor {
            return;
        } else {
            self.last_mouse_cursor = mouse_cursor;
        }

        let io = context.io_mut();
        if io
            .config_flags
            .contains(ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            return;
        }

        if io.mouse_draw_cursor || mouse_cursor == -1 {
            unsafe { winuser::SetCursor(null_mut()) };
        } else {
            let cursor_name;
            match mouse_cursor {
                imgui::sys::ImGuiMouseCursor_Arrow => cursor_name = IDC_ARROW,
                imgui::sys::ImGuiMouseCursor_TextInput => cursor_name = IDC_IBEAM,
                imgui::sys::ImGuiMouseCursor_ResizeAll => cursor_name = IDC_SIZEALL,
                imgui::sys::ImGuiMouseCursor_ResizeEW => cursor_name = IDC_SIZEWE,
                imgui::sys::ImGuiMouseCursor_ResizeNS => cursor_name = IDC_SIZENS,
                imgui::sys::ImGuiMouseCursor_ResizeNESW => cursor_name = IDC_SIZENESW,
                imgui::sys::ImGuiMouseCursor_ResizeNWSE => cursor_name = IDC_SIZENWSE,
                imgui::sys::ImGuiMouseCursor_Hand => cursor_name = IDC_HAND,
                imgui::sys::ImGuiMouseCursor_NotAllowed => cursor_name = IDC_NO,
                _ => cursor_name = IDC_ARROW,
            }
            unsafe { winuser::SetCursor(winuser::LoadCursorW(self.hinstance, cursor_name)) };
        }
    }

    fn update_cursor_pos(&mut self) {
        let mut context = self.context.borrow_mut();
        let io = context.io_mut();

        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            if winuser::GetCursorPos(&mut point) != 0
                && winuser::ScreenToClient(self.hwnd, &mut point) != 0
            {
                io.mouse_pos = [point.x as f32, point.y as f32];
            }
        }
    }

    fn process_message_internal(&mut self, msg: &MSG) {
        let mut context = self.context.borrow_mut();
        let io = context.io_mut();
        match msg.message {
            winuser::WM_LBUTTONDOWN
            | winuser::WM_LBUTTONDBLCLK
            | winuser::WM_RBUTTONDOWN
            | winuser::WM_RBUTTONDBLCLK
            | winuser::WM_MBUTTONDOWN
            | winuser::WM_MBUTTONDBLCLK => {
                let button = IMGUI_MOUSE_BUTTON_MAP.get(&msg.message).unwrap();
                io.mouse_down[*button as usize] = true;
            }
            winuser::WM_LBUTTONUP | winuser::WM_RBUTTONUP | winuser::WM_MBUTTONUP => {
                let button = IMGUI_MOUSE_BUTTON_MAP.get(&msg.message).unwrap();
                io.mouse_down[*button as usize] = false;
            }
            winuser::WM_MOUSEWHEEL => {
                io.mouse_wheel += winuser::GET_WHEEL_DELTA_WPARAM(msg.wParam) as f32
                    / winuser::WHEEL_DELTA as f32;
            }
            winuser::WM_MOUSEHWHEEL => {
                io.mouse_wheel_h += winuser::GET_WHEEL_DELTA_WPARAM(msg.wParam) as f32
                    / winuser::WHEEL_DELTA as f32;
            }
            winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
                if msg.wParam < 256 {
                    io.keys_down[msg.wParam] = true;
                }

                match msg.wParam as i32 {
                    winuser::VK_CONTROL => io.key_ctrl = true,
                    winuser::VK_SHIFT => io.key_shift = true,
                    winuser::VK_MENU => io.key_alt = true,
                    _ => {}
                }
            }
            winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
                if msg.wParam < 256 {
                    io.keys_down[msg.wParam] = false;
                }

                match msg.wParam as i32 {
                    winuser::VK_CONTROL => io.key_ctrl = false,
                    winuser::VK_SHIFT => io.key_shift = false,
                    winuser::VK_MENU => io.key_alt = false,
                    _ => {}
                }
            }
            winuser::WM_CHAR => {
                let ch = std::char::from_u32(msg.wParam as u32);
                if ch.is_some() {
                    io.add_input_character(ch.unwrap());
                }
            }
            _ => {}
        }
    }
}

lazy_static! {
    pub static ref IMGUI_MOUSE_BUTTON_MAP: HashMap<UINT, i32> = create_mouse_button_hashmap();
}

fn create_mouse_button_hashmap() -> HashMap<UINT, i32> {
    let mut map = HashMap::new();
    map.insert(winuser::WM_LBUTTONDOWN, 0);
    map.insert(winuser::WM_LBUTTONDBLCLK, 0);
    map.insert(winuser::WM_LBUTTONUP, 0);
    map.insert(winuser::WM_RBUTTONDOWN, 1);
    map.insert(winuser::WM_RBUTTONDBLCLK, 1);
    map.insert(winuser::WM_RBUTTONUP, 1);
    map.insert(winuser::WM_MBUTTONDOWN, 2);
    map.insert(winuser::WM_MBUTTONDBLCLK, 2);
    map.insert(winuser::WM_MBUTTONUP, 2);
    map
}
