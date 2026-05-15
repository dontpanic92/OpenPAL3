//! Shared texture-resolver trait. Used by `services::texture_cache` to
//! implement com-id → `imgui::TextureId` lookup and by `services::ui_host`
//! to access that lookup through a per-frame trait object.

pub trait TextureResolver {
    fn resolve(&mut self, com_id: i64) -> Option<imgui::TextureId>;
}
