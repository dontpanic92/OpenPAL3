use std::{ffi::c_void, marker::PhantomData, os::raw::c_long, mem::size_of};

use uuid::Uuid;

pub struct ComRc<TComInterface: ComInterface> {
    this: *mut c_void,
    _pd: PhantomData<TComInterface>,
}

impl<TComInterface: ComInterface> ComRc<TComInterface> {
    pub fn from<TComObject>(obj: TComObject) -> ComRc<TComInterface> {
        let p = Box::new(obj);
        std::mem::forget(&p);
        Self {
            this: Box::into_raw(p) as *mut c_void,
            _pd: PhantomData,
        }
    }
}

pub trait ComInterface {
    type VirtualTableType;
    type TraitType: ?Sized;
}

#[repr(C)]
pub struct IUnknownVirtualTable {
    pub query_interface: unsafe extern "system" fn(this: *const c_void, guid: Uuid, retval: *mut c_void) -> c_long,
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
}




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

pub struct ITest;
pub trait ITestTrait {
    fn test(&self);
}

impl ComInterface for ITest {
    type VirtualTableType = ITestVirtualTable;
    type TraitType = dyn ITestTrait;
}

#[allow(non_upper_case_globals)]
pub const GLOBAL_ITestVirtualTable_CCW_FOR_TEST: ITestVirtualTableCcw = ITestVirtualTableCcw {
    offset: 0 as *const c_void,
    vtable: ITestVirtualTable {
        unknown: IUnknownVirtualTable {
            query_interface,
            add_ref: 0,
            release: 0,
        },
        test: 0,
    },
};

unsafe extern "system" fn query_interface(
    mut this: *const c_void,
    guid: Uuid,
    retval: *mut c_void,
) -> c_long {
    let vtable = *(this as *const *const c_void);
    let vtable_ccw = vtable.offset(- (size_of::<*const c_void>() as isize)) as *const IUnknownVirtualTableCcw;
    let object = vtable.offset((*vtable_ccw).offset as isize) as *const TestObject;
    (*object).query_interface(guid, retval)
}

#[repr(C)]
#[allow(non_snake_case)]
pub struct TestObject {
    vtable_ITestVirtualTable: *const ITestVirtualTableCcw,
    inner: Test,
}

impl TestObject {
    pub fn new(inner: Test) -> TestObject {
        Self {
            vtable_ITestVirtualTable: &GLOBAL_ITestVirtualTable_CCW_FOR_TEST,
            inner,
        }
    }
}

impl IUnknownTrait for TestObject {
    fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long {
        return 0;
    }
}

pub struct Test {}

impl ITestTrait for Test {
    fn test(&self) {
        println!("In Rust!");
    }
}
