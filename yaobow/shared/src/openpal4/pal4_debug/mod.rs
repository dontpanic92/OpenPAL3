//! Protosept-authored PAL4 debug overlay bridge.
//!
//! The script side (`box<pal4_debug.IPal4DebugOverlay>`) owns the imgui
//! window. Rust only owns the data — see [`context::Pal4DebugContext`]
//! — and a one-shot reverse-wrap helper that turns the script box into
//! a `ComRc<IPal4DebugOverlay>` the `OpenPAL4Director` can call each
//! frame.

pub mod context;

pub use context::{
    Pal4DebugContext, Pal4DebugSession, Pal4DebugSnapshot, Pal4DebugState, create_debug_session,
};
// Auto-generated reverse-wrap helper, re-exported under the historical
// name.
pub use crate::script_bridges::pal4_debug::wrap_pal4_debug_overlay as wrap_overlay;
