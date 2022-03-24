use std::{ffi::c_void, os::raw::c_long};

use crosscom::ComInterface;
use uuid::Uuid;




/////////////////////// Interface ITest ///////////////////////

#[repr(C)]
#[allow(non_snake_case)]
pub struct ITestVirtualTable {
    
    pub query_interface: unsafe extern "system" fn (this: *const std::os::raw::c_void, guid: uuid::Uuid, retval: *mut std::os::raw::c_void,) -> std::os::raw::c_long,
    
    pub add_ref: unsafe extern "system" fn (this: *const std::os::raw::c_void,) -> std::os::raw::c_long,
    
    pub release: unsafe extern "system" fn (this: *const std::os::raw::c_void,) -> std::os::raw::c_long,
    
    pub test: unsafe extern "system" fn (this: *const std::os::raw::c_void, ) -> (),
    
}

#[allow(dead_code)]
#[repr(C)]
pub struct ITestVirtualTableCcw {
    pub offset: *const c_void,
    pub vtable: ITestVirtualTable,
}

#[allow(dead_code)]
#[repr(C)]
pub struct ITest {
    pub vtable: *const ITestVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ITest {
    // 6ac46481-7efa-45ff-a279-687b4603c746
    pub const INTERFACE_ID: [u8; 16] = [
        0x6a, 0xc4, 0x64, 0x81, 0x7e, 0xfa, 0x45, 0xff, 0xa2, 0x79, 0x68, 0x7b, 0x46, 0x03, 0xc7, 0x46
    ];
    
    
        pub fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long {
            unsafe {
                let this = self as *const ITest as *const c_void;
                ((*self.vtable).query_interface)(this, guid, retval)
            }
        }
    
        pub fn add_ref(&self) -> c_long {
            unsafe {
                let this = self as *const ITest as *const c_void;
                ((*self.vtable).add_ref)(this, )
            }
        }
    
        pub fn release(&self) -> c_long {
            unsafe {
                let this = self as *const ITest as *const c_void;
                ((*self.vtable).release)(this, )
            }
        }
    
        pub fn test (&self, ) -> () {
            unsafe {
                let this = self as *const ITest as *const c_void;
                ((*self.vtable).test)(this, )
            }
        }
    
}


pub trait ITestTrait {
    
    fn test (&self, ) -> ();
    
}

impl ComInterface for ITest {}



/////////////////////// Interface ITest2 ///////////////////////

#[repr(C)]
#[allow(non_snake_case)]
pub struct ITest2VirtualTable {
    
    pub query_interface: unsafe extern "system" fn (this: *const std::os::raw::c_void, guid: uuid::Uuid, retval: *mut std::os::raw::c_void,) -> std::os::raw::c_long,
    
    pub add_ref: unsafe extern "system" fn (this: *const std::os::raw::c_void,) -> std::os::raw::c_long,
    
    pub release: unsafe extern "system" fn (this: *const std::os::raw::c_void,) -> std::os::raw::c_long,
    
    pub test: unsafe extern "system" fn (this: *const std::os::raw::c_void, ) -> (),
    
    pub test2: unsafe extern "system" fn (this: *const std::os::raw::c_void, ) -> (),
    
}

#[allow(dead_code)]
#[repr(C)]
pub struct ITest2VirtualTableCcw {
    pub offset: *const c_void,
    pub vtable: ITest2VirtualTable,
}

#[allow(dead_code)]
#[repr(C)]
pub struct ITest2 {
    pub vtable: *const ITest2VirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ITest2 {
    // de3d989d-2b1d-42a3-b085-a23e40840126
    pub const INTERFACE_ID: [u8; 16] = [
        0xde, 0x3d, 0x98, 0x9d, 0x2b, 0x1d, 0x42, 0xa3, 0xb0, 0x85, 0xa2, 0x3e, 0x40, 0x84, 0x01, 0x26
    ];
    
    
        pub fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long {
            unsafe {
                let this = self as *const ITest2 as *const c_void;
                ((*self.vtable).query_interface)(this, guid, retval)
            }
        }
    
        pub fn add_ref(&self) -> c_long {
            unsafe {
                let this = self as *const ITest2 as *const c_void;
                ((*self.vtable).add_ref)(this, )
            }
        }
    
        pub fn release(&self) -> c_long {
            unsafe {
                let this = self as *const ITest2 as *const c_void;
                ((*self.vtable).release)(this, )
            }
        }
    
        pub fn test (&self, ) -> () {
            unsafe {
                let this = self as *const ITest2 as *const c_void;
                ((*self.vtable).test)(this, )
            }
        }
    
        pub fn test2 (&self, ) -> () {
            unsafe {
                let this = self as *const ITest2 as *const c_void;
                ((*self.vtable).test2)(this, )
            }
        }
    
}


pub trait ITest2Trait {
    
    fn test2 (&self, ) -> ();
    
}

impl ComInterface for ITest2 {}




/////////////////////// Class Test ///////////////////////

#[allow(unused)]
macro_rules! implement_Test {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod Test_impl {


use crate::crosscom_gen::ITestTrait;

use crate::crosscom_gen::ITest2Trait;


#[repr(C)]
pub struct TestCcw {
    
    ITest2: crate::crosscom_gen::ITest2,
    

    ref_count: std::sync::atomic::AtomicU32,
    inner: $impl_type,
}

unsafe extern "system" fn query_interface(
    this: *const std::os::raw::c_void,
    guid: uuid::Uuid,
    retval: *mut std::os::raw::c_void,
) -> std::os::raw::c_long {
    let object = crosscom::get_object::<TestCcw>(this);

    0
}

unsafe extern "system" fn add_ref(this: *const std::os::raw::c_void) -> std::os::raw::c_long {
    let object = crosscom::get_object::<TestCcw>(this);
    let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    (previous + 1) as std::os::raw::c_long
}

unsafe extern "system" fn release(this: *const std::os::raw::c_void) -> std::os::raw::c_long {
    let object = crosscom::get_object::<TestCcw>(this);

    let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    if previous - 1 == 0 {
        Box::from_raw(object as *mut TestCcw);
    }

    (previous - 1) as std::os::raw::c_long
}


unsafe extern "system" fn test2 (this: *const std::os::raw::c_void, ) -> () {
    let object = crosscom::get_object::<TestCcw>(this);
    (*object).inner.test2();
}

unsafe extern "system" fn test (this: *const std::os::raw::c_void, ) -> () {
    let object = crosscom::get_object::<TestCcw>(this);
    (*object).inner.test();
}



#[allow(non_upper_case_globals)]
pub const GLOBAL_ITest2VirtualTable_CCW_FOR_Test: crate::crosscom_gen::ITest2VirtualTableCcw 
    = crate::crosscom_gen::ITest2VirtualTableCcw {
    offset: 0 as *const std::os::raw::c_void,
    vtable: crate::crosscom_gen::ITest2VirtualTable {
        
        query_interface,
        
        add_ref,
        
        release,
        
        test,
        
        test2,
        
    },
};


impl TestCcw {
    pub fn new(inner: $impl_type) -> TestCcw {
        Self {
            
            ITest2: crate::crosscom_gen::ITest2 {
                vtable: &GLOBAL_ITest2VirtualTable_CCW_FOR_Test.vtable
                    as *const crate::crosscom_gen::ITest2VirtualTable,
            },
            
            ref_count: std::sync::atomic::AtomicU32::new(1),
            inner,
        }
    }
}

impl crosscom::ComObject for $impl_type {
    type CcwType = TestCcw;

    fn create_ccw(self) -> Self::CcwType {
        Self::CcwType::new(self)
    }
}


}
};
}


