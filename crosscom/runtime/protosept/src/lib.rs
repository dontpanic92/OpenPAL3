//! Protosept runtime support for crosscom.
//!
//! This crate is the seam between the *generated* protosept (`.p7`) bindings
//! produced by `crosscom-ccidl --protosept` and the *protosept interpreter*
//! (the `p7` crate, which lives in `radiance/protosept/`).
//!
//! It provides three pieces, all in one crate:
//!
//! - The COM-object handle table ([`ComObjectTable`]) and the encoded
//!   script-side id type ([`ComObjectId`]).
//! - The [`HostContext`] / [`HostServices`] traits plus a thread-local
//!   [`scope`] / [`with_services`] helper so generated host-fn shims can
//!   reach the active services bundle without taking on a static dep on the
//!   protosept workspace.
//! - The generic, AST-free `@foreign` proto dispatcher
//!   ([`install_com_dispatcher`]) that wires `com.invoke` and `com.release`
//!   onto a freshly-created [`p7::interpreter::context::Context`]. Adding a
//!   new crosscom IDL only requires running
//!   `crosscom-ccidl --protosept` to produce its `.p7` source — no per-IDL
//!   Rust code is generated.
//! - A default adapter ([`P7HostContext`]) that implements [`HostContext`]
//!   for [`p7::interpreter::context::Context`].
//!
//! Use [`install_com_dispatcher`] after `Context::new()` and before loading
//! any modules whose `@foreign` protos use the dispatcher names.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub use crosscom;
use crosscom::{ComInterface, ComRc, IUnknown};

pub mod adapter;
pub mod dispatcher;
pub mod script_proxy;

pub use adapter::{MinimalServices, P7HostContext};
pub use dispatcher::install_com_dispatcher;
pub use script_proxy::{wrap_action, ScriptActionProxy};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error type returned by host-fn shims. Generated code constructs these via
/// [`HostError::message`]; runtime callers see them as the `Err` of a
/// [`HostContext`] operation.
#[derive(Debug, Clone)]
pub struct HostError {
    pub message: String,
}

impl HostError {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HostError {}

// ---------------------------------------------------------------------------
// ComObjectTable
// ---------------------------------------------------------------------------

/// Generation-checked slotmap of live `ComRc<IUnknown>` handles.
///
/// Scripts only ever see the encoded `i64` id ([`ComObjectId::encode`]).
/// The encoding bakes the slot index and a per-slot generation counter
/// into the value so that stale ids surface cleanly at lookup time even
/// after the slot is reused.
pub struct ComObjectTable {
    slots: Vec<Slot>,
    free: Vec<usize>,
}

struct Slot {
    rc: Option<ComRc<IUnknown>>,
    generation: u32,
    /// Outstanding strong-handle count. `intern` / `intern_unknown`
    /// initialise this to 1 (the host's own handle). Each call to
    /// [`ComObjectTable::add_ref`] bumps it; each [`ComObjectTable::release`]
    /// decrements it. When it reaches 0 the underlying `ComRc` is
    /// dropped, the slot enters the free list, and its generation is
    /// bumped so stale ids fail-fast. This makes the
    /// `host.foreign_box(id) → script-side box → GC finalizer
    /// (com.release)` round-trip safe even when multiple foreign
    /// boxes share the same intern id (e.g. a per-frame `box<IUiHost>`
    /// that drops after the frame while the next frame allocates a
    /// fresh one pointing at the same singleton ComObject).
    refs: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ComObjectId(u64);

impl ComObjectId {
    /// Encode `(slot, generation)` into an i64 the script can hold. Bit 63
    /// is always zero so the value fits inside protosept's `int`.
    pub fn encode(slot: usize, generation: u32) -> i64 {
        let s = (slot as u64) & 0xffff_ffff;
        let g = (generation as u64) & 0x7fff_ffff;
        ((g << 32) | s) as i64
    }

    pub fn decode(raw: i64) -> Option<(usize, u32)> {
        if raw < 0 {
            return None;
        }
        let raw = raw as u64;
        let slot = (raw & 0xffff_ffff) as usize;
        let generation = ((raw >> 32) & 0x7fff_ffff) as u32;
        Some((slot, generation))
    }
}

impl Default for ComObjectTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ComObjectTable {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Intern a `ComRc<I>` and return the encoded id.
    pub fn intern<I: ComInterface + 'static>(&mut self, rc: ComRc<I>) -> i64 {
        // Up-cast to ComRc<IUnknown>. Every interface vtable in crosscom
        // starts with the three IUnknown slots (query_interface/add_ref/
        // release), so reinterpreting the pointer as `*const IUnknown` is
        // safe; the IUnknown::query_interface call adds a strong ref, and
        // we drop the original `rc` to keep the net count at one.
        let unk: ComRc<IUnknown> = unsafe {
            let p = rc.ptr_value() as *const IUnknown;
            (*p).query_interface::<IUnknown>()
                .expect("every ComInterface must expose IUnknown")
        };
        drop(rc);
        self.intern_unknown(unk)
    }

    /// Intern an already up-cast `ComRc<IUnknown>`.
    pub fn intern_unknown(&mut self, rc: ComRc<IUnknown>) -> i64 {
        if let Some(idx) = self.free.pop() {
            let s = &mut self.slots[idx];
            s.rc = Some(rc);
            s.refs = 1;
            return ComObjectId::encode(idx, s.generation);
        }
        let idx = self.slots.len();
        self.slots.push(Slot {
            rc: Some(rc),
            generation: 0,
            refs: 1,
        });
        ComObjectId::encode(idx, 0)
    }

    /// Look up `id` and downcast to interface `I` via `query_interface`.
    /// Returns `None` if the id is invalid, the slot is empty, the
    /// generation does not match, or the held object does not expose `I`.
    pub fn get<I: ComInterface + 'static>(&self, id: i64) -> Option<ComRc<I>> {
        let (slot, generation) = ComObjectId::decode(id)?;
        let s = self.slots.get(slot)?;
        if s.generation != generation {
            return None;
        }
        let rc = s.rc.as_ref()?;
        // SAFETY: ComRc<IUnknown>::ptr_value yields the canonical pointer
        // shape, which all crosscom interfaces share (IUnknown layout
        // prefix). The QI call adds a fresh strong ref; the original is
        // retained inside the table, so net counts stay correct.
        unsafe {
            let p = rc.ptr_value() as *const IUnknown;
            (*p).query_interface::<I>()
        }
    }

    /// Increment the strong-handle count on `id`. Returns `false` if
    /// the id is invalid, the slot is empty, or the generation does
    /// not match. Each successful `add_ref` must be balanced by a
    /// later [`ComObjectTable::release`] — typically by handing the
    /// same id to a script-side foreign box whose GC finalizer calls
    /// `com.release`.
    pub fn add_ref(&mut self, id: i64) -> bool {
        let Some((slot, generation)) = ComObjectId::decode(id) else {
            return false;
        };
        let Some(s) = self.slots.get_mut(slot) else {
            return false;
        };
        if s.generation != generation || s.rc.is_none() {
            return false;
        }
        s.refs = s.refs.saturating_add(1);
        true
    }

    /// Decrement the strong-handle count on `id`. When it reaches zero
    /// the underlying `ComRc` is dropped, the slot is freed, and its
    /// generation is bumped so subsequent lookups with the old id
    /// fail. Returns `false` if the id was already invalid.
    pub fn release(&mut self, id: i64) -> bool {
        let Some((slot, generation)) = ComObjectId::decode(id) else {
            return false;
        };
        let Some(s) = self.slots.get_mut(slot) else {
            return false;
        };
        if s.generation != generation || s.rc.is_none() {
            return false;
        }
        if s.refs > 1 {
            s.refs -= 1;
            return true;
        }
        s.refs = 0;
        s.rc = None;
        s.generation = s.generation.wrapping_add(1);
        self.free.push(slot);
        true
    }

    /// Number of currently-occupied slots (for diagnostics).
    pub fn live(&self) -> usize {
        self.slots.iter().filter(|s| s.rc.is_some()).count()
    }
    /// Look up `id`, then `query_interface` to the runtime-specified UUID.
    /// Returns the raw COM pointer (`this`) that the host dispatcher can
    /// use to read the vtable. `None` when the id is invalid or the held
    /// object does not expose the requested interface.
    ///
    /// Unlike [`Self::get`], this does not require a static `ComInterface`
    /// type parameter — the UUID comes from p7 metadata at runtime. The
    /// caller owns the strong ref returned (the COM `query_interface`
    /// adds a ref via the underlying vtable); callers performing only a
    /// single virtual call should drop the strong ref afterwards via
    /// `unsafe { ComRc::<IUnknown>::from_raw_pointer(p) }` so the count
    /// stays balanced. The dispatcher does this around every call.
    pub fn get_raw_qi(
        &self,
        id: i64,
        uuid_bytes: [u8; 16],
    ) -> Option<*const *const std::ffi::c_void> {
        let (slot, generation) = ComObjectId::decode(id)?;
        let s = self.slots.get(slot)?;
        if s.generation != generation {
            return None;
        }
        let rc = s.rc.as_ref()?;
        unsafe {
            let p = rc.ptr_value() as *const IUnknown;
            // Use the IUnknown vtable's query_interface directly so the
            // call site stays generic over the target interface UUID.
            let this = p as *const std::ffi::c_void;
            let mut raw: *const *const std::ffi::c_void = std::ptr::null();
            let guid = uuid::Uuid::from_bytes(uuid_bytes);
            let vtbl = *(this as *const *const crosscom::IUnknownVirtualTable);
            let hr = ((*vtbl).query_interface)(this, guid, &mut raw);
            if hr != 0 || raw.is_null() {
                None
            } else {
                Some(raw)
            }
        }
    }
}

/// The seam between generated host-fn shims and a protosept runtime.
///
/// A consumer with access to the `p7` crate implements this trait against
/// `p7::Context`. The IDL-generated `*_host.rs` files only depend on this
/// trait, never on `p7::*` directly, which keeps the OpenPAL3 main cargo
/// workspace independent of the protosept workspace.
pub trait HostContext {
    type Services: HostServices;

    fn pop_int(&mut self) -> Result<i64, HostError>;
    fn pop_float(&mut self) -> Result<f64, HostError>;
    fn pop_string(&mut self) -> Result<String, HostError>;
    fn pop_optional_int(&mut self) -> Result<Option<i64>, HostError>;
    fn pop_int_array(&mut self) -> Result<Vec<i64>, HostError>;

    fn push_int(&mut self, value: i64);
    fn push_float(&mut self, value: f64);
    fn push_string(&mut self, value: String);
    fn push_optional_int(&mut self, value: Option<i64>);
    fn push_int_array(&mut self, value: Vec<i64>);

    /// Register `func` under `name` so script-side `@intrinsic(name="...")`
    /// declarations resolve to it. Names use the canonical
    /// `<idl-rust-module>.<Interface>.<method>` form.
    fn register_host_function(
        &mut self,
        name: &str,
        func: fn(&mut Self) -> Result<(), HostError>,
    ) -> Result<(), HostError>;

    fn services_mut(&mut self) -> &mut Self::Services;
}

/// Supplied by the consumer; bundles every Rust-side runtime resource that
/// generated shims need to reach. The minimum requirement is access to a
/// [`ComObjectTable`].
pub trait HostServices: Any {
    fn com_table_mut(&mut self) -> &mut ComObjectTable;

    /// Weak handle to the runtime that owns this services bundle. Reverse-
    /// wrap CCWs (see [`crate::script_proxy::wrap_action`]) capture this
    /// when constructed so their thunks and `Drop` can re-enter the
    /// runtime without relying on a thread-local
    /// [`scope_context`](crate::scope_context). The default returns a
    /// permanently-dangling handle; consumers that hand out long-lived
    /// reverse-wrapped `ComRc`s must override it.
    fn runtime_handle(&self) -> RuntimeHandle {
        RuntimeHandle::dangling()
    }
}

// ---------------------------------------------------------------------------
// RuntimeAccess + RuntimeHandle
// ---------------------------------------------------------------------------

/// Object-safe, re-entrant, interior-mut access to the protosept
/// interpreter [`Context`](p7::interpreter::context::Context) owned by
/// some runtime. Implementors live behind an [`Rc<dyn RuntimeAccess>`];
/// reverse-wrap CCWs carry a [`Weak`] to one so they neither pin the
/// runtime alive nor risk dangling pointers.
///
/// Implementations are responsible for installing whatever re-entrancy
/// guard / scope they need around `body` — typically
/// [`scope_context(ctx, || body(ctx))`](crate::scope_context) — so that
/// existing [`with_context`](crate::with_context) consumers
/// transparently see the same context.
pub trait RuntimeAccess: Any {
    fn with_ctx(&self, body: &mut dyn FnMut(&mut p7::interpreter::context::Context));
}

/// Weak, cheaply-cloneable handle to a [`RuntimeAccess`]. Reverse-wrap
/// CCWs store one of these in their payload so the methods they
/// dispatch (and their `Drop`) can re-enter the owning runtime even
/// outside the dynamic extent of any [`scope_context`](crate::scope_context).
#[derive(Clone)]
pub struct RuntimeHandle {
    weak: Weak<dyn RuntimeAccess>,
}

impl RuntimeHandle {
    /// Build a handle from an `Rc` to a concrete `RuntimeAccess`
    /// implementor. The underlying `Rc` is *not* retained; only a
    /// [`Weak`] coercion is stored.
    pub fn from_rc<T: RuntimeAccess + 'static>(rc: &Rc<T>) -> Self {
        let weak: Weak<T> = Rc::downgrade(rc);
        Self {
            weak: weak as Weak<dyn RuntimeAccess>,
        }
    }

    /// A handle whose upgrade always fails. Useful as a default for
    /// [`HostServices::runtime_handle`] impls that have no runtime to
    /// point at, and as a sentinel that [`wrap_action`](crate::wrap_action)
    /// can detect to fail loudly.
    pub fn dangling() -> Self {
        Self {
            weak: Weak::<DanglingRuntime>::new() as Weak<dyn RuntimeAccess>,
        }
    }

    /// True if the underlying runtime is already gone.
    pub fn is_dangling(&self) -> bool {
        self.weak.upgrade().is_none()
    }

    /// Attempt to re-enter the runtime. Returns `None` if the
    /// underlying `Rc` is gone.
    pub fn try_with_ctx<R>(
        &self,
        mut body: impl FnMut(&mut p7::interpreter::context::Context) -> R,
    ) -> Option<R> {
        let rc = self.weak.upgrade()?;
        let mut out: Option<R> = None;
        rc.with_ctx(&mut |ctx| {
            out = Some(body(ctx));
        });
        out
    }
}

impl std::fmt::Debug for RuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RuntimeHandle {{ dangling: {} }}",
            self.weak.upgrade().is_none()
        )
    }
}

/// Carrier type so [`RuntimeHandle::dangling`] can construct a
/// `Weak<dyn RuntimeAccess>` without needing an actual runtime instance.
/// The `with_ctx` impl is unreachable — upgrade always fails.
struct DanglingRuntime;

impl RuntimeAccess for DanglingRuntime {
    fn with_ctx(&self, _body: &mut dyn FnMut(&mut p7::interpreter::context::Context)) {
        // Unreachable: `RuntimeHandle::dangling` builds the Weak via
        // `Weak::<DanglingRuntime>::new()`, which never has a live
        // strong reference. If someone constructs a real
        // `Rc<DanglingRuntime>` and downgrades it, calling `with_ctx`
        // is a programming error.
        debug_assert!(
            false,
            "DanglingRuntime::with_ctx invoked; \
             RuntimeHandle::dangling should never resolve to a live runtime"
        );
    }
}

// ---------------------------------------------------------------------------
// Thread-local services scope
// ---------------------------------------------------------------------------

thread_local! {
    static CURRENT_SERVICES: RefCell<Option<*mut dyn HostServices>> = const { RefCell::new(None) };
    static CURRENT_CONTEXT: RefCell<Option<*mut p7::interpreter::context::Context>> = const { RefCell::new(None) };
}

/// Run `body` with `services` available via [`with_services`]. The pointer
/// is cleared before this function returns, so dangling access is impossible
/// outside the dynamic extent of `scope`.
pub fn scope<S: HostServices, R>(services: &mut S, body: impl FnOnce() -> R) -> R {
    let prev = CURRENT_SERVICES.with(|c| {
        let mut c = c.borrow_mut();
        let prev = *c;
        *c = Some(services as *mut S as *mut dyn HostServices);
        prev
    });
    struct Guard {
        prev: Option<*mut dyn HostServices>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            CURRENT_SERVICES.with(|c| {
                *c.borrow_mut() = self.prev;
            });
        }
    }
    let _guard = Guard { prev };
    body()
}

/// Access the services installed by [`scope`]. Returns `Err` if called
/// outside any `scope`. The closure must not call back into `scope`.
pub fn with_services<R>(body: impl FnOnce(&mut dyn HostServices) -> R) -> Result<R, HostError> {
    CURRENT_SERVICES.with(|c| {
        let ptr = c.borrow().ok_or_else(|| {
            HostError::message("with_services called outside crosscom_protosept::scope")
        })?;
        // Safety: `scope` guarantees the pointer is valid for the duration
        // of `body`. Re-entry into `scope` from `body` is documented as
        // disallowed.
        let services = unsafe { &mut *ptr };
        Ok(body(services))
    })
}

/// Run `body` with `ctx` accessible via [`with_context`]. Mirrors
/// [`scope`] for the interpreter [`Context`]. Used by host methods that
/// receive a script-implemented `@foreign` proto handle: the receiving
/// site installs the context scope so that, when the
/// [`ComRc`](crosscom::ComRc) wrapper around the script handle has its
/// vtable invoked (e.g. `IAction.invoke()`), the thunk can find the
/// owning interpreter to push a frame onto and resume.
///
/// The pointer is cleared before this function returns; access outside
/// the dynamic extent is impossible.
pub fn scope_context<R>(
    ctx: &mut p7::interpreter::context::Context,
    body: impl FnOnce() -> R,
) -> R {
    let prev = CURRENT_CONTEXT.with(|c| {
        let mut c = c.borrow_mut();
        let prev = *c;
        *c = Some(ctx as *mut _);
        prev
    });
    struct Guard {
        prev: Option<*mut p7::interpreter::context::Context>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            CURRENT_CONTEXT.with(|c| {
                *c.borrow_mut() = self.prev;
            });
        }
    }
    let _guard = Guard { prev };
    body()
}

/// Access the [`Context`] installed by [`scope_context`]. Returns `Err`
/// when called outside any `scope_context`. The closure must not call
/// back into `scope_context`.
pub fn with_context<R>(
    body: impl FnOnce(&mut p7::interpreter::context::Context) -> R,
) -> Result<R, HostError> {
    CURRENT_CONTEXT.with(|c| {
        let ptr = c.borrow().ok_or_else(|| {
            HostError::message("with_context called outside crosscom_protosept::scope_context")
        })?;
        let ctx = unsafe { &mut *ptr };
        Ok(body(ctx))
    })
}

// ---------------------------------------------------------------------------
// UUID lookup table
// ---------------------------------------------------------------------------

/// Maps the (lowercase, hyphenated) interface UUID string to a stable tag
/// the host can use to dispatch `com.query_interface` from script. Bound at
/// startup by the consumer.
pub fn com_uuid_table() -> &'static std::sync::Mutex<HashMap<String, &'static str>> {
    use std::sync::{Mutex, OnceLock};
    static TABLE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_encode_decode_roundtrip() {
        for slot in [0usize, 1, 17, 255, 65535, 70_000] {
            for gen in [0u32, 1, 7, 1_000_000, 0x7fff_ffff] {
                let id = ComObjectId::encode(slot, gen);
                let (s, g) = ComObjectId::decode(id).unwrap();
                assert_eq!(s, slot);
                assert_eq!(g, gen);
                assert!(id >= 0, "encoded id should be non-negative");
            }
        }
    }

    #[test]
    fn decode_rejects_negative() {
        assert!(ComObjectId::decode(-1).is_none());
        assert!(ComObjectId::decode(i64::MIN).is_none());
    }

    #[test]
    fn release_invalid_id_is_noop() {
        let mut t = ComObjectTable::new();
        assert!(!t.release(ComObjectId::encode(0, 0)));
        assert!(!t.release(-1));
        assert_eq!(t.live(), 0);
    }
}
