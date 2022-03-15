use std::{
    ffi::c_void,
    ops::Deref,
    os::raw::{c_char, c_long},
    sync::atomic::{AtomicU32, Ordering},
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

/*
#[repr(C)]
pub struct ITestVirtualTable {
    pub unknown: IUnknownVirtualTable,
    pub test: unsafe extern "system" fn(this: *const c_void),
}

#[repr(C)]
pub struct ITestVirtualTableCcw {
    pub offset: *const c_void,
    pub vtable: ITestVirtualTable,
}

#[repr(C)]
pub struct ITest {
    vtable: *const ITestVirtualTable,
}

impl ITest {
    // de3d989d-2b1d-42a3-b085-a23e40840126
    pub const INTERFACE_ID: [u8; 16] = [
        0xde, 0x3d, 0x98, 0x9d, 0x2b, 0x1d, 0x42, 0xa3, 0xb0, 0x85, 0xa2, 0x3e, 0x40, 0x84, 0x01,
        0x26,
    ];
    
    pub fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long {
        unsafe {
            let unknown = self as *const ITest as *const IUnknown as *const c_void;
            ((*self.vtable).unknown.query_interface)(unknown, guid, retval)
        }
    }

    pub fn add_ref(&self) -> c_long {
        unsafe {
            let unknown = self as *const ITest as *const IUnknown as *const c_void;
            ((*self.vtable).unknown.add_ref)(unknown)
        }
    }

    pub fn release(&self) -> c_long {
        unsafe {
            let unknown = self as *const ITest as *const IUnknown as *const c_void;
            ((*self.vtable).unknown.release)(unknown)
        }
    }
    
    fn test(&self) {
        unsafe { ((*self.vtable).test)(self as *const ITest as *const c_void) }
    }
}


pub trait ITestTrait {
    fn test(&self);
}

impl ComInterface for ITest {}

#[allow(non_upper_case_globals)]
pub const GLOBAL_ITestVirtualTable_CCW_FOR_TEST: ITestVirtualTableCcw = ITestVirtualTableCcw {
    offset: 0 as *const c_void,
    vtable: ITestVirtualTable {
        unknown: IUnknownVirtualTable {
            query_interface,
            add_ref,
            release,
        },
        test,
    },
};


#[repr(C)]
#[allow(non_snake_case)]
pub struct TestCcw {
    ITest: ITest,
    ref_count: AtomicU32,
    inner: Test,
}

unsafe extern "system" fn query_interface(
    this: *const c_void,
    guid: Uuid,
    retval: *mut c_void,
) -> c_long {
    let object = get_object::<TestCcw>(this);

    0
}

unsafe extern "system" fn add_ref(this: *const c_void) -> c_long {
    let object = get_object::<TestCcw>(this);
    let previous = (*object).ref_count.fetch_add(1, Ordering::SeqCst);
    (previous + 1) as c_long
}

unsafe extern "system" fn release(this: *const c_void) -> c_long {
    let object = get_object::<TestCcw>(this);

    let previous = (*object).ref_count.fetch_sub(1, Ordering::SeqCst);
    if previous - 1 == 0 {
        Box::from_raw(object as *mut TestCcw);
    }

    (previous - 1) as c_long
}

unsafe extern "system" fn test(this: *const c_void) -> () {
    let object = get_object::<TestCcw>(this);
    (*object).inner.test();
}

impl TestCcw {
    pub fn new(inner: Test) -> TestCcw {
        Self {
            ITest: ITest {
                vtable: &GLOBAL_ITestVirtualTable_CCW_FOR_TEST.vtable.unknown
                    as *const IUnknownVirtualTable
                    as *const ITestVirtualTable,
            },
            ref_count: AtomicU32::new(1),
            inner,
        }
    }
}

*/
