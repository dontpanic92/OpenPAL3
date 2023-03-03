use crate as yaobow;

// Class OpenPal3ApplicationLoaderComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_OpenPal3ApplicationLoaderComponent {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod OpenPal3ApplicationLoaderComponent_crosscom_impl {
    use crate as yaobow;
    use crosscom::ComInterface;
use crosscom::IUnknownImpl;
use crosscom::IObjectArrayImpl;
use radiance::comdef::IComponentImpl;
use radiance::comdef::IComponentContainerImpl;
use radiance::comdef::IApplicationImpl;
use radiance::comdef::IApplicationLoaderComponentImpl;
use radiance::comdef::ISceneImpl;
use radiance::comdef::IEntityImpl;
use radiance::comdef::IStaticMeshComponentImpl;
use radiance::comdef::IAnimatedMeshComponentImpl;
use radiance::comdef::IDirectorImpl;
use radiance::comdef::ISceneManagerImpl;


    #[repr(C)]
    pub struct OpenPal3ApplicationLoaderComponentCcw {
        IApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponent,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<OpenPal3ApplicationLoaderComponentCcw>(this);
        match guid.as_bytes() {

&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


&radiance::comdef::IComponent::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


&radiance::comdef::IApplicationLoaderComponent::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<OpenPal3ApplicationLoaderComponentCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<OpenPal3ApplicationLoaderComponentCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut OpenPal3ApplicationLoaderComponentCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }



    unsafe extern "system" fn on_loading (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<OpenPal3ApplicationLoaderComponentCcw>(this);
        (*__crosscom_object).inner.on_loading().into()
    }



    unsafe extern "system" fn on_updating (this: *const *const std::os::raw::c_void, delta_sec: std::os::raw::c_float,
) -> () {
        let delta_sec: f32 = delta_sec.into()
;

        let __crosscom_object = crosscom::get_object::<OpenPal3ApplicationLoaderComponentCcw>(this);
        (*__crosscom_object).inner.on_updating(delta_sec.into()).into()
    }






#[allow(non_upper_case_globals)]
pub const GLOBAL_IApplicationLoaderComponentVirtualTable_CCW_FOR_OpenPal3ApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponentVirtualTableCcw
    = radiance::comdef::IApplicationLoaderComponentVirtualTableCcw {
    offset: 0,
    vtable: radiance::comdef::IApplicationLoaderComponentVirtualTable {
        query_interface,
add_ref,
release,
on_loading,
on_updating,

    },
};




    impl crosscom::ComObject for $impl_type {
        type CcwType = OpenPal3ApplicationLoaderComponentCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {

IApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponent {
    vtable: &GLOBAL_IApplicationLoaderComponentVirtualTable_CCW_FOR_OpenPal3ApplicationLoaderComponent.vtable
        as *const radiance::comdef::IApplicationLoaderComponentVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(OpenPal3ApplicationLoaderComponentCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_OpenPal3ApplicationLoaderComponent;

// Class OpenPal4ApplicationLoaderComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_OpenPal4ApplicationLoaderComponent {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod OpenPal4ApplicationLoaderComponent_crosscom_impl {
    use crate as yaobow;
    use crosscom::ComInterface;
use crosscom::IUnknownImpl;
use crosscom::IObjectArrayImpl;
use radiance::comdef::IComponentImpl;
use radiance::comdef::IComponentContainerImpl;
use radiance::comdef::IApplicationImpl;
use radiance::comdef::IApplicationLoaderComponentImpl;
use radiance::comdef::ISceneImpl;
use radiance::comdef::IEntityImpl;
use radiance::comdef::IStaticMeshComponentImpl;
use radiance::comdef::IAnimatedMeshComponentImpl;
use radiance::comdef::IDirectorImpl;
use radiance::comdef::ISceneManagerImpl;


    #[repr(C)]
    pub struct OpenPal4ApplicationLoaderComponentCcw {
        IApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponent,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<OpenPal4ApplicationLoaderComponentCcw>(this);
        match guid.as_bytes() {

&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


&radiance::comdef::IComponent::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


&radiance::comdef::IApplicationLoaderComponent::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<OpenPal4ApplicationLoaderComponentCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<OpenPal4ApplicationLoaderComponentCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut OpenPal4ApplicationLoaderComponentCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }



    unsafe extern "system" fn on_loading (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<OpenPal4ApplicationLoaderComponentCcw>(this);
        (*__crosscom_object).inner.on_loading().into()
    }



    unsafe extern "system" fn on_updating (this: *const *const std::os::raw::c_void, delta_sec: std::os::raw::c_float,
) -> () {
        let delta_sec: f32 = delta_sec.into()
;

        let __crosscom_object = crosscom::get_object::<OpenPal4ApplicationLoaderComponentCcw>(this);
        (*__crosscom_object).inner.on_updating(delta_sec.into()).into()
    }






#[allow(non_upper_case_globals)]
pub const GLOBAL_IApplicationLoaderComponentVirtualTable_CCW_FOR_OpenPal4ApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponentVirtualTableCcw
    = radiance::comdef::IApplicationLoaderComponentVirtualTableCcw {
    offset: 0,
    vtable: radiance::comdef::IApplicationLoaderComponentVirtualTable {
        query_interface,
add_ref,
release,
on_loading,
on_updating,

    },
};




    impl crosscom::ComObject for $impl_type {
        type CcwType = OpenPal4ApplicationLoaderComponentCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {

IApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponent {
    vtable: &GLOBAL_IApplicationLoaderComponentVirtualTable_CCW_FOR_OpenPal4ApplicationLoaderComponent.vtable
        as *const radiance::comdef::IApplicationLoaderComponentVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(OpenPal4ApplicationLoaderComponentCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_OpenPal4ApplicationLoaderComponent;

// Class MainMenuDirector

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_MainMenuDirector {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod MainMenuDirector_crosscom_impl {
            use crate as yaobow;
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
            use radiance::comdef::ISceneImpl;
            use radiance::comdef::ISceneManagerImpl;
            use radiance::comdef::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct MainMenuDirectorCcw {
                IDirector: radiance::comdef::IDirector,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MainMenuDirectorCcw>(this);
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

                    _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MainMenuDirectorCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MainMenuDirectorCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut MainMenuDirectorCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn activate(
                this: *const *const std::os::raw::c_void,
                scene_manager: *const *const std::os::raw::c_void,
            ) -> () {
                let scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager> =
                    scene_manager.into();

                let __crosscom_object = crosscom::get_object::<MainMenuDirectorCcw>(this);
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
                    let __crosscom_object = crosscom::get_object::<MainMenuDirectorCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .update(scene_manager, ui, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IDirectorVirtualTable_CCW_FOR_MainMenuDirector:
                radiance::comdef::IDirectorVirtualTableCcw =
                radiance::comdef::IDirectorVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::IDirectorVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        activate,
                        update,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = MainMenuDirectorCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IDirector: radiance::comdef::IDirector {
                            vtable: &GLOBAL_IDirectorVirtualTable_CCW_FOR_MainMenuDirector.vtable
                                as *const radiance::comdef::IDirectorVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this
                            .offset(-(crosscom::offset_of!(MainMenuDirectorCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_MainMenuDirector;
