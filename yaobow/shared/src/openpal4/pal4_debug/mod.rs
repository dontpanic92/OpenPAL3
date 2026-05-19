//! Protosept-authored PAL4 debug overlay bridge.
//!
//! The script side (`box<pal4_debug.IPal4DebugOverlay>`) owns the imgui
//! window. Rust only owns the data — see [`context::Pal4DebugContext`]
//! — and a one-shot reverse-wrap helper that turns the script box into
//! a `ComRc<IPal4DebugOverlay>` the `OpenPAL4Director` can call each
//! frame.

pub mod context;
pub mod wrap_overlay;

pub use context::{
    create_debug_session, Pal4DebugContext, Pal4DebugSession, Pal4DebugSnapshot, Pal4DebugState,
};
pub use wrap_overlay::wrap_overlay;

/// p7 binding source generated from `pal4_debug.idl`. Hosts must
/// register this with `ScriptHost::add_binding("pal4_debug", ...)`
/// before loading any script that `import pal4_debug;`.
pub const PAL4_DEBUG_P7: &str = include_str!(concat!(env!("OUT_DIR"), "/shared_pal4_debug.p7"));
