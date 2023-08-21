mod engine;
mod factory;
mod material;
mod platform;
mod render_object;
mod rendering_component;
mod shader;
mod sprite;
mod texture;
mod vertex_buffer;
mod video_player;

#[cfg(vulkan)]
mod vulkan;

#[cfg(vitagl)]
mod vitagl;

pub use engine::RenderingEngine;
pub use factory::ComponentFactory;
pub use material::{LightMapMaterialDef, Material, MaterialDef, SimpleMaterialDef};
pub use platform::Window;
pub use render_object::RenderObject;
pub use rendering_component::RenderingComponent;
pub use shader::{Shader, ShaderProgram};
pub use sprite::Sprite;
pub use texture::{Texture, TextureDef, TextureStore};
pub use vertex_buffer::{VertexBuffer, VertexComponents};
pub use video_player::VideoPlayer;

#[cfg(vitagl)]
pub use vitagl::VitaGLRenderingEngine;
#[cfg(vulkan)]
pub use vulkan::VulkanRenderingEngine;
