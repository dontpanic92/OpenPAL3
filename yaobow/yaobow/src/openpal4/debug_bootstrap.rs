//! Bootstrap the protosept-authored PAL4 debug overlay.
//!
//! Same recipe as `yaobow_editor::directors::scripted_welcome_page`:
//!
//! 1. Install (or reuse) the engine-attached [`ScriptHost`].
//! 2. Register the `pal4_debug` IDL binding + sibling overlay module.
//! 3. Load `pal4_debug_main.p7`.
//! 4. Build the host-side `IPal4DebugContext` (carries per-frame data)
//!    and pass it to `init(ctx)` on the script side.
//! 5. Reverse-wrap the returned `box<pal4_debug.IPal4DebugOverlay>`
//!    into a `ComRc<IPal4DebugOverlay>` via
//!    [`shared::openpal4::debug::wrap_overlay`].
//!
//! Returns the bundle the director needs to call the overlay each
//! frame, plus the `ScriptHost` so the loader can keep it alive for
//! the duration of the application (the script host is also held by
//! the engine via `ScriptHost::install`, so this is belt-and-braces).

use std::rc::Rc;

use radiance::radiance::CoreRadianceEngine;
use radiance_scripting::{
    with_services, RuntimeAccess, RuntimeHandle, ScriptHost,
};
use shared::openpal4::debug::{create_context, wrap_overlay, Pal4DebugContextInner};
use shared::openpal4::debug::PAL4_DEBUG_P7;

use crate::openpal4::director_bundle::Pal4DebugBootstrap;
use crate::openpal4::script_source::{register_pal4_debug_modules, PAL4_DEBUG_MAIN_P7};

pub fn install(engine: &CoreRadianceEngine) -> Pal4DebugBootstrap {
    let host = ScriptHost::install(engine);

    // Register bindings + sibling modules. Idempotent across calls
    // (`add_binding` overwrites), so installing the debug overlay
    // multiple times in-process (e.g. PAL4 restart) is safe.
    host.add_binding("pal4_debug", PAL4_DEBUG_P7);
    register_pal4_debug_modules(&host);

    host.load_source(PAL4_DEBUG_MAIN_P7)
        .expect("pal4_debug_main.p7 must load successfully");

    let (ctx_inner, ctx_com): (Rc<Pal4DebugContextInner>, _) = create_context();
    let ctx_handle = host.intern(ctx_com.clone());
    let ctx_box = host
        .foreign_box(
            "shared.openpal4.comdef.pal4_debug.IPal4DebugContext",
            ctx_handle,
        )
        .expect("IPal4DebugContext foreign_box must construct");

    let overlay_data = host
        .call_returning_data("init", vec![ctx_box])
        .expect("pal4_debug init() must succeed");

    let runtime_handle = host_runtime_handle(&host);
    let overlay = wrap_overlay(&runtime_handle, overlay_data)
        .expect("wrap_overlay must succeed");

    Pal4DebugBootstrap {
        host,
        bundle: shared::openpal4::director::Pal4DebugBundle {
            overlay,
            overlay_ctx: ctx_com,
            overlay_ctx_inner: ctx_inner,
        },
    }
}

/// Pull a `RuntimeHandle` out of the `ScriptHost`'s services bundle
/// by entering its `RuntimeAccess` scope once. Same pattern as the
/// editor's `host_runtime_handle`.
fn host_runtime_handle(host: &Rc<ScriptHost>) -> RuntimeHandle {
    let mut out = None;
    <ScriptHost as RuntimeAccess>::with_ctx(host, &mut |_ctx| {
        let h = with_services(|s| s.runtime_handle())
            .expect("with_services inside RuntimeAccess scope");
        out = Some(h);
    });
    out.expect("RuntimeAccess::with_ctx ran body")
}
