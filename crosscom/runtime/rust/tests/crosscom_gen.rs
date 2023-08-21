// Interface ITest

#[repr(C)]
#[allow(non_snake_case)]
pub struct ITestVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub test:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITestVirtualTableCcw {
    pub offset: isize,
    pub vtable: ITestVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest {
    pub vtable: *const ITestVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ITest {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ITest as *const *const std::os::raw::c_void;
        let mut raw = 0 as *const *const std::os::raw::c_void;
        let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
        let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
        if ret_val != 0 {
            None
        } else {
            Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
        }
    }

    pub fn add_ref(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn test(&self) -> std::os::raw::c_int {
        unsafe {
            let this = self as *const ITest as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).test)(this);
            let ret: std::os::raw::c_int = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ITest::INTERFACE_ID)
    }
}

pub trait ITestImpl {
    fn test(&self) -> std::os::raw::c_int;
}

impl crosscom::ComInterface for ITest {
    // 6ac46481-7efa-45ff-a279-687b4603c746
    const INTERFACE_ID: [u8; 16] = [
        106u8, 196u8, 100u8, 129u8, 126u8, 250u8, 69u8, 255u8, 162u8, 121u8, 104u8, 123u8, 70u8,
        3u8, 199u8, 70u8,
    ];
}

// Interface ITest2

#[repr(C)]
#[allow(non_snake_case)]
pub struct ITest2VirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub test:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
    pub mul: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        a: std::os::raw::c_int,
        b: std::os::raw::c_float,
    ) -> std::os::raw::c_float,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest2VirtualTableCcw {
    pub offset: isize,
    pub vtable: ITest2VirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest2 {
    pub vtable: *const ITest2VirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ITest2 {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ITest2 as *const *const std::os::raw::c_void;
        let mut raw = 0 as *const *const std::os::raw::c_void;
        let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
        let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
        if ret_val != 0 {
            None
        } else {
            Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
        }
    }

    pub fn add_ref(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest2 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest2 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn test(&self) -> std::os::raw::c_int {
        unsafe {
            let this = self as *const ITest2 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).test)(this);
            let ret: std::os::raw::c_int = ret.into();

            ret
        }
    }

    pub fn mul(&self, a: std::os::raw::c_int, b: f32) -> f32 {
        unsafe {
            let this = self as *const ITest2 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).mul)(this, a.into(), b.into());
            let ret: f32 = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ITest2::INTERFACE_ID)
    }
}

pub trait ITest2Impl {
    fn mul(&self, a: std::os::raw::c_int, b: f32) -> f32;
}

impl crosscom::ComInterface for ITest2 {
    // de3d989d-2b1d-42a3-b085-a23e40840126
    const INTERFACE_ID: [u8; 16] = [
        222u8, 61u8, 152u8, 157u8, 43u8, 29u8, 66u8, 163u8, 176u8, 133u8, 162u8, 62u8, 64u8, 132u8,
        1u8, 38u8,
    ];
}

// Interface ITest3

#[repr(C)]
#[allow(non_snake_case)]
pub struct ITest3VirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub echo: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        a: std::os::raw::c_int,
    ) -> std::os::raw::c_int,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest3VirtualTableCcw {
    pub offset: isize,
    pub vtable: ITest3VirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest3 {
    pub vtable: *const ITest3VirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ITest3 {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ITest3 as *const *const std::os::raw::c_void;
        let mut raw = 0 as *const *const std::os::raw::c_void;
        let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
        let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
        if ret_val != 0 {
            None
        } else {
            Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
        }
    }

    pub fn add_ref(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest3 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest3 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn echo(&self, a: std::os::raw::c_int) -> std::os::raw::c_int {
        unsafe {
            let this = self as *const ITest3 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).echo)(this, a.into());
            let ret: std::os::raw::c_int = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ITest3::INTERFACE_ID)
    }
}

pub trait ITest3Impl {
    fn echo(&self, a: std::os::raw::c_int) -> std::os::raw::c_int;
}

impl crosscom::ComInterface for ITest3 {
    // de3d989d-2b1d-42a3-b085-a23e40840128
    const INTERFACE_ID: [u8; 16] = [
        222u8, 61u8, 152u8, 157u8, 43u8, 29u8, 66u8, 163u8, 176u8, 133u8, 162u8, 62u8, 64u8, 132u8,
        1u8, 40u8,
    ];
}

// Interface ITest4

#[repr(C)]
#[allow(non_snake_case)]
pub struct ITest4VirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub get: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
    ) -> *const *const std::os::raw::c_void,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest4VirtualTableCcw {
    pub offset: isize,
    pub vtable: ITest4VirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ITest4 {
    pub vtable: *const ITest4VirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ITest4 {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ITest4 as *const *const std::os::raw::c_void;
        let mut raw = 0 as *const *const std::os::raw::c_void;
        let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
        let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
        if ret_val != 0 {
            None
        } else {
            Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
        }
    }

    pub fn add_ref(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest4 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ITest4 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn get(&self) -> crosscom::ComRc<ITest3> {
        unsafe {
            let this = self as *const ITest4 as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this);
            let ret: crosscom::ComRc<ITest3> = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ITest4::INTERFACE_ID)
    }
}

pub trait ITest4Impl {
    fn get(&self) -> crosscom::ComRc<ITest3>;
}

impl crosscom::ComInterface for ITest4 {
    // de3d989d-2b1d-42a3-b085-a23e40840129
    const INTERFACE_ID: [u8; 16] = [
        222u8, 61u8, 152u8, 157u8, 43u8, 29u8, 66u8, 163u8, 176u8, 133u8, 162u8, 62u8, 64u8, 132u8,
        1u8, 41u8,
    ];
}

// Class Test

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Test {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod Test_crosscom_impl {
            use crate as test;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use test::crosscom_gen::ITest2Impl;
            use test::crosscom_gen::ITest3Impl;
            use test::crosscom_gen::ITest4Impl;
            use test::crosscom_gen::ITestImpl;

            #[repr(C)]
            pub struct TestCcw {
                ITest2: test::crosscom_gen::ITest2,
                ITest: test::crosscom_gen::ITest,
                ITest3: test::crosscom_gen::ITest3,
                ITest4: test::crosscom_gen::ITest4,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<TestCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &test::crosscom_gen::ITest::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &test::crosscom_gen::ITest2::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &test::crosscom_gen::ITest3::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(2);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &test::crosscom_gen::ITest4::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(3);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<TestCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<TestCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut TestCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn mul(
                this: *const *const std::os::raw::c_void,
                a: std::os::raw::c_int,
                b: std::os::raw::c_float,
            ) -> std::os::raw::c_float {
                let a: std::os::raw::c_int = a.into();
                let b: f32 = b.into();

                let object = crosscom::get_object::<TestCcw>(this);
                (*object).inner.mul(a.into(), b.into()).into()
            }

            unsafe extern "system" fn test(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_int {
                let object = crosscom::get_object::<TestCcw>(this);
                (*object).inner.test().into()
            }

            unsafe extern "system" fn echo(
                this: *const *const std::os::raw::c_void,
                a: std::os::raw::c_int,
            ) -> std::os::raw::c_int {
                let a: std::os::raw::c_int = a.into();

                let object = crosscom::get_object::<TestCcw>(this);
                (*object).inner.echo(a.into()).into()
            }

            unsafe extern "system" fn get(
                this: *const *const std::os::raw::c_void,
            ) -> *const *const std::os::raw::c_void {
                let object = crosscom::get_object::<TestCcw>(this);
                (*object).inner.get().into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ITest2VirtualTable_CCW_FOR_Test:
                test::crosscom_gen::ITest2VirtualTableCcw =
                test::crosscom_gen::ITest2VirtualTableCcw {
                    offset: 0,
                    vtable: test::crosscom_gen::ITest2VirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        test,
                        mul,
                    },
                };

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ITestVirtualTable_CCW_FOR_Test:
                test::crosscom_gen::ITestVirtualTableCcw =
                test::crosscom_gen::ITestVirtualTableCcw {
                    offset: -1,
                    vtable: test::crosscom_gen::ITestVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        test,
                    },
                };

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ITest3VirtualTable_CCW_FOR_Test:
                test::crosscom_gen::ITest3VirtualTableCcw =
                test::crosscom_gen::ITest3VirtualTableCcw {
                    offset: -2,
                    vtable: test::crosscom_gen::ITest3VirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        echo,
                    },
                };

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ITest4VirtualTable_CCW_FOR_Test:
                test::crosscom_gen::ITest4VirtualTableCcw =
                test::crosscom_gen::ITest4VirtualTableCcw {
                    offset: -3,
                    vtable: test::crosscom_gen::ITest4VirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = TestCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        ITest2: test::crosscom_gen::ITest2 {
                            vtable: &GLOBAL_ITest2VirtualTable_CCW_FOR_Test.vtable
                                as *const test::crosscom_gen::ITest2VirtualTable,
                        },

                        ITest: test::crosscom_gen::ITest {
                            vtable: &GLOBAL_ITestVirtualTable_CCW_FOR_Test.vtable
                                as *const test::crosscom_gen::ITestVirtualTable,
                        },

                        ITest3: test::crosscom_gen::ITest3 {
                            vtable: &GLOBAL_ITest3VirtualTable_CCW_FOR_Test.vtable
                                as *const test::crosscom_gen::ITest3VirtualTable,
                        },

                        ITest4: test::crosscom_gen::ITest4 {
                            vtable: &GLOBAL_ITest4VirtualTable_CCW_FOR_Test.vtable
                                as *const test::crosscom_gen::ITest4VirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        this.offset(-(crosscom::offset_of!(TestCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_Test;
