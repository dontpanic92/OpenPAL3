pub mod core_engine;
pub mod backend;
mod vertex;

pub use vertex::Vertex;
pub use core_engine::CoreRenderingEngine;

pub trait RenderingEngine {
    fn render(&mut self);
}

pub fn create_core_rendering_engine<TBackend: backend::RenderingBackend>(
    window: &Window
) -> Result<CoreRenderingEngine<TBackend>, Box<dyn std::error::Error>>
{
    Ok(CoreRenderingEngine::<TBackend>::new(window)?)
}

#[cfg(target_os = "windows")]
pub struct Window {
    pub hwnd: winapi::shared::windef::HWND,
}
