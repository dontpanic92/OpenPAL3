use crate as shared;
// Interface IOpenPAL5Director

#[repr(C)]
#[allow(non_snake_case)]
pub struct IOpenPAL5DirectorVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub activate: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub update: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> crosscom::RawPointer,
    pub get: fn(
        this: *const *const std::os::raw::c_void,
    ) -> &'static shared::openpal5::director::OpenPAL5Director,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IOpenPAL5DirectorVirtualTableCcw {
    pub offset: isize,
    pub vtable: IOpenPAL5DirectorVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IOpenPAL5Director {
    pub vtable: *const IOpenPAL5DirectorVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IOpenPAL5Director {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IOpenPAL5Director as *const *const std::os::raw::c_void;
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
            let this = self as *const IOpenPAL5Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IOpenPAL5Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn activate(&self) -> () {
        unsafe {
            let this = self as *const IOpenPAL5Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).activate)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn update(&self, delta_sec: f32) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        unsafe {
            let this = self as *const IOpenPAL5Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).update)(this, delta_sec.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IDirector>> = ret.into();

            ret
        }
    }

    pub fn get(&self) -> &'static shared::openpal5::director::OpenPAL5Director {
        unsafe {
            let this = self as *const IOpenPAL5Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IOpenPAL5Director::INTERFACE_ID)
    }
}

pub trait IOpenPAL5DirectorImpl {
    fn get(&self) -> &'static shared::openpal5::director::OpenPAL5Director;
}

impl crosscom::ComInterface for IOpenPAL5Director {
    // 1c4589d1-769a-4fdc-aac9-16744e4a88b0
    const INTERFACE_ID: [u8; 16] = [
        28u8, 69u8, 137u8, 209u8, 118u8, 154u8, 79u8, 220u8, 170u8, 201u8, 22u8, 116u8, 78u8, 74u8,
        136u8, 176u8,
    ];
}

// Class OpenPAL5Director

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_OpenPAL5Director {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod OpenPAL5Director_crosscom_impl {
            use crate as shared;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use radiance::comdef::IAnimatedMeshComponentImpl;
            use radiance::comdef::IAnimationEventObserverImpl;
            use radiance::comdef::IApplicationImpl;
            use radiance::comdef::IApplicationLoaderComponentImpl;
            use radiance::comdef::IArmatureComponentImpl;
            use radiance::comdef::IComponentContainerImpl;
            use radiance::comdef::IComponentImpl;
            use radiance::comdef::IDirectorImpl;
            use radiance::comdef::IEntityImpl;
            use radiance::comdef::IHAnimBoneComponentImpl;
            use radiance::comdef::ISceneImpl;
            use radiance::comdef::ISceneManagerImpl;
            use radiance::comdef::ISkinnedMeshComponentImpl;
            use radiance::comdef::IStaticMeshComponentImpl;
            use shared::openpal5::comdef::IOpenPAL5DirectorImpl;

            #[repr(C)]
            pub struct OpenPAL5DirectorCcw {
                IOpenPAL5Director: shared::openpal5::comdef::IOpenPAL5Director,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<OpenPAL5DirectorCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IDirector::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &shared::openpal5::comdef::IOpenPAL5Director::INTERFACE_ID => {
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
                let object = crosscom::get_object::<OpenPAL5DirectorCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<OpenPAL5DirectorCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut OpenPAL5DirectorCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn get(
                this: *const *const std::os::raw::c_void,
            ) -> &'static shared::openpal5::director::OpenPAL5Director {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<OpenPAL5DirectorCcw>(this);
                    (*__crosscom_object).inner.get()
                }
            }

            unsafe extern "system" fn activate(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<OpenPAL5DirectorCcw>(this);
                (*__crosscom_object).inner.activate().into()
            }

            unsafe extern "system" fn update(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> crosscom::RawPointer {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<OpenPAL5DirectorCcw>(this);
                (*__crosscom_object).inner.update(delta_sec.into()).into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IOpenPAL5DirectorVirtualTable_CCW_FOR_OpenPAL5Director:
                shared::openpal5::comdef::IOpenPAL5DirectorVirtualTableCcw =
                shared::openpal5::comdef::IOpenPAL5DirectorVirtualTableCcw {
                    offset: 0,
                    vtable: shared::openpal5::comdef::IOpenPAL5DirectorVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        activate,
                        update,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = OpenPAL5DirectorCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IOpenPAL5Director: shared::openpal5::comdef::IOpenPAL5Director {
                            vtable: &GLOBAL_IOpenPAL5DirectorVirtualTable_CCW_FOR_OpenPAL5Director
                                .vtable
                                as *const shared::openpal5::comdef::IOpenPAL5DirectorVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this
                            .offset(-(crosscom::offset_of!(OpenPAL5DirectorCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_OpenPAL5Director;
