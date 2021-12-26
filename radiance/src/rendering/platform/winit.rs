#[derive(Copy, Clone)]
pub struct Window {}

impl Window {
    pub fn size(&self) -> (u32, u32) {
        unsafe { (1280, 720) }
    }
}
