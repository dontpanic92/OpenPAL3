//! Host-side runtime support for crosscom IDL → protosept binding.
//!
//! This crate is the seam between the *generated* host-fn shims (produced by
//! `crosscom-ccidl --host-shims`) and the *protosept interpreter* (the `p7`
//! crate, which lives in its own cargo workspace).
//!
//! Decoupling the two is intentional: the generated shims only see the
//! [`HostContext`] trait declared here. A consumer that wants to plug in a
//! protosept runtime implements the trait against `p7::Context` in its own
//! crate. This avoids forcing the OpenPAL3 main workspace to depend directly
//! on protosept, while still letting the IDL/code-generation pipeline
//! validate end-to-end at compile time.
//!
//! # Components
//!
//! - [`ComObjectTable`] — a generation-checked slotmap of
//!   `ComRc<dyn IUnknown>`. Scripts hold `i64` ids; this is the only path
//!   from a script-side id back to a live COM handle.
//! - [`HostContext`] — the trait the generated shims call (push/pop typed
//!   values, register host fns, fetch services).
//! - [`HostError`] — error type returned by shims.
//! - [`scope`] / [`with_services`] — thread-local services scope helper.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

pub use crosscom;
use crosscom::{ComInterface, ComRc, IUnknown};

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
            (*p)
                .query_interface::<IUnknown>()
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
            return ComObjectId::encode(idx, s.generation);
        }
        let idx = self.slots.len();
        self.slots.push(Slot {
            rc: Some(rc),
            generation: 0,
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

    /// Drop the strong ref backing `id`, freeing the slot and bumping its
    /// generation so subsequent lookups with the old id fail. Returns
    /// `false` if the id was already invalid.
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
    pub fn get_raw_qi(&self, id: i64, uuid_bytes: [u8; 16]) -> Option<*const *const std::ffi::c_void> {
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
}

// ---------------------------------------------------------------------------
// Thread-local services scope
// ---------------------------------------------------------------------------

thread_local! {
    static CURRENT_SERVICES: RefCell<Option<*mut dyn HostServices>> = const { RefCell::new(None) };
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
pub fn with_services<R>(
    body: impl FnOnce(&mut dyn HostServices) -> R,
) -> Result<R, HostError> {
    CURRENT_SERVICES.with(|c| {
        let ptr = c.borrow().ok_or_else(|| {
            HostError::message("with_services called outside crosscom_p7host::scope")
        })?;
        // Safety: `scope` guarantees the pointer is valid for the duration
        // of `body`. Re-entry into `scope` from `body` is documented as
        // disallowed.
        let services = unsafe { &mut *ptr };
        Ok(body(services))
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
