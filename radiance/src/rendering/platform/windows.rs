use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser;

#[derive(Copy, Clone)]
pub struct Window {
    pub hwnd: HWND,
}

impl Window {
    pub fn size(&self) -> (u32, u32) {
        unsafe {
            let mut rc: RECT = std::mem::zeroed();
            winuser::GetWindowRect(self.hwnd, &mut rc);
            ((rc.right - rc.left) as u32, (rc.bottom - rc.top) as u32)
        }
    }
}
