use crate as crosscom;
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
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
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
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn len(&self) -> std::os::raw::c_int {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).len)(this);
            let ret: std::os::raw::c_int = ret.into();

            ret
        }
    }

    pub fn get(&self, index: std::os::raw::c_int) -> crosscom::ComRc<IUnknown> {
        unsafe {
            let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this, index.into());
            let ret: crosscom::ComRc<crosscom::IUnknown> = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IObjectArray::INTERFACE_ID)
    }
}

pub trait IObjectArrayImpl {
    fn len(&self) -> std::os::raw::c_int;
    fn get(&self, index: std::os::raw::c_int) -> crosscom::ComRc<IUnknown>;
}

impl crosscom::ComInterface for IObjectArray {
    // 928e03ea-0017-4741-80f9-c70a93b16702
    const INTERFACE_ID: [u8; 16] = [
        146u8, 142u8, 3u8, 234u8, 0u8, 23u8, 71u8, 65u8, 128u8, 249u8, 199u8, 10u8, 147u8, 177u8,
        103u8, 2u8,
    ];
}

// Class ObjectArray

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_ObjectArray {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod ObjectArray_crosscom_impl {
            use crate as crosscom;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;

            #[repr(C)]
            pub struct ObjectArrayCcw {
                IObjectArray: crosscom::IObjectArray,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ObjectArrayCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &crosscom::IObjectArray::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ObjectArrayCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ObjectArrayCcw>(this);

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
                let object = crosscom::get_object::<ObjectArrayCcw>(this);
                (*object).inner.len().into()
            }

            unsafe extern "system" fn get(
                this: *const *const std::os::raw::c_void,
                index: std::os::raw::c_int,
            ) -> *const *const std::os::raw::c_void {
                let index: std::os::raw::c_int = index.into();

                let object = crosscom::get_object::<ObjectArrayCcw>(this);
                (*object).inner.get(index.into()).into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IObjectArrayVirtualTable_CCW_FOR_ObjectArray:
                crosscom::IObjectArrayVirtualTableCcw = crosscom::IObjectArrayVirtualTableCcw {
                offset: 0,
                vtable: crosscom::IObjectArrayVirtualTable {
                    query_interface,
                    add_ref,
                    release,
                    len,
                    get,
                },
            };

            impl crosscom::ComObject for $impl_type {
                type CcwType = ObjectArrayCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IObjectArray: crosscom::IObjectArray {
                            vtable: &GLOBAL_IObjectArrayVirtualTable_CCW_FOR_ObjectArray.vtable
                                as *const crosscom::IObjectArrayVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        this.offset(-(crosscom::offset_of!(ObjectArrayCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_ObjectArray;
