//! Reverse-wrap a script-side `box<pal4_debug.IPal4DebugOverlay>` as a
//! Rust-side [`ComRc<IPal4DebugOverlay>`] using the runtime-typed CCW
//! factory in `crosscom-protosept`. Mirrors
//! `radiance_scripting::proxies::wrap_im_director` but registers the
//! single-method `IPal4DebugOverlay` proto.
//!
//! The overlay has no release-side hook (no `deactivate` analogue);
//! `release_method` is left `None`, so the CCW just drops the script
//! root when its ref-count reaches zero.

use std::sync::OnceLock;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::{
    register_proto_ccw, wrap_proto, ArgKind, HostError, MethodSpec, ProtoSpec, RetKind,
    RuntimeHandle,
};
use p7::interpreter::context::Data;
use radiance_scripting::comdef::immediate_director::IUiHost;

use crate::openpal4::comdef::pal4_debug::{IPal4DebugContext, IPal4DebugOverlay};

pub fn wrap_overlay(
    handle: &RuntimeHandle,
    data: Data,
) -> Result<ComRc<IPal4DebugOverlay>, HostError> {
    ensure_registered();
    wrap_proto::<IPal4DebugOverlay>(handle, data)
}

fn ensure_registered() {
    static GUARD: OnceLock<()> = OnceLock::new();
    GUARD.get_or_init(|| {
        let _ = register_proto_ccw(ProtoSpec {
            uuid: IPal4DebugOverlay::INTERFACE_ID,
            type_tag: "shared.openpal4.comdef.pal4_debug.IPal4DebugOverlay".into(),
            methods: vec![MethodSpec {
                name: "render".into(),
                args: vec![
                    ArgKind::Foreign {
                        type_tag:
                            "radiance_scripting.comdef.immediate_director.IUiHost".into(),
                        uuid: IUiHost::INTERFACE_ID,
                    },
                    ArgKind::Float,
                    ArgKind::Foreign {
                        type_tag: "shared.openpal4.comdef.pal4_debug.IPal4DebugContext".into(),
                        uuid: IPal4DebugContext::INTERFACE_ID,
                    },
                ],
                ret: RetKind::Void,
            }],
            release_method: None,
            additional_query_uuids: vec![],
        });
    });
}
