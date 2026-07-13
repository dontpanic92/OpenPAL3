//! Isolated stub for a future drag-and-drop "drop a `.yapatch` onto
//! the window" hook.
//!
//! This is **not wired up**. As of this writing, `radiance`'s
//! `winit`-backed application bootstrap
//! (`radiance::radiance::create_radiance_engine` /
//! `radiance/src/application/winit.rs`) only exposes
//! `Platform::add_window_event_callback` *during* engine construction,
//! before the event loop starts — there is no way for app code
//! (`IApplicationExt`/`Application`) to register a callback for
//! `winit::event::WindowEvent::DroppedFile` /
//! `WindowEvent::HoveredFile` after the engine is already running,
//! short of editing `radiance` itself. This crate is only allowed to
//! own files under `yaobow_asset_patcher/`, so that's out of scope
//! here.
//!
//! Until that engine-level hook exists, the GUI relies exclusively on
//! the native "Open" button (`native-dialog`, see
//! `src/bin/yaobow_asset_patcher.rs`), which is fully reliable across
//! platforms.
//!
//! # Future hook
//!
//! If/when `radiance` grows a post-construction window-event
//! subscription API (e.g. an `IApplicationExt::add_window_event_callback`
//! or a dedicated `on_file_dropped` callback on `Application`), wire it
//! up here:
//!
//! 1. Implement [`FileDropHandler::on_file_dropped`] to validate the
//!    dropped path has a `.yapatch` extension and hand it to the same
//!    `open_patch` codepath the "Open" button uses.
//! 2. Register it from `main()` once the new radiance API exists,
//!    replacing the `unimplemented!()` in [`install_hook`].
//!
//! No other module in this crate depends on this file — deleting it
//! (or leaving it permanently unimplemented) does not affect
//! correctness of the transaction engine or the rest of the GUI.

use std::path::Path;

/// Implemented by whatever the GUI wants notified when a file is
/// dropped onto the window. Kept as a trait (rather than a bare
/// closure type) so a future real implementation can hold state
/// (e.g. a reference back to the GUI's shared app state) without this
/// module needing to know its shape.
pub trait FileDropHandler {
    fn on_file_dropped(&self, path: &Path);
}

/// Would register `handler` with the engine's window-event stream.
/// Not callable today — see the module doc for why. Kept as a named,
/// documented function (rather than omitted entirely) so the future
/// hook has an obvious, discoverable place to land without needing to
/// invent a new module.
#[allow(dead_code)]
pub fn install_hook(_handler: impl FileDropHandler + 'static) {
    unimplemented!(
        "file-drop is not wireable without changes to radiance's application bootstrap; \
         see this module's doc comment for the exact API gap and the intended future hook."
    )
}
