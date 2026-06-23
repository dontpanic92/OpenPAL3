//! Shared imgui texture-resolver trait. Maps an engine COM-id (the
//! script-facing handle a `ITexture`/sprite/render-target advertises) to
//! a live `imgui::TextureId` for the current frame.
//!
//! Defined in `radiance` (rather than `radiance_scripting`) so the engine
//! — specifically [`UiManager`](crate::radiance::UiManager) — can hold an
//! engine-owned resolver and expose immediate-mode UI composition without
//! depending on the scripting crate. `radiance_scripting`'s
//! `ImguiTextureCache` is the production implementation.

pub trait TextureResolver {
    fn resolve(&mut self, com_id: i64) -> Option<imgui::TextureId>;
}
