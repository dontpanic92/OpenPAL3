//! Phase 5 (B2-style): convenience helper that reverse-wraps a
//! script-side `box<radiance.IImmediateDirector>` value as a
//! Rust-side `ComRc<IImmediateDirector>` using the runtime-typed
//! CCW factory in `crosscom-protosept`.
//!
//! Mirrors [`crate::wrap_director`] but registers the
//! `IImmediateDirector` proto (activate + update + render_im) with
//! `additional_query_uuids: [IDirector::INTERFACE_ID]` so the
//! returned ComRc can be QI'd back to `ComRc<IDirector>` and passed
//! to engine APIs like `ISceneManager::set_director`.
//!
//! The release hook calls the script-side `deactivate()` method.

use std::sync::OnceLock;

use crosscom::{ComInterface, ComRc};
use crosscom_protosept::{
    register_proto_ccw, wrap_proto, ArgKind, HostError, MethodSpec, ProtoSpec, RetKind,
    RuntimeHandle,
};
use p7::interpreter::context::Data;
use radiance::comdef::IDirector;

use crate::comdef::immediate_director::{IImmediateDirector, IUiHost};

pub fn wrap_im_director(
    handle: &RuntimeHandle,
    data: Data,
) -> Result<ComRc<IImmediateDirector>, HostError> {
    ensure_registered();
    wrap_proto::<IImmediateDirector>(handle, data)
}

fn ensure_registered() {
    static GUARD: OnceLock<()> = OnceLock::new();
    GUARD.get_or_init(|| {
        let _ = register_proto_ccw(ProtoSpec {
            uuid: IImmediateDirector::INTERFACE_ID,
            type_tag:
                "radiance_scripting.comdef.immediate_director.IImmediateDirector".into(),
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
                MethodSpec {
                    name: "render_im".into(),
                    args: vec![
                        ArgKind::Foreign {
                            type_tag:
                                "radiance_scripting.comdef.immediate_director.IUiHost".into(),
                            uuid: IUiHost::INTERFACE_ID,
                        },
                        ArgKind::Float,
                    ],
                    ret: RetKind::Void,
                },
            ],
            release_method: Some("deactivate".into()),
            additional_query_uuids: vec![IDirector::INTERFACE_ID],
        });
    });
}
