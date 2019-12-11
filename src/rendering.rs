mod engine;
mod backend;
mod entity;
mod scene;

use engine::RuntimeEngine;

pub trait Engine {
    fn render(&mut self);
}

pub fn create(window: &Window) -> Result<Box<dyn Engine>, Box<dyn std::error::Error>>
{
    Ok(Box::new(RuntimeEngine::new(window)?))
}

#[cfg(target_os = "windows")]
pub struct Window {
    pub hwnd: winapi::shared::windef::HWND,
}
