//! Protosept-authored `Pal4ActorController` glue.
//!
//! Mirrors the `pal4_debug` module's split: the .p7 source is bundled
//! here as a `pub const`, and we re-export the auto-generated
//! `wrap_pal4_actor_controller` helper under the friendlier
//! `wrap_actor_controller` name. The host calls the script's
//! `make_actor_controller(ctx)` factory to mint a
//! `box<IPal4ActorController>`, then reverse-wraps it via
//! `wrap_actor_controller` and attaches the result as an
//! `IPal4ActorController` (which QIs to `IComponent`) on the player
//! entity.

/// Auto-generated reverse-wrap helper for `IPal4ActorController`,
/// re-exported under the historical name.
pub use crate::script_bridges::openpal4::wrap_pal4_actor_controller as wrap_actor_controller;

/// p7 source for the `Pal4ActorController` behavior struct. Hosts
/// load this with `ScriptHost::add_binding` or `load_source` and
/// then call the exported `make_actor_controller(ctx)` function to
/// mint a `box<IPal4ActorController>`.
pub const ACTOR_CONTROLLER_P7: &str = include_str!("../../scripts/openpal4/actor_controller.p7");

/// Auto-generated p7 binding for `openpal4.idl`. Hosts must
/// register this with `ScriptHost::add_binding("openpal4", ...)`
/// before loading any script that `import openpal4;`.
pub const OPENPAL4_P7: &str = include_str!(concat!(env!("OUT_DIR"), "/shared_openpal4.p7"));
