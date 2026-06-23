//! The imgui texture-resolver trait moved into `radiance` (so the engine
//! can own a shared resolver on `UiManager`); re-exported here under the
//! historical path for back-compat. New code should prefer
//! `radiance::imgui::TextureResolver`.

pub use radiance::imgui::TextureResolver;
