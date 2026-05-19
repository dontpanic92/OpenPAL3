//! Read-only editor access shims around `Pal4Scene`.
//!
//! `Pal4Scene` keeps its inner `ComRc<IScene>` field `pub(crate)` so
//! gameplay callers can't accidentally reach past the wrapper. The
//! editor however only needs the loaded scene (to render an offscreen
//! preview) and discards the gameplay machinery (`events`, `gob`,
//! `script_module`, …). This module exposes a single accessor that
//! consumes the wrapper and returns the inner `IScene`.

use crosscom::ComRc;
use radiance::comdef::IScene;

use super::scene::Pal4Scene;

/// Consume the wrapper and return its inner scene. The wrapper's other
/// fields (NPCs, GOB objects, events, …) are dropped; the entities
/// they hold remain alive through the scene because `Pal4Scene::load`
/// has already called `scene.add_entity(...)` on each one.
pub fn take_scene(scene: Pal4Scene) -> ComRc<IScene> {
    scene.into_inner_scene()
}
