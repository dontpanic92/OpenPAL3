extern crate winapi;
use std::ptr::null_mut;
use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::{HWND, POINT};
use winapi::um::{errhandlingapi, libloaderapi, wingdi, winuser};

fn utf16_z<T: AsRef<str>>(s: T) -> Vec<u16> {
    s.as_ref()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

/// Dynamically resolve a function from a system DLL.
///
/// `dll_name` and `proc_name` must be NUL-terminated. Returns `None` if the
/// DLL cannot be loaded or the export is missing — used so we can call
/// Windows-version-gated APIs without preventing the binary from loading on
/// older OS versions.
unsafe fn load_proc<F: Copy>(dll_name: &str, proc_name: &str) -> Option<F> {
    debug_assert!(dll_name.ends_with('\0'));
    debug_assert!(proc_name.ends_with('\0'));
    debug_assert_eq!(
        std::mem::size_of::<F>(),
        std::mem::size_of::<winapi::shared::minwindef::FARPROC>()
    );
    let dll_w: Vec<u16> = dll_name.encode_utf16().collect();
    let module = libloaderapi::LoadLibraryW(dll_w.as_ptr());
    if module.is_null() {
        return None;
    }
    let addr = libloaderapi::GetProcAddress(module, proc_name.as_ptr() as *const i8);
    if addr.is_null() {
        return None;
    }
    Some(std::mem::transmute_copy::<_, F>(&addr))
}

const WM_CLOSE_WINDOW: u32 = winuser::WM_USER + 1;
pub type MessageCallback = Box<dyn Fn(&winuser::MSG)>;

pub struct Platform {
    instance: HINSTANCE,
    hwnd: HWND,
    dpi_scale: f32,
    msg_callbacks: Vec<MessageCallback>,
    quit_requested: std::rc::Rc<std::cell::Cell<bool>>,
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
            quit_requested: std::rc::Rc::new(std::cell::Cell::new(false)),
        }
    }

    pub fn show_error_dialog(title: &str, msg: &str) {
        let msg_w = utf16_z(msg);
        let title_w = utf16_z(title);
        unsafe {
            winuser::MessageBoxW(
                null_mut(),
                msg_w.as_ptr(),
                title_w.as_ptr(),
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
                    winuser::PeekMessageW(&mut msg, null_mut(), 0, 0, winuser::PM_REMOVE) != 0;
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

    pub fn run_event_loop<F: FnMut()>(&self, mut update_engine: F) {
        loop {
            if self.quit_requested.get() {
                break;
            }
            if !self.process_message() {
                break;
            }

            update_engine();
        }
    }

    pub fn request_exit(&self) {
        self.quit_requested.set(true);
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

    pub fn set_title(&self, title: &str) {
        let title_w = utf16_z(title);
        unsafe {
            winuser::SetWindowTextW(self.hwnd, title_w.as_ptr());
        }
    }

    fn set_dpi_awareness() {
        // Resolve DPI-awareness APIs at runtime so the binary still loads on
        // Windows 7/8/8.1/early-10, which lack SetProcessDpiAwarenessContext.
        unsafe {
            // 1. user32!SetProcessDpiAwarenessContext (Windows 10, 1607+)
            type SetProcessDpiAwarenessContextFn =
                unsafe extern "system" fn(isize) -> i32;
            const DPI_AWARENESS_CONTEXT_SYSTEM_AWARE: isize = -2;
            if let Some(f) = load_proc::<SetProcessDpiAwarenessContextFn>(
                "user32.dll\0",
                "SetProcessDpiAwarenessContext\0",
            ) {
                if f(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) != 0 {
                    return;
                }
            }

            // 2. shcore!SetProcessDpiAwareness (Windows 8.1+)
            type SetProcessDpiAwarenessFn = unsafe extern "system" fn(u32) -> i32;
            const PROCESS_SYSTEM_DPI_AWARE: u32 = 1;
            const S_OK: i32 = 0;
            if let Some(f) = load_proc::<SetProcessDpiAwarenessFn>(
                "shcore.dll\0",
                "SetProcessDpiAwareness\0",
            ) {
                if f(PROCESS_SYSTEM_DPI_AWARE) == S_OK {
                    return;
                }
            }

            // 3. user32!SetProcessDPIAware (Windows Vista/7)
            type SetProcessDpiAwareFn = unsafe extern "system" fn() -> i32;
            if let Some(f) = load_proc::<SetProcessDpiAwareFn>(
                "user32.dll\0",
                "SetProcessDPIAware\0",
            ) {
                let _ = f();
            }
        }
    }

    fn create_window(instance: HINSTANCE, title: &str) -> HWND {
        let class_name_w = utf16_z(WINDOW_CLASS_NAME);
        let title_w = utf16_z(title);
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
                lpszClassName: class_name_w.as_ptr(),
            };

            winuser::RegisterClassW(&wnd_class);
            winuser::CreateWindowExW(
                winuser::WS_EX_OVERLAPPEDWINDOW,
                class_name_w.as_ptr(),
                title_w.as_ptr(),
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

fn get_dpi(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let dc = winuser::GetDC(null_mut());
        let x = wingdi::GetDeviceCaps(dc, wingdi::LOGPIXELSX);
        let y = wingdi::GetDeviceCaps(dc, wingdi::LOGPIXELSY);
        winuser::ReleaseDC(null_mut(), dc);

        (x, y)
    }
}
