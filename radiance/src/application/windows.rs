extern crate winapi;
use std::ptr::null_mut;
use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::{HWND, POINT};
use winapi::um::{libloaderapi, errhandlingapi, winuser, wingdi};

macro_rules! utf16_ptr {
    ( $x:expr ) => {
        append_zero($x)
            .encode_utf16()
            .collect::<Vec<u16>>()
            .as_ptr()
   };
}

const WM_CLOSE_WINDOW: u32 = winuser::WM_USER + 1;
pub type MessageCallback = Box<dyn Fn(&winuser::MSG)>;

pub struct Platform {
    instance: HINSTANCE,
    hwnd: HWND,
    dpi_scale: f32,
    msg_callbacks: Vec<MessageCallback>,
}

impl Platform {
    pub fn new() -> Self {
        Self::set_dpi_awareness();
        let instance = unsafe { libloaderapi::GetModuleHandleW(std::ptr::null_mut()) };
        let hwnd = Platform::create_window(instance, "Radiance");
        if hwnd.is_null() {
            println!("{}", unsafe { errhandlingapi::GetLastError() });
        }

        let dpi_scale = get_dpi(hwnd).0 as f32 / 96.;
        Self {
            instance,
            hwnd,
            dpi_scale,
            msg_callbacks: vec![],
        }
    }

    pub fn show_error_dialog(title: &str, msg: &str) {
        unsafe {
            winuser::MessageBoxW(
                null_mut(),
                utf16_ptr!(msg),
                utf16_ptr!(title),
                winuser::MB_OK | winuser::MB_ICONERROR,
            );
        }
    }

    pub fn initialize(&self) {
        unsafe {
            winuser::ShowWindow(self.hwnd, winuser::SW_SHOW);
        }
    }

    pub fn add_message_callback(&mut self, callback: MessageCallback) {
        self.msg_callbacks.push(callback);
    }

    pub fn process_message(&self) -> bool {
        unsafe {
            let mut msg = winuser::MSG {
                hwnd: null_mut(),
                message: 0,
                wParam: 0,
                lParam: 0,
                time: 0,
                pt: POINT { x: 0, y: 0 },
            };
            loop {
                let has_msg =
                    winuser::PeekMessageW(&mut msg, null_mut(), 0, 0, winuser::PM_REMOVE) > 0;
                if !has_msg {
                    return true;
                }

                if msg.message == WM_CLOSE_WINDOW {
                    return false;
                }

                for cb in &self.msg_callbacks {
                    cb(&msg);
                }

                if msg.message != winuser::WM_SYSKEYDOWN {
                    winuser::TranslateMessage(&msg);
                    winuser::DispatchMessageW(&msg);
                }
            }
        }
    }

    pub fn hinstance(&self) -> HINSTANCE {
        self.instance
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    pub fn set_title(&mut self, title: &str) {
        unsafe {
            winuser::SetWindowTextW(self.hwnd, utf16_ptr!(title));
        }
    }

    fn set_dpi_awareness() {
        unsafe {
            winuser::SetProcessDpiAwarenessContext(
                winapi::shared::windef::DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
            );
        }
    }

    fn create_window(instance: HINSTANCE, title: &str) -> HWND {
        unsafe {
            let wnd_class = winuser::WNDCLASSW {
                style: winuser::CS_HREDRAW | winuser::CS_VREDRAW,
                lpfnWndProc: Some(Platform::window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: winuser::LoadIconW(null_mut(), winuser::IDI_APPLICATION),
                hCursor: winuser::LoadCursorW(null_mut(), winuser::IDC_ARROW),
                hbrBackground: null_mut(),
                lpszMenuName: null_mut(),
                lpszClassName: utf16_ptr!(WINDOW_CLASS_NAME),
            };

            winuser::RegisterClassW(&wnd_class);
            winuser::CreateWindowExW(
                winuser::WS_EX_OVERLAPPEDWINDOW,
                utf16_ptr!(WINDOW_CLASS_NAME),
                utf16_ptr!(title),
                winuser::WS_OVERLAPPEDWINDOW,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                1280,
                960,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            )
        }
    }

    extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            winuser::WM_ERASEBKGND => 1,
            winuser::WM_CLOSE => {
                unsafe { winuser::PostMessageW(hwnd, WM_CLOSE_WINDOW, 0, 0) };
                1
            }
            _ => unsafe { winuser::DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }
}

const WINDOW_CLASS_NAME: &str = "RADIANCE_WINDOW";

fn append_zero<T: Into<String>>(s: T) -> String {
    format!("{}\0", s.into())
}

fn get_dpi(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let dc = winuser::GetDC(null_mut());
        let x = wingdi::GetDeviceCaps(dc, wingdi::LOGPIXELSX);
        let y = wingdi::GetDeviceCaps(dc, wingdi::LOGPIXELSY);
        winuser::ReleaseDC(null_mut(), dc);

        (x, y)
    }
}

