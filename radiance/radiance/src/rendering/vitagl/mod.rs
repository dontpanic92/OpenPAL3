mod factory;
mod material;
mod render_object;
mod shader;
mod texture;
mod vitagl_engine;

// Backend-typed handles surfaced to sibling rendering modules
// (rendering_component, render_object, render_target) so they can store
// concrete VitaGL references alongside the cross-backend trait objects
// without paying a per-frame downcast.
pub(super) use render_object::VitaGLRenderObject;

pub use vitagl_engine::VitaGLRenderingEngine;
