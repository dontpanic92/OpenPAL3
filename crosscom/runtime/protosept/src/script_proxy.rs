//! Adapter that wraps a *script-side* struct (declared in protosept with
//! a foreign-proto conformance via `struct[F] T(...) { ... }`) as a
//! refcounted [`ComRc<I>`](crosscom::ComRc), making it indistinguishable
//! from a host-side implementation at the crosscom boundary.
//!
//! This is the reverse direction of `dispatcher.rs`. The forward
//! direction handles script-calling-host: a `box<F>` on the script side
//! whose dynamic type is a *host* `ComRc<I>` (stored in
//! [`crate::ComObjectTable`]). The reverse direction handles
//! host-calling-script: a `ComRc<I>` on the Rust side whose vtable
//! thunks re-enter the protosept interpreter and dispatch to a script
//! struct's methods.
//!
//! ## Why hand-rolled vtables
//!
//! crosscom's generated `ComObject_*` macros internally
//! `use crate as crosscom`, which only resolves correctly when the
//! macro is expanded inside the `crosscom` crate (or one whose root
//! re-exports the crosscom types under that name).
//! `crosscom-protosept` is an external crate, so we hand-roll the
//! equivalent CCW struct + vtable here. The shape mirrors the macro
//! output line-for-line (see crosscom defs.rs `ComObject_Action!`);
//! only the path prefixes change to absolute `crosscom::` references.
//!
//! ## Lifetime
//!
//! The script handle (`Data::ProtoBoxRef`) is rooted via
//! [`Context::add_external_root`] for as long as the wrapping
//! [`ComRc`](crosscom::ComRc) exists. Releasing the last `ComRc`
//! triggers `Drop` on the inner [`ScriptActionProxy`], which unroots —
//! *if* a script context is currently in scope (via
//! [`crate::scope_context`]).
//!
//! In v1, `ComRc<I>` instances obtained from this adapter MUST be
//! released within the dynamic extent of the same
//! [`crate::scope_context`] that produced them. Storing one past the
//! end of the scope leaks the script handle. This matches the
//! editor-UI scripting plan's "callbacks don't escape" stance for v1;
//! a future revision can lift the limitation by carrying a `Weak`
//! handle to the runtime so `Drop` can unroot even outside any scope.

use std::ffi::c_void;
use std::os::raw::c_long;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{with_context, HostError};
use crosscom::{ComInterface, ComRc, IAction, IActionVirtualTable, IActionVirtualTableCcw, ResultCode};
use p7::interpreter::context::{Context, Data};

/// Wrap a script-side `Data::ProtoBoxRef` (or `BoxRef`) — produced when
/// a script struct conforming to `crosscom.IAction` is passed across
/// the host boundary as `box<crosscom.IAction>` — as a Rust-side
/// [`ComRc<crosscom::IAction>`].
pub fn wrap_action(ctx: &mut Context, data: Data) -> Result<ComRc<IAction>, HostError> {
    match data {
        Data::ProtoBoxRef { .. } | Data::BoxRef { .. } => {
            let root_idx = ctx.add_external_root(data);
            Ok(ActionCcw::into_com_rc(ScriptActionProxy { root_idx }))
        }
        other => Err(HostError::message(format!(
            "wrap_action: expected ProtoBoxRef / BoxRef, got {:?}",
            other
        ))),
    }
}

/// Script-side payload carried by an `IAction`-shaped CCW. Holds the
/// [`Context::add_external_root`] index that pins the script's
/// `Data::ProtoBoxRef` for the proxy's lifetime.
pub struct ScriptActionProxy {
    pub(crate) root_idx: usize,
}

impl Drop for ScriptActionProxy {
    fn drop(&mut self) {
        let _ = with_context(|ctx| {
            ctx.remove_external_root(self.root_idx);
        });
    }
}

// ---------------------------------------------------------------------------
// Hand-rolled CCW for `crosscom::IAction` backed by `ScriptActionProxy`.
// Mirrors `ComObject_Action!` output with absolute `crosscom::` paths.
// ---------------------------------------------------------------------------

#[repr(C)]
struct ActionCcw {
    iface: IAction,
    ref_count: AtomicU32,
    inner: ScriptActionProxy,
}

impl ActionCcw {
    fn into_com_rc(inner: ScriptActionProxy) -> ComRc<IAction> {
        let boxed = Box::new(ActionCcw {
            iface: IAction {
                vtable: &VTABLE_CCW.vtable as *const IActionVirtualTable,
            },
            // ComRc::from_raw_pointer (used below) does not call AddRef;
            // the macro-driven `ComObject::from_object` route runs QI
            // through IUnknown, which AddRefs. We bypass QI for simpler
            // path-independence (see module docs), so seed the count at
            // 1 to match the strong ref the returned ComRc represents.
            ref_count: AtomicU32::new(1),
            inner,
        });
        let raw = Box::into_raw(boxed);
        unsafe { ComRc::<IAction>::from_raw_pointer(raw as *const *const c_void) }
    }
}

unsafe extern "system" fn query_interface(
    this: *const *const c_void,
    guid: uuid::Uuid,
    retval: &mut *const *const c_void,
) -> c_long {
    let object = this as *const ActionCcw;
    let bytes = *guid.as_bytes();
    if bytes == crosscom::IUnknown::INTERFACE_ID || bytes == IAction::INTERFACE_ID {
        *retval = object as *const *const c_void;
        add_ref(object as *const *const c_void);
        ResultCode::Ok as c_long
    } else {
        *retval = std::ptr::null();
        ResultCode::ENoInterface as c_long
    }
}

unsafe extern "system" fn add_ref(this: *const *const c_void) -> c_long {
    let object = &*(this as *const ActionCcw);
    let previous = object.ref_count.fetch_add(1, Ordering::SeqCst);
    (previous + 1) as c_long
}

unsafe extern "system" fn release(this: *const *const c_void) -> c_long {
    let object = &*(this as *const ActionCcw);
    let previous = object.ref_count.fetch_sub(1, Ordering::SeqCst);
    if previous - 1 == 0 {
        drop(Box::from_raw(this as *mut ActionCcw));
    }
    (previous - 1) as c_long
}

unsafe extern "system" fn invoke(this: *const *const c_void) {
    let object = &*(this as *const ActionCcw);
    let root_idx = object.inner.root_idx;
    match with_context(|ctx| invoke_unit_method(ctx, root_idx, "invoke")) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            // Loud-failure on script-side errors: a SAM callback that
            // panics or throws would otherwise silently no-op (the
            // observable symptom in earlier integration runs was "body
            // recorded BodyEnter/BodyExit but produced no inner calls").
            eprintln!("IAction.invoke failed: {}", err);
        }
        Err(err) => {
            eprintln!("IAction.invoke could not find context: {}", err);
        }
    }
}

static VTABLE_CCW: IActionVirtualTableCcw = IActionVirtualTableCcw {
    offset: 0,
    vtable: IActionVirtualTable {
        query_interface,
        add_ref,
        release,
        invoke,
    },
};

/// Invoke a zero-arg, unit-returning method on the script struct held
/// by external-root `root_idx`. Pops the unit return so the host
/// frame's stack stays balanced.
fn invoke_unit_method(
    ctx: &mut Context,
    root_idx: usize,
    method_name: &str,
) -> Result<(), HostError> {
    let receiver = ctx.external_root(root_idx).ok_or_else(|| {
        HostError::message(format!(
            "ScriptActionProxy.{method_name}: external root {root_idx} is empty"
        ))
    })?;
    ctx.push_proto_method(receiver, method_name, Vec::new())
        .map_err(|e| {
            HostError::message(format!(
                "ScriptActionProxy.{method_name}: push_proto_method failed: {e:?}"
            ))
        })?;
    ctx.resume().map_err(|e| {
        HostError::message(format!(
            "ScriptActionProxy.{method_name}: resume failed: {e:?}"
        ))
    })?;
    if let Ok(frame) = ctx.stack_frame_mut() {
        let _ = frame.stack.pop();
    }
    Ok(())
}
