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
//! - Returns:   `Void`, `Int`, `Float`, `Bool`, `Foreign`, `OptionalForeign`.
//!
//! Arrays and structs are intentionally unsupported; registrations
//! that reference them error loudly so Phase B5 can extend coverage
//! incrementally.

use std::alloc::{Layout, alloc, dealloc};
use std::collections::HashMap;
use std::ffi::{CStr, c_void};
use std::mem::size_of;
use std::os::raw::{c_char, c_float, c_int, c_long};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use crosscom::{ComInterface, ComRc, IUnknown, ResultCode};
use libffi::middle::{Cif, Closure, Type};
use p7::interpreter::context::{Context, Data};

use crate::{HostError, RuntimeHandle, with_services};

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
    /// `F` (non-optional) interface return. C-ABI is a
    /// `*const *const c_void`; the script's returned box is recursively
    /// reverse-wrapped via [`wrap_proto_unknown`] using the spec keyed
    /// by `uuid`. Unlike [`RetKind::OptionalForeign`], a `null` / absent
    /// script return is a hard error (the non-optional contract), and
    /// the C-ABI never yields a null pointer on success.
    Foreign {
        type_tag: String,
        uuid: [u8; 16],
    },
    /// `F?` interface return. C-ABI is a `*const *const c_void`;
    /// `Null` maps to `null`, otherwise the script's returned box is
    /// recursively reverse-wrapped via [`wrap_proto_unknown`] using
    /// the spec keyed by `uuid`.
    OptionalForeign {
        type_tag: String,
        uuid: [u8; 16],
    },
    /// `float?` primitive return. C-ABI is a single `c_float`; the
    /// `NaN` bit-pattern is the absence sentinel (mirrors the comdef
    /// gen `Option<f32> ↔ f32::NAN` convention). On dispatch the
    /// libffi thunk writes `f32::NAN` when the script returns
    /// `Data::Null` and the actual value otherwise.
    OptionalFloat,
}

/// Register an interface so [`wrap_proto`] can produce CCWs of it.
///
/// Idempotent for the same UUID — a second call with the same UUID
/// is rejected loudly to surface accidental double-registration.
/// Register an interface so [`wrap_proto`] can produce CCWs of it.
///
/// **Idempotent** — re-registering an already-known UUID is a silent
/// no-op (the first registration wins). This lets convenience
/// wrappers (such as the auto-generated `wrap_director` /
/// `wrap_immediate_director` in `radiance_scripting::script_bridges`)
/// lazily register their target interface without conflicting with
/// callers that registered it explicitly at startup.
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

/// Blueprint for re-instantiating a per-interface vtable at a
/// specific slot index. The QI/AddRef/Release thunks are global —
/// only the leading `offset` field and a fresh per-slot prefix
/// allocation differ between slot indices.
struct VtableBlueprint {
    /// Pre-built shared method code pointers (length == methods.len()).
    method_codes: &'static [*const c_void],
}

// SAFETY: VtableBlueprint stores raw fn pointers to leaked closures.
// All reads are wait-free after publication via the registry mutex.
unsafe impl Send for VtableBlueprint {}
unsafe impl Sync for VtableBlueprint {}

struct RegisteredProto {
    uuid: [u8; 16],
    #[allow(dead_code)]
    type_tag: String,
    blueprint: VtableBlueprint,
    /// `'static` slice (via `Box::leak`) of additional QI UUIDs the
    /// CCW slot should accept in addition to its primary `uuid`. Used
    /// for IDL inheritance (e.g. IImmediateDirector → IDirector) where
    /// the parent interface's vtable is a structural prefix of the
    /// child's.
    additional_query_uuids: &'static [[u8; 16]],
    /// Lazily-minted per-slot-index vtables. Each entry is a leaked
    /// `[isize, *const c_void, ...]` allocation whose stored pointer
    /// (one slot past `offset`) is what the CCW writes into its
    /// interface slot.
    slot_vtables: Mutex<HashMap<usize, SlotVtablePtr>>,
}

/// Wrapper so we can store a raw pointer in a HashMap and still get
/// the right `Send`/`Sync` bounds; the pointer is to leaked memory
/// that lives for the program's lifetime.
#[derive(Copy, Clone)]
struct SlotVtablePtr(*const *const c_void);
unsafe impl Send for SlotVtablePtr {}
unsafe impl Sync for SlotVtablePtr {}

// SAFETY: see SlotVtablePtr / VtableBlueprint comments — all stored
// pointers are leaked, immutable after publication.
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
            | RetKind::OptionalForeign { .. }
            | RetKind::Foreign { .. }
            | RetKind::OptionalFloat => {}
        }
        // Re-asserting the type names is intentionally exhaustive so a
        // future variant addition forces an update here.
        let _ = spec.uuid;
    }
    Ok(())
}

fn build_registered_proto(spec: ProtoSpec) -> Result<RegisteredProto, HostError> {
    // Build the per-method libffi closures once. Each closure is
    // shared across every slot index — only the leading `offset`
    // changes per slot, and the QI/AddRef/Release thunks at the head
    // of the vtable are themselves global fn pointers (their bodies
    // recover the CCW base via the offset field).
    let mut method_codes: Vec<*const c_void> = Vec::with_capacity(spec.methods.len());
    for method in &spec.methods {
        let cif = build_cif_for(method);
        let userdata: Box<MethodUserdata> = Box::new(MethodUserdata {
            iface_type_tag: spec.type_tag.clone(),
            method_name: method.name.clone(),
            args: method.args.clone(),
            ret: method.ret.clone(),
        });
        let userdata_ptr: &'static MethodUserdata = Box::leak(userdata);

        let closure = match method.ret {
            RetKind::Void => Closure::new(cif, method_thunk_void, userdata_ptr),
            RetKind::Int | RetKind::Bool => Closure::new(cif, method_thunk_int, userdata_ptr),
            RetKind::Float | RetKind::OptionalFloat => {
                Closure::new(cif, method_thunk_float, userdata_ptr)
            }
            RetKind::OptionalForeign { .. } | RetKind::Foreign { .. } => {
                Closure::new(cif, method_thunk_ptr, userdata_ptr)
            }
        };
        let code: *const c_void = *closure.code_ptr() as *const c_void;
        let leaked: &'static Closure<'static> = Box::leak(Box::new(closure));
        let _ = leaked;
        method_codes.push(code);
    }
    let method_codes: &'static [*const c_void] = Box::leak(method_codes.into_boxed_slice());

    let additional_query_uuids: &'static [[u8; 16]] =
        Box::leak(spec.additional_query_uuids.into_boxed_slice());

    let registered = RegisteredProto {
        uuid: spec.uuid,
        type_tag: spec.type_tag,
        blueprint: VtableBlueprint { method_codes },
        additional_query_uuids,
        slot_vtables: Mutex::new(HashMap::new()),
    };
    // Pre-mint slot 0 so the common single-interface wrap path never
    // touches the per-spec mutex.
    let _ = mint_slot_vtable(&registered, 0);
    Ok(registered)
}

/// Mint (or look up) the vtable for `slot_index` on `proto`. The
/// returned pointer points one slot past the leading `offset` field,
/// matching the C-ABI shape every consumer expects.
fn mint_slot_vtable(proto: &RegisteredProto, slot_index: usize) -> *const *const c_void {
    {
        let map = proto.slot_vtables.lock().expect("slot_vtables poisoned");
        if let Some(p) = map.get(&slot_index) {
            return p.0;
        }
    }
    // Slow path: build the [offset, qi, add_ref, release, methods...]
    // allocation and cache it. The leading offset is the negated slot
    // index (in *const c_void units) — identical convention to
    // `crosscom::get_object` (which uses `*const isize` strides equal
    // to a pointer-width).
    let mut buf: Vec<*const c_void> = Vec::with_capacity(4 + proto.blueprint.method_codes.len());
    // SAFETY: writing an isize into a `*const c_void` slot is valid;
    // both are pointer-width on every platform crosscom supports.
    buf.push((-(slot_index as isize)) as *const c_void);
    buf.push(proto_query_interface as *const c_void);
    buf.push(proto_add_ref as *const c_void);
    buf.push(proto_release as *const c_void);
    for &code in proto.blueprint.method_codes {
        buf.push(code);
    }
    let leaked: &'static [*const c_void] = Box::leak(buf.into_boxed_slice());
    // The slot stored in the CCW (and the vtable pointer reported to
    // consumers) is `&leaked[1]` — one slot past the `offset` header.
    let vtable_ptr = unsafe { leaked.as_ptr().add(1) };

    let mut map = proto.slot_vtables.lock().expect("slot_vtables poisoned");
    map.entry(slot_index).or_insert(SlotVtablePtr(vtable_ptr)).0
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
        RetKind::Float | RetKind::OptionalFloat => Type::f32(),
        RetKind::Bool => Type::c_int(),
        RetKind::OptionalForeign { .. } | RetKind::Foreign { .. } => Type::pointer(),
    };
    Cif::new(arg_types.into_iter(), ret_type)
}

// ---------------------------------------------------------------------------
// Fat CCW layout
// ---------------------------------------------------------------------------
//
// Each CCW is a manually-allocated buffer with a fixed-size header
// followed by `num_slots` inline vtable-pointer slots. The
// vtable-pointer at slot K is what consumers receive as the
// interface `this` pointer; the slot's *stored* pointer references
// a per-(uuid, slot_index) global allocation whose leading `offset
// = -K` field lets [`recover_ccw_base`] walk back to slot 0 from
// any slot pointer.
//
// ```text
//   offset 0:                       [ CcwHeader { ref_count, num_slots, payload } ]
//   offset sizeof(CcwHeader):       [ slot_0_vtable_ptr ]
//   offset sizeof(CcwHeader)+ptr:   [ slot_1_vtable_ptr ]
//   ...
// ```
//
// Layout invariant: `slot_0_addr - sizeof::<CcwHeader>() ==
// header_addr`. Both are produced by the same [`ccw_layout`] helper
// so dealloc never disagrees with alloc.
#[repr(C)]
struct CcwHeader {
    ref_count: AtomicU32,
    num_slots: usize,
    payload: ProtoCcwPayload,
}

struct ProtoCcwPayload {
    root_idx: usize,
    handle: RuntimeHandle,
    /// Per-slot interface metadata, in declaration order. `slots[k]`
    /// describes the interface backing slot K of the CCW.
    slots: Box<[SlotInfo]>,
}

struct SlotInfo {
    uuid: [u8; 16],
    additional_query_uuids: &'static [[u8; 16]],
}

/// `(layout, slot_offset_bytes)`. `slot_offset_bytes` is the byte
/// offset from the allocation start to slot 0; the header sits at
/// offset 0 and slots start right after it (with alignment padding
/// folded in by `Layout::extend`).
fn ccw_layout(num_slots: usize) -> (Layout, usize) {
    let header = Layout::new::<CcwHeader>();
    let slots = Layout::array::<*const c_void>(num_slots).expect("ccw slot array layout");
    let (combined, slot_offset) = header.extend(slots).expect("ccw extend");
    (combined.pad_to_align(), slot_offset)
}

unsafe fn ccw_slot_array_base(alloc_ptr: *const u8, slot_offset: usize) -> *const *const c_void {
    unsafe { alloc_ptr.add(slot_offset) as *const *const c_void }
}

unsafe fn ccw_slot_ptr(
    slot_array_base: *const *const c_void,
    slot_index: usize,
) -> *const *const c_void {
    unsafe { slot_array_base.add(slot_index) }
}

/// Walk back from any interface `this` pointer (a `*const *const c_void`
/// pointing at a slot in the CCW) to slot 0 — the start of the
/// slot array. Mirrors `crosscom::get_object`: the slot's vtable
/// pointer is preceded by an `isize` offset (in *const c_void slots)
/// that, applied to the interface pointer, lands on slot 0.
unsafe fn recover_slot0_addr(this: *const *const c_void) -> *const *const c_void {
    unsafe {
        let vtable_ptr = *(this as *const *const isize);
        let offset = *vtable_ptr.offset(-1);
        this.offset(offset)
    }
}

/// Recover the CcwHeader from any interface `this` pointer. The
/// slot array starts immediately after the header (per [`ccw_layout`]),
/// so the header sits at `slot0_addr - sizeof::<CcwHeader>()`.
unsafe fn recover_header<'a>(this: *const *const c_void) -> &'a CcwHeader {
    unsafe {
        let slot0 = recover_slot0_addr(this);
        let header_addr = (slot0 as *const u8).sub(size_of::<CcwHeader>()) as *const CcwHeader;
        &*header_addr
    }
}

unsafe fn recover_header_addr(this: *const *const c_void) -> *const CcwHeader {
    unsafe {
        let slot0 = recover_slot0_addr(this);
        (slot0 as *const u8).sub(size_of::<CcwHeader>()) as *const CcwHeader
    }
}

// ---------------------------------------------------------------------------
// IUnknown thunks (operate on any slot of the fat CCW)
// ---------------------------------------------------------------------------

// The old ProtoCcw / duplicate ProtoCcwPayload definitions are gone;
// the fat layout above is the single source of truth.

unsafe extern "system" fn proto_query_interface(
    this: *const *const c_void,
    guid: uuid::Uuid,
    retval: &mut *const *const c_void,
) -> c_long {
    unsafe {
        let header = recover_header(this);
        let slot0 = recover_slot0_addr(this);
        let bytes = *guid.as_bytes();

        if bytes == IUnknown::INTERFACE_ID {
            // IUnknown is conventionally satisfied via slot 0.
            *retval = slot0;
            proto_add_ref(slot0);
            return ResultCode::Ok as c_long;
        }

        // Walk slots in declaration order; first match wins. Each slot's
        // own UUID and its (vtable-layout-compatible) additional QI list
        // are eligible. The returned pointer is the slot pointer — i.e.
        // an interface pointer with the right vtable for the requested
        // interface, including the slot's offset prefix.
        for (i, slot) in header.payload.slots.iter().enumerate() {
            if slot.uuid == bytes || slot.additional_query_uuids.iter().any(|u| *u == bytes) {
                let slot_ptr = slot0.add(i);
                *retval = slot_ptr;
                proto_add_ref(slot_ptr);
                return ResultCode::Ok as c_long;
            }
        }

        *retval = std::ptr::null();
        ResultCode::ENoInterface as c_long
    }
}

unsafe extern "system" fn proto_add_ref(this: *const *const c_void) -> c_long {
    unsafe {
        let header = recover_header(this);
        let prev = header.ref_count.fetch_add(1, Ordering::SeqCst);
        (prev + 1) as c_long
    }
}

unsafe extern "system" fn proto_release(this: *const *const c_void) -> c_long {
    unsafe {
        let header_addr = recover_header_addr(this);
        let prev = (*header_addr).ref_count.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            // Drop the CCW: read out num_slots + payload, unroot the
            // script handle, drop the payload (handle + slots Box), and
            // dealloc the entire buffer with the same Layout used at
            // alloc time.
            let num_slots = (*header_addr).num_slots;
            let (layout, _slot_offset) = ccw_layout(num_slots);
            // Move out the payload (Drop-runs the RuntimeHandle clone +
            // slots Box). `read` is safe because no other reference to
            // the header survives past this point — ref_count just hit
            // zero.
            let payload = std::ptr::read(&(*header_addr).payload);
            let _ = payload
                .handle
                .try_with_ctx(|ctx| ctx.remove_external_root(payload.root_idx));
            drop(payload);
            dealloc(header_addr as *mut u8, layout);
        }
        (prev - 1) as c_long
    }
}

// ---------------------------------------------------------------------------
// wrap_proto_raw
// ---------------------------------------------------------------------------

fn wrap_proto_raw(
    handle: &RuntimeHandle,
    data: Data,
    requested_uuid: [u8; 16],
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

    // Resolve the slot list. For `ProtoBoxRef`, we consult the
    // script struct's `conforming_to` to enumerate every interface
    // the impl supports — that's what gives QI to sibling interfaces
    // its real (vtable-distinct) backing. For `BoxRef` (no
    // conformance info), the slot list is just the requested UUID.
    let mut slot_uuids = collect_slot_uuids(handle, &data);
    // Always ensure the requested UUID is satisfiable. If the
    // conformance list doesn't include it (e.g. the script struct
    // conforms to a DIFFERENT registered proto but the caller asked
    // for IUnknown's well-known UUID, or the IDL ret type is a
    // sub/super-interface accessible via additional_query_uuids),
    // probe via additional_query_uuids first; only if nothing
    // matches do we append the requested UUID as a fresh slot.
    let need_extra_slot = !slot_uuids.iter().any(|u| {
        if *u == requested_uuid {
            return true;
        }
        if let Ok(reg) = registry().lock() {
            if let Some(p) = reg.get(u) {
                return p.additional_query_uuids.contains(&requested_uuid);
            }
        }
        false
    });
    if need_extra_slot {
        // Prepend so the requested UUID is the "primary" slot. Drop
        // unregistered UUIDs from the rest of the list as we go;
        // they have no vtable to give us anyway.
        let mut combined = Vec::with_capacity(slot_uuids.len() + 1);
        combined.push(requested_uuid);
        combined.extend(slot_uuids.into_iter());
        slot_uuids = combined;
    }

    // Snapshot each slot's RegisteredProto pieces in a single lock
    // acquisition. Unregistered tags are dropped silently — they're
    // a script-side conformance declaration that the host hasn't
    // bridged yet, and exposing them as QI-success would hand out a
    // vtable that doesn't exist.
    struct SlotPlan {
        uuid: [u8; 16],
        vtable_ptr: *const *const c_void,
        additional_query_uuids: &'static [[u8; 16]],
    }
    let mut plans: Vec<SlotPlan> = Vec::with_capacity(slot_uuids.len());
    {
        let reg = registry().lock().expect("proto_ccw registry poisoned");
        for uuid in &slot_uuids {
            if plans.iter().any(|p| p.uuid == *uuid) {
                // De-dupe: the same interface can legitimately appear
                // in both the conformance list and as the requested
                // UUID. One slot is enough.
                continue;
            }
            let Some(proto) = reg.get(uuid) else {
                if *uuid == requested_uuid {
                    return Err(HostError::message(format!(
                        "wrap_proto: no ProtoSpec registered for interface UUID {:?}; \
                         call register_proto_ccw before wrapping",
                        uuid
                    )));
                }
                continue;
            };
            let slot_index = plans.len();
            let vtable_ptr = mint_slot_vtable(proto, slot_index);
            plans.push(SlotPlan {
                uuid: *uuid,
                vtable_ptr,
                additional_query_uuids: proto.additional_query_uuids,
            });
        }
    }

    // Pick the slot that satisfies the requested UUID. With
    // first-match-wins, this is the first slot whose own UUID equals
    // the request OR whose additional_query_uuids contains it. The
    // need_extra_slot logic above guarantees at least one such slot
    // exists.
    let selected_slot = plans
        .iter()
        .position(|p| {
            p.uuid == requested_uuid || p.additional_query_uuids.contains(&requested_uuid)
        })
        .ok_or_else(|| {
            HostError::message(format!(
                "wrap_proto: no slot satisfies requested UUID {:?} after planning",
                requested_uuid
            ))
        })?;

    // Allocate the CCW buffer with the fat layout.
    let num_slots = plans.len();
    let (layout, slot_offset) = ccw_layout(num_slots);
    // SAFETY: layout has non-zero size (CcwHeader is non-empty).
    let alloc_ptr = unsafe { alloc(layout) };
    if alloc_ptr.is_null() {
        std::alloc::handle_alloc_error(layout);
    }

    // Root the script data once for the whole CCW.
    let root_idx = handle
        .try_with_ctx(|ctx| ctx.add_external_root(data.clone()))
        .ok_or_else(|| {
            // SAFETY: we never published the alloc; dealloc with the
            // same layout is sound.
            unsafe { dealloc(alloc_ptr, layout) };
            HostError::message("wrap_proto: runtime gone before rooting")
        })?;

    // Build the per-slot metadata Box before writing into the
    // buffer; that way a panic during allocation surfaces as a
    // Rust panic rather than corrupted memory.
    let slots_box: Box<[SlotInfo]> = plans
        .iter()
        .map(|p| SlotInfo {
            uuid: p.uuid,
            additional_query_uuids: p.additional_query_uuids,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();

    // Initialise the header in-place.
    unsafe {
        let header_addr = alloc_ptr as *mut CcwHeader;
        std::ptr::write(
            header_addr,
            CcwHeader {
                ref_count: AtomicU32::new(1),
                num_slots,
                payload: ProtoCcwPayload {
                    root_idx,
                    handle: handle.clone(),
                    slots: slots_box,
                },
            },
        );

        // Write each slot's vtable pointer into the slot array.
        let slot_array_base = ccw_slot_array_base(alloc_ptr, slot_offset);
        for (i, plan) in plans.iter().enumerate() {
            std::ptr::write(
                slot_array_base.add(i) as *mut *const *const c_void,
                plan.vtable_ptr,
            );
        }

        Ok(ccw_slot_ptr(slot_array_base, selected_slot))
    }
}

/// Collect every foreign-tagged proto UUID that the script-side
/// struct backing `data` conforms to, in declaration order. Empty
/// for non-proto boxes (plain `BoxRef`) — those carry no
/// conformance information.
fn collect_slot_uuids(handle: &RuntimeHandle, data: &Data) -> Vec<[u8; 16]> {
    let (concrete_type_id, origin_module_idx) = match data {
        Data::ProtoBoxRef {
            concrete_type_id,
            origin_module_idx,
            ..
        } => (*concrete_type_id, *origin_module_idx),
        _ => return Vec::new(),
    };

    handle
        .try_with_ctx(|ctx| {
            ctx.struct_foreign_proto_tags(origin_module_idx as usize, concrete_type_id)
                .into_iter()
                .filter_map(|tag| {
                    ctx.foreign_uuid(tag)
                        .and_then(|s| uuid::Uuid::parse_str(s).ok())
                        .map(|u| *u.as_bytes())
                })
                .collect::<Vec<[u8; 16]>>()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Method dispatch: libffi closure callbacks
// ---------------------------------------------------------------------------

struct MethodUserdata {
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
    /// Returned interface where the script's `?box<F>` value was a
    /// foreign-carrier box (a `Data::Foreign{...}` wrapped in a
    /// `ProtoBoxRef`) — i.e. a box that wraps an actual Rust
    /// `ComRc<I>` rather than a script struct conforming to `F`.
    /// In this case we skip the recursive `wrap_proto` (which would
    /// build a CCW on top of a CCW and fail script-method dispatch)
    /// and pass the underlying COM pointer through directly.
    OptionalForeignRaw(*const *const c_void),
    /// `float?` return: `None` is sent over the C ABI as `NaN`.
    OptionalFloat(Option<f32>),
    Error(HostError),
}

unsafe extern "C" fn method_thunk_void(
    _cif: &libffi::low::ffi_cif,
    _result: &mut (),
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    unsafe {
        let _ = dispatch_method(args, userdata);
    }
}

unsafe extern "C" fn method_thunk_int(
    _cif: &libffi::low::ffi_cif,
    result: &mut c_int,
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    unsafe {
        match dispatch_method(args, userdata) {
            DispatchOutcome::Int(i) => *result = i as c_int,
            DispatchOutcome::Bool(b) => *result = b as c_int,
            DispatchOutcome::Error(_) | _ => *result = 0,
        }
    }
}

unsafe extern "C" fn method_thunk_float(
    _cif: &libffi::low::ffi_cif,
    result: &mut c_float,
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    unsafe {
        match dispatch_method(args, userdata) {
            DispatchOutcome::Float(f) => *result = f as c_float,
            DispatchOutcome::OptionalFloat(Some(v)) => *result = v,
            DispatchOutcome::OptionalFloat(None) => *result = f32::NAN,
            DispatchOutcome::Error(_) | _ => *result = 0.0,
        }
    }
}

unsafe extern "C" fn method_thunk_ptr(
    _cif: &libffi::low::ffi_cif,
    result: &mut *const c_void,
    args: *const *const c_void,
    userdata: &MethodUserdata,
) {
    unsafe {
        let outcome = dispatch_method(args, userdata);
        *result = std::ptr::null();
        match outcome {
            DispatchOutcome::OptionalForeign(inner) => {
                if let Some(data) = inner {
                    // Recursive wrap_proto_unknown for the returned box.
                    let uuid = match &userdata.ret {
                        RetKind::OptionalForeign { uuid, .. } => *uuid,
                        RetKind::Foreign { uuid, .. } => *uuid,
                        _ => return,
                    };
                    // Re-read the CCW's RuntimeHandle from args[0] so the
                    // recursive wrap uses the same runtime.
                    let this_slot = *args.add(0);
                    let this_pp = *(this_slot as *const *const *const c_void);
                    let header = recover_header(this_pp);
                    match wrap_proto_unknown(&header.payload.handle, data, uuid) {
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
            DispatchOutcome::OptionalForeignRaw(raw) => {
                // Script returned a foreign-carrier box that wraps a real
                // Rust ComObject. Pass its raw COM pointer through; the
                // engine sees the underlying ComObject directly.
                *result = raw as *const c_void;
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
}

unsafe fn dispatch_method(
    args: *const *const c_void,
    userdata: &MethodUserdata,
) -> DispatchOutcome {
    unsafe {
        // libffi passes `args` as an array of slot pointers. args.add(i)
        // points to the i-th element of that array, and *args.add(i) is
        // the slot pointer itself (a `*const c_void` pointing at the
        // arg's storage). Dereference once more with the right type to
        // read the actual value.
        let this_slot = *args.add(0);
        let this_pp = *(this_slot as *const *const *const c_void);
        let header = recover_header(this_pp);

        // Re-enter the runtime *first*: marshalling `Foreign` args uses
        // `with_services` which requires the host services scope to be
        // active. `RuntimeAccess::with_ctx` (implemented on `ScriptHost`)
        // installs `scope` + `scope_context` for the duration of the
        // closure, which is exactly what marshalling and the subsequent
        // `push_proto_method`/`resume` both need.
        let root_idx = header.payload.root_idx;
        let method_name = userdata.method_name.clone();
        let ret_kind = userdata.ret.clone();
        let arg_kinds = userdata.args.clone();
        let arg_slot_ptrs: Vec<*const c_void> =
            (0..userdata.args.len()).map(|i| *args.add(1 + i)).collect();

        let outcome = header
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
        RetKind::OptionalFloat => match frame.stack.pop() {
            Some(Data::Null) => DispatchOutcome::OptionalFloat(None),
            Some(Data::Some(inner)) => match &*inner {
                Data::Float(f) => DispatchOutcome::OptionalFloat(Some(*f as f32)),
                other => DispatchOutcome::Error(HostError::message(format!(
                    "{}: expected ?Float inner Float, got {:?}",
                    method_name, other
                ))),
            },
            Some(Data::Float(f)) => DispatchOutcome::OptionalFloat(Some(f as f32)),
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected OptionalFloat return, got {:?}",
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
        RetKind::OptionalForeign { uuid: ret_uuid, .. } => match frame.stack.pop() {
            Some(Data::Null) => DispatchOutcome::OptionalForeign(None),
            Some(Data::Some(inner)) => {
                // `inner: Rc<Data>`; clone the inner Data out (the
                // recursive wrap_proto will root it again).
                classify_foreign_return(ctx, (*inner).clone(), *ret_uuid)
            }
            Some(d @ Data::ProtoBoxRef { .. }) | Some(d @ Data::BoxRef { .. }) => {
                // Tolerate bare-box returns (the script's `return self` /
                // `return some_box` pattern); director.md A2 flags this
                // as an explicit p7 ergonomic gap.
                classify_foreign_return(ctx, d, *ret_uuid)
            }
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected OptionalForeign return, got {:?}",
                method_name, other
            ))),
        },
        RetKind::Foreign { uuid: ret_uuid, .. } => match frame.stack.pop() {
            // Non-optional: a null / absent return violates the contract.
            Some(Data::Null) => DispatchOutcome::Error(HostError::message(format!(
                "{}: non-optional foreign return was null",
                method_name
            ))),
            Some(Data::Some(inner)) => classify_foreign_return(ctx, (*inner).clone(), *ret_uuid),
            Some(d @ Data::ProtoBoxRef { .. }) | Some(d @ Data::BoxRef { .. }) => {
                classify_foreign_return(ctx, d, *ret_uuid)
            }
            other => DispatchOutcome::Error(HostError::message(format!(
                "{}: expected Foreign return, got {:?}",
                method_name, other
            ))),
        },
    }
}

/// Classify the inner value of an `box<F>` / `?box<F>` return (used by
/// both [`RetKind::Foreign`] and [`RetKind::OptionalForeign`] — the
/// optionality is resolved by the caller before this point):
///
/// * If the underlying box wraps a foreign carrier (i.e. a
///   `Data::Foreign{type_tag, handle, ...}` payload in the box heap),
///   look up the carrier's interned `ComObjectTable` entry and return
///   its raw COM pointer via `OptionalForeignRaw`. This is the path
///   taken when a script `update` returns a `box<radiance.IDirector>`
///   value that originated as a Rust `ComRc<IDirector>` handed to the
///   script via `host.foreign_box(...)` — re-wrapping it in another
///   CCW would create a CCW pointing at a foreign carrier whose
///   script-side struct has no method impls, causing method dispatch
///   to fail.
///
/// * Otherwise the box is a script-side struct conforming to the
///   target proto; return `OptionalForeign(Some(data))` so the libffi
///   thunk recursively builds a CCW around it.
fn classify_foreign_return(
    ctx: &mut Context,
    data: Data,
    ret_uuid: [u8; 16],
) -> DispatchOutcome {
    // Read the box payload from p7's box heap. Foreign carriers store
    // a `Data::Foreign` inside the box; script structs store a
    // struct heap reference. Only the former needs the pass-through.
    let (box_idx, generation) = match &data {
        Data::ProtoBoxRef {
            box_idx,
            generation,
            ..
        } => (*box_idx, *generation),
        Data::BoxRef { idx, generation } => (*idx, *generation),
        _ => return DispatchOutcome::OptionalForeign(Some(data)),
    };
    let payload = match ctx.box_heap.get(box_idx, generation) {
        Ok(p) => p.clone(),
        Err(_) => return DispatchOutcome::OptionalForeign(Some(data)),
    };
    if let Data::Foreign {
        handle: com_handle, ..
    } = payload
    {
        // Bypass wrap_proto: fetch the ComObjectTable entry's raw COM
        // pointer for the target interface UUID, add_ref it so the
        // caller's strong reference matches `wrap_proto`'s output
        // contract, and return.
        let raw = match with_services(|s| s.com_table_mut().get_raw_qi(com_handle, ret_uuid)) {
            Ok(Some(p)) => p,
            Ok(None) => {
                return DispatchOutcome::Error(HostError::message(format!(
                    "foreign-return: ComObject {} does not expose interface {:?}",
                    com_handle, ret_uuid
                )));
            }
            Err(e) => return DispatchOutcome::Error(e),
        };
        return DispatchOutcome::OptionalForeignRaw(raw);
    }
    DispatchOutcome::OptionalForeign(Some(data))
}

unsafe fn marshal_arg_in(
    ctx: &mut Context,
    kind: &ArgKind,
    slot_ptr: *const c_void,
) -> Result<Data, HostError> {
    unsafe {
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
                let hr =
                    ((*unk_vtbl).query_interface)(com_ptr as *const c_void, guid, &mut raw_unk);
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
                    HostError::message(format!("alloc_foreign('{}') failed: {:?}", type_tag, e))
                })
            }
        }
    }
}

// Keep the `c_long` import used by the IUnknown thunks alive even if
// the `lints unused-import` heuristic gets confused.
#[allow(dead_code)]
fn _import_anchor() {
    let _ = std::mem::size_of::<c_long>();
}
