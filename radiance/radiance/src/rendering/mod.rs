mod engine;
mod factory;
mod material;
mod platform;
mod render_object;
mod render_target;
mod rendering_component;
mod sampler;
mod shader;
mod sprite;
mod texture;
mod vertex_buffer;
mod video_player;

#[cfg(vulkan)]
mod vulkan;

#[cfg(vitagl)]
mod vitagl;

pub use engine::{CapturedFrame, RenderingEngine};
pub use factory::ComponentFactory;
pub use material::{
    BlendMode, CullMode, DepthMode, GradientYMaterialDef, GrassMaterialDef, LightMapMaterialDef,
    LitMaterialDef, MaterialDef, MaterialDefBuilder, MaterialKey, MaterialParams,
    Pal3ActorMaterialDef, SimpleMaterialDef, TerrainLayer, TerrainSplatMaterialDef,
    Pal3GeomMaterialDef, Pal3PropMaterialDef,
};
pub use platform::Window;
pub use render_object::{RenderObject, RenderObjectHandle};
pub use render_target::RenderTarget;
pub use rendering_component::RenderingComponent;
pub use sampler::{AddressMode, FilterMode, MipmapMode, SamplerDef};
pub use shader::{Shader, ShaderProgram};
pub use sprite::Sprite;
pub use texture::{AlphaKind, Texture, TextureDef, TextureStore};
pub use vertex_buffer::{VertexBuffer, VertexComponents};
pub use video_player::VideoPlayer;

#[cfg(vitagl)]
pub use vitagl::VitaGLRenderingEngine;
#[cfg(vulkan)]
pub use vulkan::VulkanRenderingEngine;

/// Engine-side mirror of the user-facing `SceneScaleMode` flag. Lives
/// here (rather than in the downstream `shared` crate) so the radiance
/// engine has no upward dependency. The host application is expected
/// to translate its user config into this enum at engine construction.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SceneScaleMode {
    /// Render the 3D scene at the physical swapchain extent. Default.
    #[default]
    Native,
    /// Render the 3D scene at the supplied logical extent and upscale
    /// (LINEAR blit) to the swapchain image. Imgui still renders at
    /// the native swapchain extent.
    Logical,
}

/// Rendering-engine construction options. Forwarded by
/// `radiance::create_radiance_engine` into the active backend.
#[derive(Copy, Clone, Debug, Default)]
pub struct RenderingEngineOptions {
    pub scene_scale_mode: SceneScaleMode,
    /// Logical extent in pixels for `SceneScaleMode::Logical`. Must
    /// be `Some` when `scene_scale_mode == Logical`; ignored
    /// otherwise. The host application is responsible for deriving
    /// this from its windowing system (e.g.
    /// `window.inner_size() / scale_factor` on winit).
    pub logical_extent: Option<(u32, u32)>,
}
