pub mod backend;
pub mod core_engine;
mod vertex;

pub use core_engine::CoreRenderingEngine;
pub use vertex::Vertex;

pub trait RenderingEngine {
    fn render(&mut self);
}

pub fn create_core_rendering_engine<TBackend: backend::RenderingBackend>(
    window: &Window,
) -> Result<CoreRenderingEngine<TBackend>, Box<dyn std::error::Error>> {
    Ok(CoreRenderingEngine::<TBackend>::new(window)?)
}

#[cfg(target_os = "windows")]
pub struct Window {
    pub hwnd: winapi::shared::windef::HWND,
}
