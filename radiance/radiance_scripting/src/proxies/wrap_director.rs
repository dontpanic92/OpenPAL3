//! Phase 4 (B2 + B4): convenience helper that reverse-wraps a
//! script-side `box<radiance.IDirector>` value as a Rust
//! `ComRc<IDirector>` using the runtime-typed CCW factory in
//! `crosscom-protosept`.
//!
//! Lazily registers the [`radiance.comdef.IDirector`](IDirector)
//! `ProtoSpec` on first call (idempotent across many calls, and
//! coexists with explicit `register_proto_ccw` callers — first
//! registration wins).
//!
//! The registered spec carries `release_method: Some("deactivate")`,
//! so the final release of a `ComRc<IDirector>` returned here
//! invokes the script-side `deactivate()` method before unrooting
//! and dropping the CCW. This mirrors
//! [`crate::ScriptedImmediateDirector`]'s `Drop` behaviour, letting
//! later phases substitute a `wrap_director` ComRc into
//! `SceneManager::set_director` without losing deactivation
//! semantics.

use std::sync::OnceLock;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::{
    register_proto_ccw, wrap_proto, ArgKind, HostError, MethodSpec, ProtoSpec, RetKind,
    RuntimeHandle,
};
use p7::interpreter::context::Data;
use radiance::comdef::IDirector;

/// Reverse-wrap a script-side `box<radiance.IDirector>` as a
/// Rust-side `ComRc<IDirector>` backed by the runtime-typed CCW
/// factory. On final release, the CCW invokes the script's
/// `deactivate()` method (if defined) before unrooting.
pub fn wrap_director(
    handle: &RuntimeHandle,
    data: Data,
) -> Result<ComRc<IDirector>, HostError> {
    ensure_registered();
    wrap_proto::<IDirector>(handle, data)
}

fn ensure_registered() {
    static GUARD: OnceLock<()> = OnceLock::new();
    GUARD.get_or_init(|| {
        // `register_proto_ccw` is idempotent (Phase 4 B4); we ignore
        // the result so existing callers that registered IDirector
        // explicitly at startup continue to win.
        let _ = register_proto_ccw(ProtoSpec {
            uuid: IDirector::INTERFACE_ID,
            type_tag: "radiance.comdef.IDirector".into(),
            methods: vec![
                MethodSpec {
                    name: "activate".into(),
                    args: vec![],
                    ret: RetKind::Void,
                },
                MethodSpec {
                    name: "update".into(),
                    args: vec![ArgKind::Float],
                    ret: RetKind::OptionalForeign {
                        type_tag: "radiance.comdef.IDirector".into(),
                        uuid: IDirector::INTERFACE_ID,
                    },
                },
            ],
            release_method: Some("deactivate".into()),
            additional_query_uuids: vec![],
        });
    });
}
