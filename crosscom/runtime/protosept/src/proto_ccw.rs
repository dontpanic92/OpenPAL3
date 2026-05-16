//! Phase 3 (B1b): runtime-typed CCW factory.
//!
//! `crosscom-protosept` reverse-wraps script-side structs that
//! conform to crosscom `@foreign` protos as Rust `ComRc`s. The
//! factory is generic — any interface registered via
//! [`register_proto_ccw`] (or pre-registered by
//! [`crate::install_com_dispatcher`] for the well-known
//! `crosscom.IAction`) can be reverse-wrapped without hand-rolling
//! a per-interface CCW.
//!
//! ## Concepts
//!
//! - A [`ProtoSpec`] describes a crosscom interface: its UUID, a
//!   stable type tag (for diagnostics), and per-method signatures
//!   described by [`MethodSpec`] / [`ArgKind`] / [`RetKind`].
//! - [`register_proto_ccw`] consumes a spec and builds (a) a
//!   per-interface C-callable vtable with shared `IUnknown` slots and
//!   per-method libffi closure trampolines, and (b) a registry entry
//!   keyed by UUID. Registration leaks closures + userdata + vtable
//!   for `'static` lifetime — the bound is the number of registered
//!   IDL interfaces, which is finite.
//! - [`wrap_proto`] (typed) and [`wrap_proto_unknown`] (uuid-keyed)
//!   allocate a [`ProtoCcw`] backed by a script-side root and return
//!   a `ComRc<I>` (or `ComRc<IUnknown>`). The `ProtoCcw`'s vtable
//!   points at the per-interface table; method dispatch re-enters
//!   the runtime via the [`RuntimeHandle`] captured in the CCW's
//!   payload.
//!
//! ## Marshalling surface (Phase 3 minimum)
//!
//! This module currently supports just enough types to cover
//! `radiance.IDirector` and the synthetic protos in
//! `tests/proto_ccw_e2e.rs`:
//!
//! - Arguments: `Int`, `Float`, `Bool`, `Str`, `Foreign`.
//! - Returns:   `Void`, `Int`, `Float`, `Bool`, `OptionalForeign`.
//!
//! Arrays and structs are intentionally unsupported; registrations
//! that reference them error loudly so Phase B5 can extend coverage
//! incrementally.

use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::os::raw::{c_char, c_float, c_int, c_long};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use crosscom::{ComInterface, ComRc, IUnknown, ResultCode};
use libffi::middle::{Cif, Closure, Type};
use p7::interpreter::context::{Context, Data};

use crate::{with_services, HostError, RuntimeHandle};

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Describes a crosscom interface for reverse-wrapping.
#[derive(Debug, Clone)]
pub struct ProtoSpec {
    pub uuid: [u8; 16],
    /// Stable name used in diagnostics (e.g. `"radiance.comdef.IDirector"`).
    pub type_tag: String,
    pub methods: Vec<MethodSpec>,
    /// Optional script-side method name invoked on final CCW
    /// release (when `proto_release` decrements the refcount to
    /// zero). The method is called with zero args; any return is
    /// discarded. Fires *before* the script handle is unrooted and
    /// the CCW Box is freed. If the runtime is already gone at
    /// release time, the hook silently no-ops.
    ///
    /// **Constraint:** the method name must be declared on at least
    /// one of the protocols the script's `struct[...]` conforms to
    /// — p7's `push_proto_method` resolves via the conforming-proto
    /// vtable and cannot reach struct-only methods. If the method
    /// isn't found at release time, the hook logs and no-ops; the
    /// CCW is still freed.
    ///
    /// Used by [`crate::wrap_proto`] consumers (e.g.
    /// `radiance_scripting::wrap_director`) to mirror lifecycle
    /// hooks that hand-rolled CCWs implemented via `Drop`.
    pub release_method: Option<String>,
    /// Additional interface UUIDs that the CCW's `query_interface`
    /// thunk should accept in addition to IUnknown and the primary
    /// `uuid`. Used to honour interface inheritance — e.g. for
    /// `IImmediateDirector` registrations, this contains
    /// `IDirector::INTERFACE_ID` so a `ComRc<IImmediateDirector>`
    /// can be QI'd back to `ComRc<IDirector>` and round-trip through
    /// engine APIs like `SceneManager::set_director(ComRc<IDirector>)`.
    pub additional_query_uuids: Vec<[u8; 16]>,
}

/// One method on a registered interface. `name` must match the
/// script-side proto method (passed to [`Context::push_proto_method`]).
#[derive(Debug, Clone)]
pub struct MethodSpec {
    pub name: String,
    pub args: Vec<ArgKind>,
    pub ret: RetKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgKind {
    Int,
    Float,
    Bool,
    Str,
    /// `box<F>` interface arg. The C-ABI is a `*const *const c_void`
    /// pointing at the interface's vtable; the script side receives
    /// it as a foreign cell tagged with `type_tag`.
    Foreign {
        type_tag: String,
        uuid: [u8; 16],
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetKind {
    Void,
    Int,
    Float,
    Bool,
    /// `F?` interface return. C-ABI is a `*const *const c_void`;
    /// `Null` maps to `null`, otherwise the script's returned box is
    /// recursively reverse-wrapped via [`wrap_proto_unknown`] using
    /// the spec keyed by `uuid`.
    OptionalForeign {
        type_tag: String,
        uuid: [u8; 16],
    },
}

/// Register an interface so [`wrap_proto`] can produce CCWs of it.
///
/// Idempotent for the same UUID — a second call with the same UUID
/// is rejected loudly to surface accidental double-registration.
/// Register an interface so [`wrap_proto`] can produce CCWs of it.
///
/// **Idempotent** — re-registering an already-known UUID is a silent
/// no-op (the first registration wins). This lets convenience
/// wrappers (such as `radiance_scripting::wrap_director`) lazily
/// register their target interface without conflicting with callers
/// that registered it explicitly at startup.
pub fn register_proto_ccw(spec: ProtoSpec) -> Result<(), HostError> {
    validate_spec(&spec)?;
    let mut reg = registry().lock().expect("proto_ccw registry poisoned");
    if reg.contains_key(&spec.uuid) {
        // Phase 4 (B4): silent no-op on duplicate registration so
        // lazy-registering helpers coexist with explicit setup.
        return Ok(());
    }
    let registered = build_registered_proto(spec)?;
    reg.insert(registered.uuid, registered);
    Ok(())
}

pub fn is_proto_registered(uuid: [u8; 16]) -> bool {
    registry()
        .lock()
        .expect("proto_ccw registry poisoned")
        .contains_key(&uuid)
}

/// Pre-register `crosscom.IAction` with the runtime-typed CCW
/// factory. Called by [`crate::install_com_dispatcher`] so the
/// `com.invoke` script-impl-arg path can reverse-wrap any
/// SAM-coerced script closure that crosses the host boundary as a
/// `box<crosscom.IAction>`.
///
/// The IDL declares `IAction::invoke()` with a `void` C-ABI return,
/// so the spec uses [`RetKind::Void`]. The script side declares
/// `invoke(self: ref<IAction>) -> int` per the p7 void→int mapping;
/// the runtime-typed CCW pops and discards that int.
pub fn register_crosscom_iaction() {
    let _ = register_proto_ccw(ProtoSpec {
        uuid: crosscom::IAction::INTERFACE_ID,
        type_tag: "crosscom.IAction".into(),
        methods: vec![MethodSpec {
            name: "invoke".into(),
            args: vec![],
            ret: RetKind::Void,
        }],
        release_method: None,
        additional_query_uuids: vec![],
    });
}

/// Reverse-wrap a script-side `box<F>` value (`Data::ProtoBoxRef` or
/// `Data::BoxRef`) as a Rust-side `ComRc<I>`. `I::INTERFACE_ID` must
/// match the UUID of a previously-registered [`ProtoSpec`].
pub fn wrap_proto<I: ComInterface + 'static>(
    handle: &RuntimeHandle,
    data: Data,
) -> Result<ComRc<I>, HostError> {
    let raw = wrap_proto_raw(handle, data, I::INTERFACE_ID)?;
    Ok(unsafe { ComRc::<I>::from_raw_pointer(raw) })
}

/// UUID-keyed sibling of [`wrap_proto`] for callers that know the
/// interface UUID dynamically (e.g. the `com.invoke` dispatcher).
pub fn wrap_proto_unknown(
    handle: &RuntimeHandle,
    data: Data,
    uuid: [u8; 16],
) -> Result<ComRc<IUnknown>, HostError> {
    let raw = wrap_proto_raw(handle, data, uuid)?;
    Ok(unsafe { ComRc::<IUnknown>::from_raw_pointer(raw) })
}

// ---------------------------------------------------------------------------
// Registry internals
// ---------------------------------------------------------------------------

struct RegisteredProto {
    uuid: [u8; 16],
    #[allow(dead_code)]
    type_tag: String,
    /// Per-interface vtable as a flat array of fn pointers, leaked
    /// for `'static` lifetime. Length is `3 + methods.len()`. Cast
    /// to `*const I::VirtualTable` at handout time; the `#[repr(C)]`
    /// of crosscom-generated vtable structs makes this layout-
    /// compatible.
    vtable_ptr: *const *const c_void,
    /// `'static` copy of `ProtoSpec::release_method`, set when the
    /// proto is registered with a non-None release hook.
    release_method: Option<&'static str>,
    /// `'static` slice (via `Box::leak`) of additional QI UUIDs the
    /// CCW should accept, copied from
    /// [`ProtoSpec::additional_query_uuids`] at registration time.
    additional_query_uuids: &'static [[u8; 16]],
}

// SAFETY: `RegisteredProto` only stores immutable pointers to leaked,
// initialise-once data: the vtable's contents are `'static` fn
// pointers + raw addresses of leaked `Closure` / `MethodUserdata`
// allocations. Reads are wait-free and serialised by the registry's
// `Mutex` on the write side.
unsafe impl Send for RegisteredProto {}
unsafe impl Sync for RegisteredProto {}

fn registry() -> &'static Mutex<HashMap<[u8; 16], RegisteredProto>> {
    static REG: OnceLock<Mutex<HashMap<[u8; 16], RegisteredProto>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn validate_spec(spec: &ProtoSpec) -> Result<(), HostError> {
    for m in &spec.methods {
        for a in &m.args {
            match a {
                ArgKind::Int | ArgKind::Float | ArgKind::Bool | ArgKind::Str => {}
                ArgKind::Foreign { .. } => {}
            }
        }
        match &m.ret {
            RetKind::Void
            | RetKind::Int
            | RetKind::Float
            | RetKind::Bool
            | RetKind::OptionalForeign { .. } => {}
        }
        // Re-asserting the type names is intentionally exhaustive so a
        // future variant addition forces an update here.
        let _ = spec.uuid;
    }
    Ok(())
}

fn build_registered_proto(spec: ProtoSpec) -> Result<RegisteredProto, HostError> {
    let mut vtable: Vec<*const c_void> = Vec::with_capacity(3 + spec.methods.len());
    vtable.push(proto_query_interface as *const c_void);
    vtable.push(proto_add_ref as *const c_void);
    vtable.push(proto_release as *const c_void);

    for method in &spec.methods {
        let cif = build_cif_for(method);
        let userdata: Box<MethodUserdata> = Box::new(MethodUserdata {
            iface_uuid: spec.uuid,
            iface_type_tag: spec.type_tag.clone(),
            method_name: method.name.clone(),
            args: method.args.clone(),
            ret: method.ret.clone(),
        });
        // Leak the userdata for `'static` lifetime; the `Closure`
        // captures `&'static MethodUserdata`.
        let userdata_ptr: &'static MethodUserdata = Box::leak(userdata);

        // Pick a callback whose `R` matches the C-ABI return slot
        // size. libffi writes `*result` based on the Cif's return
        // type, but our Rust closure type still has to match.
        let closure = match method.ret {
            RetKind::Void => Closure::new(cif, method_thunk_void, userdata_ptr),
            RetKind::Int | RetKind::Bool => Closure::new(cif, method_thunk_int, userdata_ptr),
            RetKind::Float => Closure::new(cif, method_thunk_float, userdata_ptr),
            RetKind::OptionalForeign { .. } => Closure::new(cif, method_thunk_ptr, userdata_ptr),
        };
        // Leak the closure too; `code_ptr()` only stays valid as
        // long as the `Closure` lives.
        let code: *const c_void = unsafe { *closure.code_ptr() } as *const c_void;
        let leaked: &'static Closure<'static> = Box::leak(Box::new(closure));
        // Reference `leaked` so the leak is visible if anything
        // structural changes later; the pointer itself is what we
        // need.
        let _ = leaked;
        vtable.push(code);
    }

    let boxed_vtable: Box<[*const c_void]> = vtable.into_boxed_slice();
    let leaked_vtable: &'static [*const c_void] = Box::leak(boxed_vtable);
    let vtable_ptr = leaked_vtable.as_ptr();

    let release_method: Option<&'static str> = spec
        .release_method
        .map(|s| &*Box::leak(s.into_boxed_str()));
    let additional_query_uuids: &'static [[u8; 16]] =
        Box::leak(spec.additional_query_uuids.into_boxed_slice());

    Ok(RegisteredProto {
        uuid: spec.uuid,
        type_tag: spec.type_tag,
        vtable_ptr,
        release_method,
        additional_query_uuids,
    })
}

fn build_cif_for(method: &MethodSpec) -> Cif {
    // First arg is always `this: *const *const c_void`.
    let mut arg_types: Vec<Type> = Vec::with_capacity(1 + method.args.len());
    arg_types.push(Type::pointer());
    for a in &method.args {
        arg_types.push(match a {
            ArgKind::Int => Type::c_int(),
            ArgKind::Float => Type::f32(),
            ArgKind::Bool => Type::c_int(),
            ArgKind::Str => Type::pointer(),
            ArgKind::Foreign { .. } => Type::pointer(),
        });
    }
    let ret_type = match &method.ret {
        RetKind::Void => Type::void(),
        RetKind::Int => Type::c_int(),
        RetKind::Float => Type::f32(),
        RetKind::Bool => Type::c_int(),
        RetKind::OptionalForeign { .. } => Type::pointer(),
    };
    Cif::new(arg_types.into_iter(), ret_type)
}

// ---------------------------------------------------------------------------
// ProtoCcw + IUnknown thunks
// ---------------------------------------------------------------------------

#[repr(C)]
struct ProtoCcw {
    /// `*const *const c_void` cast of the per-interface vtable.
    /// First field, matching every crosscom `Ixxx { vtable: ... }`
    /// shape.
    vtable: *const *const c_void,
    ref_count: AtomicU32,
    payload: ProtoCcwPayload,
}

struct ProtoCcwPayload {
    iface_uuid: [u8; 16],
    root_idx: usize,
    handle: RuntimeHandle,
    /// Copy of the registered proto's additional QI UUIDs. Stored
    /// per CCW so `proto_query_interface` is lock-free on the hot
    /// path of any engine that QIs the active director every frame.
    additional_query_uuids: &'static [[u8; 16]],
}

unsafe extern "system" fn proto_query_interface(
    this: *const *const c_void,
    guid: uuid::Uuid,
    retval: &mut *const *const c_void,
) -> c_long {
    let ccw = &*(this as *const ProtoCcw);
    let bytes = *guid.as_bytes();
    if bytes == IUnknown::INTERFACE_ID
        || bytes == ccw.payload.iface_uuid
        || ccw.payload.additional_query_uuids.iter().any(|u| *u == bytes)
    {
        *retval = this;
        proto_add_ref(this);
        ResultCode::Ok as c_long
    } else {
        *retval = std::ptr::null();
        ResultCode::ENoInterface as c_long
    }
}

unsafe extern "system" fn proto_add_ref(this: *const *const c_void) -> c_long {
    let ccw = &*(this as *const ProtoCcw);
    let prev = ccw.ref_count.fetch_add(1, Ordering::SeqCst);
    (prev + 1) as c_long
}

unsafe extern "system" fn proto_release(this: *const *const c_void) -> c_long {
    let ccw_ref = &*(this as *const ProtoCcw);
    let prev = ccw_ref.ref_count.fetch_sub(1, Ordering::SeqCst);
    if prev == 1 {
        // Drop the CCW. Fire the release hook (if registered)
        // *before* unrooting so the receiver is still live for the
        // dispatch, then unroot and drop the Box. Each step
        // tolerates a runtime that has already been dropped.
        let ccw_box: Box<ProtoCcw> = Box::from_raw(this as *mut ProtoCcw);
        let release_method = release_method_for(ccw_box.payload.iface_uuid);
        let root_idx = ccw_box.payload.root_idx;
        if let Some(method) = release_method {
            let _ = ccw_box.payload.handle.try_with_ctx(|ctx| {
                invoke_release_hook(ctx, root_idx, method);
            });
        }
        let _ = ccw_box
            .payload
            .handle
            .try_with_ctx(|ctx| ctx.remove_external_root(root_idx));
        drop(ccw_box);
    }
    (prev - 1) as c_long
}

fn release_method_for(uuid: [u8; 16]) -> Option<&'static str> {
    registry()
        .lock()
        .expect("proto_ccw registry poisoned")
        .get(&uuid)
        .and_then(|r| r.release_method)
}

fn invoke_release_hook(ctx: &mut Context, root_idx: usize, method: &str) {
    let receiver = match ctx.external_root(root_idx) {
        Some(r) => r,
        None => return,
    };
    if let Err(e) = ctx.push_proto_method(receiver, method, Vec::new()) {
        eprintln!(
            "proto_release: push_proto_method('{}') failed: {:?}",
            method, e
        );
        return;
    }
    if let Err(e) = ctx.resume() {
        eprintln!("proto_release: resume('{}') failed: {:?}", method, e);
        return;
    }
    if let Ok(frame) = ctx.stack_frame_mut() {
        let _ = frame.stack.pop();
    }
}

// ---------------------------------------------------------------------------
// wrap_proto_raw
// ---------------------------------------------------------------------------

fn wrap_proto_raw(
    handle: &RuntimeHandle,
    data: Data,
    uuid: [u8; 16],
) -> Result<*const *const c_void, HostError> {
    match data {
        Data::ProtoBoxRef { .. } | Data::BoxRef { .. } => {}
        other => {
            return Err(HostError::message(format!(
                "wrap_proto: expected ProtoBoxRef / BoxRef, got {:?}",
                other
            )));
        }
    }
    if handle.is_dangling() {
        return Err(HostError::message(
            "wrap_proto called with a dangling RuntimeHandle",
        ));
    }
    let (vtable_ptr, additional_query_uuids) = {
        let reg = registry().lock().expect("proto_ccw registry poisoned");
        let r = reg.get(&uuid).ok_or_else(|| {
            HostError::message(format!(
                "wrap_proto: no ProtoSpec registered for interface UUID {:?}; \
                 call register_proto_ccw before wrapping",
                uuid
            ))
        })?;
        (r.vtable_ptr, r.additional_query_uuids)
    };

    let root_idx = handle
        .try_with_ctx(|ctx| ctx.add_external_root(data.clone()))
        .ok_or_else(|| HostError::message("wrap_proto: runtime gone before rooting"))?;

    let ccw = Box::new(ProtoCcw {
        vtable: vtable_ptr,
        ref_count: AtomicU32::new(1),
        payload: ProtoCcwPayload {
            iface_uuid: uuid,
            root_idx,
            handle: handle.clone(),
            additional_query_uuids,
        },
    });
    let raw = Box::into_raw(ccw) as *const *const c_void;
    Ok(raw)
}

// ---------------------------------------------------------------------------
// Method dispatch: libffi closure callbacks
// ---------------------------------------------------------------------------

struct MethodUserdata {
    iface_uuid: [u8; 16],
    iface_type_tag: String,
    method_name: String,
    args: Vec<ArgKind>,
    ret: RetKind,
}

/// Outcome of a script-side method call, classified for return
/// marshalling.
enum DispatchOutcome {
    Void,
    Int(i64),
    Float(f64),
    Bool(bool),
    /// Returned interface — either `Null` or a script-side box ready
    /// for recursive wrap_proto.
    OptionalForeign(Option<Data>),
    Error(HostError),
}

unsafe extern "C" fn method_thunk_void(
    _cif: &libffi::low::ffi_cif,
    _result: &mut (),
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    let _ = dispatch_method(args, userdata);
}

unsafe extern "C" fn method_thunk_int(
    _cif: &libffi::low::ffi_cif,
    result: &mut c_int,
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    match dispatch_method(args, userdata) {
        DispatchOutcome::Int(i) => *result = i as c_int,
        DispatchOutcome::Bool(b) => *result = b as c_int,
        DispatchOutcome::Error(_) | _ => *result = 0,
    }
}

unsafe extern "C" fn method_thunk_float(
    _cif: &libffi::low::ffi_cif,
    result: &mut c_float,
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    match dispatch_method(args, userdata) {
        DispatchOutcome::Float(f) => *result = f as c_float,
        DispatchOutcome::Error(_) | _ => *result = 0.0,
    }
}

unsafe extern "C" fn method_thunk_ptr(
    _cif: &libffi::low::ffi_cif,
    result: &mut *const c_void,
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    let outcome = dispatch_method(args, userdata);
    *result = std::ptr::null();
    match outcome {
        DispatchOutcome::OptionalForeign(inner) => {
            if let Some(data) = inner {
                // Recursive wrap_proto_unknown for the returned box.
                let uuid = match &userdata.ret {
                    RetKind::OptionalForeign { uuid, .. } => *uuid,
                    _ => return,
                };
                // Re-read the CCW's RuntimeHandle from args[0] so the
                // recursive wrap uses the same runtime.
                let this_slot = *args.add(0);
                let this_pp = *(this_slot as *const *const *const c_void);
                let ccw = &*(this_pp as *const ProtoCcw);
                match wrap_proto_unknown(&ccw.payload.handle, data, uuid) {
                    Ok(rc) => {
                        // ComRc -> raw pointer. ComRc::into_raw consumes the
                        // ref count; we transfer the strong ref to the caller.
                        let raw: *const *const c_void = rc.into_raw();
                        *result = raw as *const c_void;
                    }
                    Err(err) => {
                        eprintln!(
                            "method_thunk_ptr: recursive wrap_proto failed for '{}': {}",
                            userdata.method_name, err
                        );
                    }
                }
            }
        }
        DispatchOutcome::Error(err) => {
            eprintln!(
                "method_thunk_ptr: dispatch '{}.{}' failed: {}",
                userdata.iface_type_tag, userdata.method_name, err
            );
        }
        _ => {}
    }
}

unsafe fn dispatch_method(
    args: *const *const c_void,
    userdata: &MethodUserdata,
) -> DispatchOutcome {
    // libffi passes `args` as an array of slot pointers. args.add(i)
    // points to the i-th element of that array, and *args.add(i) is
    // the slot pointer itself (a `*const c_void` pointing at the
    // arg's storage). Dereference once more with the right type to
    // read the actual value.
    let this_slot = *args.add(0);
    let this_pp = *(this_slot as *const *const *const c_void);
    let ccw = &*(this_pp as *const ProtoCcw);

    // Re-enter the runtime *first*: marshalling `Foreign` args uses
    // `with_services` which requires the host services scope to be
    // active. `RuntimeAccess::with_ctx` (implemented on `ScriptHost`)
    // installs `scope` + `scope_context` for the duration of the
    // closure, which is exactly what marshalling and the subsequent
    // `push_proto_method`/`resume` both need.
    let root_idx = ccw.payload.root_idx;
    let method_name = userdata.method_name.clone();
    let ret_kind = userdata.ret.clone();
    let arg_kinds = userdata.args.clone();
    let arg_slot_ptrs: Vec<*const c_void> = (0..userdata.args.len())
        .map(|i| *args.add(1 + i))
        .collect();

    let outcome = ccw
        .payload
        .handle
        .try_with_ctx(|ctx| {
            // Marshal args inside the scope so `with_services` works
            // for any `Foreign` arg that needs to intern its ComRc,
            // and so `ctx.alloc_foreign` can wrap interned handles
            // into proper `box<F>` ProtoBoxRefs.
            let mut marshalled: Vec<Data> = Vec::with_capacity(arg_kinds.len());
            for (i, kind) in arg_kinds.iter().enumerate() {
                let slot_ptr = arg_slot_ptrs[i];
                match marshal_arg_in(ctx, kind, slot_ptr) {
                    Ok(d) => marshalled.push(d),
                    Err(err) => return DispatchOutcome::Error(err),
                }
            }
            invoke_script_method(ctx, root_idx, &method_name, marshalled, &ret_kind)
        })
        .unwrap_or(DispatchOutcome::Error(HostError::message(
            "runtime dropped before method dispatch",
        )));
    outcome
}

fn invoke_script_method(
    ctx: &mut Context,
    root_idx: usize,
    method_name: &str,
    args: Vec<Data>,
    ret: &RetKind,
) -> DispatchOutcome {
    let receiver = match ctx.external_root(root_idx) {
        Some(r) => r,
        None => {
            return DispatchOutcome::Error(HostError::message(format!(
                "{}: external root {} is empty",
                method_name, root_idx
            )));
        }
    };
    if let Err(e) = ctx.push_proto_method(receiver, method_name, args) {
        return DispatchOutcome::Error(HostError::message(format!(
            "{}: push_proto_method failed: {:?}",
            method_name, e
        )));
    }
    if let Err(e) = ctx.resume() {
        return DispatchOutcome::Error(HostError::message(format!(
            "{}: resume failed: {:?}",
            method_name, e
        )));
    }

    // Pop the script return per ret_kind.
    let frame = match ctx.stack_frame_mut() {
        Ok(f) => f,
        Err(e) => {
            return DispatchOutcome::Error(HostError::message(format!(
                "{}: stack_frame_mut: {:?}",
                method_name, e
            )));
        }
    };
    match ret {
        RetKind::Void => {
            let _ = frame.stack.pop();
            DispatchOutcome::Void
        }
        RetKind::Int => match frame.stack.pop() {
            Some(Data::Int(i)) => DispatchOutcome::Int(i),
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected Int return, got {:?}",
                method_name, other
            ))),
        },
        RetKind::Float => match frame.stack.pop() {
            Some(Data::Float(f)) => DispatchOutcome::Float(f),
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected Float return, got {:?}",
                method_name, other
            ))),
        },
        RetKind::Bool => match frame.stack.pop() {
            Some(Data::Int(i)) => DispatchOutcome::Bool(i != 0),
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected Bool (Int) return, got {:?}",
                method_name, other
            ))),
        },
        RetKind::OptionalForeign { .. } => match frame.stack.pop() {
            Some(Data::Null) => DispatchOutcome::OptionalForeign(None),
            Some(Data::Some(inner)) => {
                // `inner: Rc<Data>`; clone the inner Data out (the
                // recursive wrap_proto will root it again).
                DispatchOutcome::OptionalForeign(Some((*inner).clone()))
            }
            Some(d @ Data::ProtoBoxRef { .. }) | Some(d @ Data::BoxRef { .. }) => {
                // Tolerate bare-box returns (the script's `return self` /
                // `return some_box` pattern); director.md A2 flags this
                // as an explicit p7 ergonomic gap.
                DispatchOutcome::OptionalForeign(Some(d))
            }
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected OptionalForeign return, got {:?}",
                method_name, other
            ))),
        },
    }
}

unsafe fn marshal_arg_in(
    ctx: &mut Context,
    kind: &ArgKind,
    slot_ptr: *const c_void,
) -> Result<Data, HostError> {
    match kind {
        ArgKind::Int => {
            let p = slot_ptr as *const c_int;
            Ok(Data::Int(*p as i64))
        }
        ArgKind::Float => {
            let p = slot_ptr as *const c_float;
            Ok(Data::Float(*p as f64))
        }
        ArgKind::Bool => {
            let p = slot_ptr as *const c_int;
            Ok(Data::Int(if *p != 0 { 1 } else { 0 }))
        }
        ArgKind::Str => {
            let p = slot_ptr as *const *const c_char;
            let raw = *p;
            if raw.is_null() {
                Ok(Data::string(""))
            } else {
                let s = CStr::from_ptr(raw).to_string_lossy().into_owned();
                Ok(Data::string(s))
            }
        }
        ArgKind::Foreign { type_tag, .. } => {
            let p = slot_ptr as *const *const *const c_void;
            let com_ptr: *const *const c_void = *p;
            if com_ptr.is_null() {
                return Err(HostError::message(format!(
                    "Foreign arg for '{}' is null",
                    type_tag
                )));
            }
            let unk_vtbl = *(com_ptr as *const *const crosscom::IUnknownVirtualTable);
            let guid = uuid::Uuid::from_bytes(IUnknown::INTERFACE_ID);
            let mut raw_unk: *const *const c_void = std::ptr::null();
            let hr = ((*unk_vtbl).query_interface)(com_ptr as *const c_void, guid, &mut raw_unk);
            if hr != 0 || raw_unk.is_null() {
                return Err(HostError::message(format!(
                    "Foreign arg for '{}' did not expose IUnknown (hr={})",
                    type_tag, hr
                )));
            }
            let unk_rc = ComRc::<IUnknown>::from_raw_pointer(raw_unk);
            let handle_id = with_services(|s| s.com_table_mut().intern(unk_rc))?;
            // Wrap the interned handle as a `box<F>` ProtoBoxRef so
            // the script's parameter receives a proper foreign box
            // (not a bare `Data::Foreign`, which p7's `push_proto_method`
            // would not box-wrap).
            ctx.alloc_foreign(type_tag, handle_id).map_err(|e| {
                // Undo the add_ref baked into intern() so a failed
                // alloc doesn't leak the handle.
                let _ = with_services(|s| s.com_table_mut().release(handle_id));
                HostError::message(format!(
                    "alloc_foreign('{}') failed: {:?}",
                    type_tag, e
                ))
            })
        }
    }
}

// Keep the `c_long` import used by the IUnknown thunks alive even if
// the `lints unused-import` heuristic gets confused.
#[allow(dead_code)]
fn _import_anchor() {
    let _ = std::mem::size_of::<c_long>();
}
