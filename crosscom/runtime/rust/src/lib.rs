use std::{
    ffi::c_void,
    ops::Deref,
    os::raw::{c_char, c_long},
};

use uuid::Uuid;

pub struct ComRc<TComInterface: ComInterface> {
    this: *const TComInterface,
}

impl<TComInterface: ComInterface> ComRc<TComInterface> {
    pub fn from_object<TComObject: ComObject>(obj: TComObject) -> ComRc<TComInterface> {
        let p = Box::new(TComObject::create_ccw(obj));
        let raw = Box::into_raw(p);

        Self {
            this: raw as *const TComInterface,
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

pub trait ComInterface {}

pub trait ComObject {
    type CcwType;
    fn create_ccw(self) -> Self::CcwType;
}

#[repr(C)]
pub struct IUnknownVirtualTable {
    pub query_interface:
        unsafe extern "system" fn(this: *const c_void, guid: Uuid, retval: *mut c_void) -> c_long,
    pub add_ref: unsafe extern "system" fn(this: *const c_void) -> c_long,
    pub release: unsafe extern "system" fn(this: *const c_void) -> c_long,
}

#[repr(C)]
pub struct IUnknownVirtualTableCcw {
    pub offset: *const c_void,
    pub vtable: IUnknownVirtualTable,
}

pub trait IUnknownTrait {
    fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long;
    fn add_ref(&self) -> c_long;
    fn release(&self) -> c_long;
}

#[repr(C)]
pub struct IUnknown {
    vtable: *const IUnknownVirtualTable,
}

impl IUnknown {
    // 00000000-0000-0000-C000-000000000046
    pub const INTERFACE_ID: [u8; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x46,
    ];
}

impl ComInterface for IUnknown {}

impl IUnknownTrait for IUnknown {
    fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long {
        unsafe {
            ((*self.vtable).query_interface)(self as *const IUnknown as *const c_void, guid, retval)
        }
    }

    fn add_ref(&self) -> c_long {
        unsafe { ((*self.vtable).add_ref)(self as *const IUnknown as *const c_void) }
    }

    fn release(&self) -> c_long {
        unsafe { ((*self.vtable).release)(self as *const IUnknown as *const c_void) }
    }
}

pub unsafe fn get_object<T>(this: *const c_void) -> *const T {
    let vtable = *(this as *const *const *const c_void);
    let vtable_ccw = vtable.offset(-1);
    let offset = (*vtable_ccw) as isize;
    (this as *const c_char).offset(offset) as *const T
}
