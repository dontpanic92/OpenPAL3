use std::{ffi::c_void, marker::PhantomData, ops::Deref, os::raw::c_long};

use uuid::Uuid;

pub use memoffset::offset_of;

pub type Void = ();
pub type StaticStr = &'static str;

pub struct ComRc<TComInterface: ComInterface> {
    this: *const TComInterface,
    /// Make `ComRc` explicitly `!Send` and `!Sync`.
    ///
    /// Concrete `ComObject` CCWs almost universally use `RefCell` /
    /// `Cell` for interior mutability, which is not `Sync`. Since
    /// `ComRc` derefs to the interface and lets callers invoke
    /// methods, allowing a `ComRc` to cross threads would let another
    /// thread reach that non-`Sync` state via the vtable. The atomic
    /// `add_ref` / `release` refcount is necessary but not sufficient
    /// for thread safety, so the type opts out at the marker level.
    /// If a genuinely thread-safe interface is ever introduced, it
    /// should opt back in via a dedicated marker trait rather than a
    /// blanket impl on `ComRc<T>`.
    _not_send_sync: PhantomData<*const ()>,
}

impl<TComInterface: ComInterface> ComRc<TComInterface> {
    pub fn from_object<TComObject: ComObject>(obj: TComObject) -> ComRc<TComInterface> {
        let p = Box::new(TComObject::create_ccw(obj));
        let raw = Box::into_raw(p);
        Self::query_interface_raw(raw as *const c_void)
    }

    pub fn uuid(&self) -> Uuid {
        Uuid::from_bytes(TComInterface::INTERFACE_ID)
    }

    pub fn from_self<TComObject: ComObject>(obj: &TComObject) -> ComRc<TComInterface> {
        let raw = TComObject::get_ccw(obj) as *const _ as *const c_void;
        Self::query_interface_raw(raw as *const c_void)
    }

    pub unsafe fn from_raw_pointer(raw: *const *const c_void) -> ComRc<TComInterface> {
        raw.into()
    }

    pub unsafe fn into_raw(self) -> *const *const c_void {
        self.into()
    }

    pub fn ptr_value(&self) -> *const c_void {
        self.this as *const c_void
    }

    pub fn is_null(&self) -> bool {
        self.this.is_null()
    }

    fn query_interface_raw(raw: *const c_void) -> ComRc<TComInterface> {
        unsafe {
            (raw as *const IUnknown)
                .as_ref()
                .unwrap()
                .query_interface::<TComInterface>()
                .expect("Failed to create ComRc: Interface not found")
        }
    }

    /// Project the CCW back to the concrete inner Rust value `C`.
    ///
    /// The returned `&C` borrow is tied to `&self` (the `ComRc` borrow),
    /// so it cannot outlive the `ComRc`: the borrow checker enforces
    /// what the legacy `unsafe { &*(self as *const _) }` cast laundered
    /// away. While `&self` is borrowed, the strong refcount stays ≥ 1,
    /// so the heap CCW remains allocated; the inner field address is
    /// stable inside a `#[repr(C)]` CCW; and no engine code ever
    /// exposes `&mut Inner` (interior mutability uses `RefCell`/`Cell`
    /// fields inside `C`).
    ///
    /// SAFETY-CONSCIOUS CONVENTION: the caller asserts `C` is the
    /// concrete class that actually backs this `ComRc`. Calling
    /// `inner::<WrongClass>()` reinterprets unrelated CCW memory as
    /// `WrongClass::CcwType` and is UB — same precondition as the
    /// legacy `'static` cast.
    pub fn inner<C: ComObject>(&self) -> &C {
        unsafe {
            let ccw = crate::get_object::<C::CcwType>(self.this as *const *const c_void);
            C::ccw_inner(&*ccw)
        }
    }
}

#[repr(transparent)]
pub struct RawPointer(pub *const *const c_void);

impl<TComInterface: ComInterface> From<*const *const c_void> for ComRc<TComInterface> {
    fn from(raw: *const *const c_void) -> Self {
        Self {
            this: raw as *const TComInterface,
            _not_send_sync: PhantomData,
        }
    }
}

impl<TComInterface: ComInterface> From<RawPointer> for Option<ComRc<TComInterface>> {
    fn from(raw: RawPointer) -> Self {
        if raw.0 == std::ptr::null() {
            None
        } else {
            Some(ComRc::<TComInterface> {
                this: raw.0 as *const TComInterface,
                _not_send_sync: PhantomData,
            })
        }
    }
}

impl<TComInterface: ComInterface> From<ComRc<TComInterface>> for *const *const c_void {
    fn from(rc: ComRc<TComInterface>) -> Self {
        let ret = rc.this as *const *const c_void;
        std::mem::forget(rc);
        ret
    }
}

impl<TComInterface: ComInterface> From<Option<ComRc<TComInterface>>> for RawPointer {
    fn from(rc: Option<ComRc<TComInterface>>) -> Self {
        if rc.is_none() {
            RawPointer {
                0: std::ptr::null(),
            }
        } else {
            let ret = rc.as_ref().unwrap().this as *const *const c_void;
            std::mem::forget(rc);
            RawPointer { 0: ret }
        }
    }
}

impl<TComInterface: ComInterface> Deref for ComRc<TComInterface> {
    type Target = TComInterface;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.this) }
    }
}

impl<TComInterface: ComInterface> Clone for ComRc<TComInterface> {
    fn clone(&self) -> Self {
        unsafe {
            (*(self.this as *const IUnknown)).add_ref();
        }

        Self {
            this: self.this.clone(),
            _not_send_sync: PhantomData,
        }
    }
}

impl<TComInterface: ComInterface> Drop for ComRc<TComInterface> {
    fn drop(&mut self) {
        unsafe {
            (*(self.this as *const IUnknown)).release();
        }
    }
}

pub trait ComInterface {
    const INTERFACE_ID: [u8; 16];
}

pub trait ComObject: Sized {
    type CcwType;
    fn create_ccw(self) -> Self::CcwType;
    fn get_ccw(&self) -> &Self::CcwType;

    /// Project a borrowed CCW back to its inner Rust value.
    ///
    /// SAFETY: the caller asserts the referenced CCW is well-formed
    /// and belongs to this class. The returned reference's lifetime
    /// is tied to the CCW borrow, so callers reached via
    /// [`ComRc::with_inner`] are sound by construction.
    fn ccw_inner(ccw: &Self::CcwType) -> &Self;
}

#[repr(C)]
pub struct IUnknownVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const c_void,
        guid: Uuid,
        retval: &mut *const *const c_void,
    ) -> c_long,
    pub add_ref: unsafe extern "system" fn(this: *const *const c_void) -> c_long,
    pub release: unsafe extern "system" fn(this: *const *const c_void) -> c_long,
}

#[repr(C)]
pub struct IUnknownVirtualTableCcw {
    pub offset: *const c_void,
    pub vtable: IUnknownVirtualTable,
}

pub trait IUnknownImpl {
    fn query_interface(&self, guid: Uuid, retval: &mut *const *const c_void) -> c_long;
    fn add_ref(&self) -> c_long;
    fn release(&self) -> c_long;
}

#[repr(C)]
pub struct IUnknown {
    vtable: *const IUnknownVirtualTable,
}

impl ComInterface for IUnknown {
    // 00000000-0000-0000-C000-000000000046
    const INTERFACE_ID: [u8; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x46,
    ];
}

impl IUnknown {
    pub fn query_interface<T: ComInterface>(&self) -> Option<ComRc<T>> {
        let this = self as *const IUnknown as *const std::os::raw::c_void;
        let mut raw = 0 as *const *const std::os::raw::c_void;
        let guid = Uuid::from_bytes(T::INTERFACE_ID);
        let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
        if ret_val != 0 {
            None
        } else {
            Some(unsafe { ComRc::<T>::from_raw_pointer(raw) })
        }
    }

    fn add_ref(&self) -> c_long {
        unsafe { ((*self.vtable).add_ref)(self as *const IUnknown as *const *const c_void) }
    }

    fn release(&self) -> c_long {
        unsafe { ((*self.vtable).release)(self as *const IUnknown as *const *const c_void) }
    }
}

pub unsafe fn get_object<T>(this: *const *const c_void) -> *const T {
    let vtable = *(this as *const *const isize);
    let vtable_ccw = vtable.offset(-1);
    let offset = *vtable_ccw;
    this.offset(offset) as *const T
}

pub type HResult = c_long;
pub type ComResult<T> = Result<T, HResult>;

pub enum ResultCode {
    Ok = 0,
    ENoInterface = -2147467262,
}

include!("defs.rs");

pub struct ObjectArrayImpl {
    buf: Vec<ComRc<IUnknown>>,
}

ComObject_ObjectArray!(crate::ObjectArrayImpl);

impl IObjectArrayImpl for ObjectArrayImpl {
    fn len(&self) -> i32 {
        self.buf.len() as i32
    }

    fn get(&self, index: i32) -> crate::ComRc<IUnknown> {
        self.buf[index as usize].clone()
    }
}

pub struct ObjectArray<TComInterface: ComInterface> {
    array: ComRc<IObjectArray>,
    _pd: PhantomData<TComInterface>,
}

impl<TComInterface: ComInterface> ObjectArray<TComInterface> {
    pub fn new(buf: Vec<ComRc<IUnknown>>) -> Self {
        Self {
            array: ComRc::<IObjectArray>::from_object(ObjectArrayImpl { buf }),
            _pd: PhantomData {},
        }
    }

    pub fn len(&self) -> i32 {
        self.array.len()
    }

    pub fn get(&self, index: i32) -> crate::ComRc<TComInterface> {
        self.array
            .get(index)
            .query_interface::<TComInterface>()
            .unwrap()
    }

    pub fn raw(&self) -> &Vec<ComRc<IUnknown>> {
        unsafe {
            let p = crate::get_object::<crate::ObjectArray_crosscom_impl::ObjectArrayCcw>(
                self.array.this as *const *const c_void,
            );
            &(*p).inner.buf
        }
    }
}

impl<TComInterface: ComInterface> Clone for ObjectArray<TComInterface> {
    fn clone(&self) -> Self {
        Self {
            array: self.array.clone(),
            _pd: self._pd.clone(),
        }
    }
}

impl<TComInterface: ComInterface> From<*const *const c_void> for ObjectArray<TComInterface> {
    fn from(raw: *const *const c_void) -> Self {
        Self {
            array: ComRc::<IObjectArray>::from(raw),
            _pd: PhantomData {},
        }
    }
}

impl<TComInterface: ComInterface> From<ObjectArray<TComInterface>> for *const *const c_void {
    fn from(arr: ObjectArray<TComInterface>) -> Self {
        arr.array.into()
    }
}

/// Compile-time assertion that `ComRc<T>` is neither `Send` nor `Sync`.
///
/// Uses the "ambiguous impl" trick: if `ComRc<IUnknown>` ever gained a
/// `Send` (or `Sync`) impl again — directly or transitively — the call
/// inside `_assert_com_rc_not_send_sync` would become ambiguous and the
/// crate would fail to compile. This is a deliberate guard rail
/// against re-introducing a blanket `unsafe impl Send for ComRc<T>`.
#[doc(hidden)]
#[allow(dead_code)]
const _: () = {
    trait AmbiguousIfSend<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}

    trait AmbiguousIfSync<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfSync<()> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}

    fn _assert_com_rc_not_send_sync() {
        <ComRc<IUnknown> as AmbiguousIfSend<_>>::some_item();
        <ComRc<IUnknown> as AmbiguousIfSync<_>>::some_item();
    }
};
