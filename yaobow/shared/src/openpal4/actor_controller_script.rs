//! Protosept-authored `Pal4ActorController` glue.
//!
//! Mirrors the `pal4_debug` module's split: the auto-generated
//! reverse-wrap helper is re-exported under the friendlier
//! `wrap_actor_controller` name. The host calls the script's
//! `make_actor_controller(ctx)` factory to mint a
//! `box<IPal4ActorController>`, then reverse-wraps it via
//! `wrap_actor_controller` and attaches the result as an
//! `IPal4ActorController` (which QIs to `IComponent`) on the player
//! entity.

/// Auto-generated reverse-wrap helper for `IPal4ActorController`,
/// re-exported under the historical name.
pub use crate::script_bridges::openpal4::wrap_pal4_actor_controller as wrap_actor_controller;
