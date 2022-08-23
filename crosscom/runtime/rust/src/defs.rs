// Interface IObjectArray

#[repr(C)]
#[allow(non_snake_case)]
pub struct IObjectArrayVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub len:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
    pub get: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        index: std::os::raw::c_int,
    ) -> *const *const std::os::raw::c_void,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IObjectArrayVirtualTableCcw {
    pub offset: isize,
    pub vtable: IObjectArrayVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IObjectArray {
    pub vtable: *const IObjectArrayVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IObjectArray {
    pub fn query_interface<T: crate::ComInterface>(&self) -> Option<crate::ComRc<T>> {
        let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
        let mut raw = 0 as *const *const std::os::raw::c_void;
        let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
        let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
        if ret_val != 0 {
            None
        } else {
            Some(unsafe { crate::ComRc::<T>::from_raw_pointer(raw) })
        }
    }

    pub fn add_ref(&self) -> i32 {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            ((*self.vtable).add_ref)(this).into()
        }
    }

    pub fn release(&self) -> i32 {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            ((*self.vtable).release)(this).into()
        }
    }

    pub fn len(&self) -> i32 {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            ((*self.vtable).len)(this).into()
        }
    }

    pub fn get(&self, index: i32) -> crate::ComRc<IUnknown> {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            ((*self.vtable).get)(this, index).into()
        }
    }
}

pub trait IObjectArrayImpl {
    fn len(&self) -> i32;
    fn get(&self, index: i32) -> crate::ComRc<IUnknown>;
}

impl crate::ComInterface for IObjectArray {
    // 928e03ea-0017-4741-80f9-c70a93b16702
    const INTERFACE_ID: [u8; 16] = [
        146u8, 142u8, 3u8, 234u8, 0u8, 23u8, 71u8, 65u8, 128u8, 249u8, 199u8, 10u8, 147u8, 177u8,
        103u8, 2u8,
    ];
}

// Class ObjectArray

#[allow(unused)]
macro_rules! ComObject_ObjectArray {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod ObjectArray_crosscom_impl {
            use crate::ComInterface;
            use crate::IObjectArrayImpl;

            #[repr(C)]
            pub struct ObjectArrayCcw {
                IObjectArray: crate::IObjectArray,

                ref_count: std::sync::atomic::AtomicU32,
                pub(crate) inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crate::get_object::<ObjectArrayCcw>(this);
                match guid.as_bytes() {
                    &crate::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crate::ResultCode::Ok as i32
                    }

                    &crate::IObjectArray::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crate::ResultCode::Ok as i32
                    }

                    _ => crate::ResultCode::ENoInterface as i32,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crate::get_object::<ObjectArrayCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crate::get_object::<ObjectArrayCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut ObjectArrayCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn len(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_int {
                let object = crate::get_object::<ObjectArrayCcw>(this);
                (*object).inner.len().into()
            }

            unsafe extern "system" fn get(
                this: *const *const std::os::raw::c_void,
                index: std::os::raw::c_int,
            ) -> *const *const std::os::raw::c_void {
                let object = crate::get_object::<ObjectArrayCcw>(this);
                (*object).inner.get(index).into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IObjectArrayVirtualTable_CCW_FOR_ObjectArray:
                crate::IObjectArrayVirtualTableCcw = crate::IObjectArrayVirtualTableCcw {
                offset: 0,
                vtable: crate::IObjectArrayVirtualTable {
                    query_interface,
                    add_ref,
                    release,
                    len,
                    get,
                },
            };

            impl crate::ComObject for $impl_type {
                type CcwType = ObjectArrayCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IObjectArray: crate::IObjectArray {
                            vtable: &GLOBAL_IObjectArrayVirtualTable_CCW_FOR_ObjectArray.vtable
                                as *const crate::IObjectArrayVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }
            }
        }
    };
}
