use crate as shared;
// Interface IOpenPAL4Director

#[repr(C)]
#[allow(non_snake_case)]
pub struct IOpenPAL4DirectorVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub activate: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        scene_manager: *const *const std::os::raw::c_void,
    ) -> (),
    pub update: fn(
        this: *const *const std::os::raw::c_void,
        scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<crosscom::ComRc<radiance::comdef::IDirector>>,
    pub get: fn(
        this: *const *const std::os::raw::c_void,
    ) -> &'static shared::openpal4::director::OpenPAL4Director,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IOpenPAL4DirectorVirtualTableCcw {
    pub offset: isize,
    pub vtable: IOpenPAL4DirectorVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IOpenPAL4Director {
    pub vtable: *const IOpenPAL4DirectorVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IOpenPAL4Director {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
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
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn activate(&self, scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>) -> () {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).activate)(this, scene_manager.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn update(
        &self,
        scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret =
                ((*self.vtable).update)(this, scene_manager.into(), ui.into(), delta_sec.into());

            ret
        }
    }

    pub fn get(&self) -> &'static shared::openpal4::director::OpenPAL4Director {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IOpenPAL4Director::INTERFACE_ID)
    }
}

pub trait IOpenPAL4DirectorImpl {
    fn get(&self) -> &'static shared::openpal4::director::OpenPAL4Director;
}

impl crosscom::ComInterface for IOpenPAL4Director {
    // f3d7f0fd-20ca-450c-bd66-ad019b984a54
    const INTERFACE_ID: [u8; 16] = [
        243u8, 215u8, 240u8, 253u8, 32u8, 202u8, 69u8, 12u8, 189u8, 102u8, 173u8, 1u8, 155u8,
        152u8, 74u8, 84u8,
    ];
}

// Class OpenPAL4Director

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_OpenPAL4Director {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod OpenPAL4Director_crosscom_impl {
            use crate as shared;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use radiance::comdef::IAnimatedMeshComponentImpl;
            use radiance::comdef::IApplicationImpl;
            use radiance::comdef::IApplicationLoaderComponentImpl;
            use radiance::comdef::IComponentContainerImpl;
            use radiance::comdef::IComponentImpl;
            use radiance::comdef::IDirectorImpl;
            use radiance::comdef::IEntityImpl;
            use radiance::comdef::IHAnimBoneComponentImpl;
            use radiance::comdef::ISceneImpl;
            use radiance::comdef::ISceneManagerImpl;
            use radiance::comdef::ISkinnedMeshComponentImpl;
            use radiance::comdef::IStaticMeshComponentImpl;
            use shared::openpal4::comdef::IOpenPAL4DirectorImpl;

            #[repr(C)]
            pub struct OpenPAL4DirectorCcw {
                IOpenPAL4Director: shared::openpal4::comdef::IOpenPAL4Director,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
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

                    &shared::openpal4::comdef::IOpenPAL4Director::INTERFACE_ID => {
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
                let object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut OpenPAL4DirectorCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn get(
                this: *const *const std::os::raw::c_void,
            ) -> &'static shared::openpal4::director::OpenPAL4Director {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                    (*__crosscom_object).inner.get()
                }
            }

            unsafe extern "system" fn activate(
                this: *const *const std::os::raw::c_void,
                scene_manager: *const *const std::os::raw::c_void,
            ) -> () {
                let scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager> =
                    scene_manager.into();

                let __crosscom_object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                (*__crosscom_object)
                    .inner
                    .activate(scene_manager.into())
                    .into()
            }

            fn update(
                this: *const *const std::os::raw::c_void,
                scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
                ui: &imgui::Ui,
                delta_sec: f32,
            ) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .update(scene_manager, ui, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IOpenPAL4DirectorVirtualTable_CCW_FOR_OpenPAL4Director:
                shared::openpal4::comdef::IOpenPAL4DirectorVirtualTableCcw =
                shared::openpal4::comdef::IOpenPAL4DirectorVirtualTableCcw {
                    offset: 0,
                    vtable: shared::openpal4::comdef::IOpenPAL4DirectorVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        activate,
                        update,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = OpenPAL4DirectorCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IOpenPAL4Director: shared::openpal4::comdef::IOpenPAL4Director {
                            vtable: &GLOBAL_IOpenPAL4DirectorVirtualTable_CCW_FOR_OpenPAL4Director
                                .vtable
                                as *const shared::openpal4::comdef::IOpenPAL4DirectorVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this
                            .offset(-(crosscom::offset_of!(OpenPAL4DirectorCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_OpenPAL4Director;
