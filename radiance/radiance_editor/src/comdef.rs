use crate as radiance_editor;
// Interface IViewContent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IViewContentVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub render: fn(
        this: *const *const std::os::raw::c_void,
        scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> crosscom::Void,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IViewContentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IViewContentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IViewContent {
    pub vtable: *const IViewContentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IViewContent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IViewContent as *const *const std::os::raw::c_void;
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
            let this = self as *const IViewContent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IViewContent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn render(
        &self,
        scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IViewContent as *const *const std::os::raw::c_void;
            let ret =
                ((*self.vtable).render)(this, scene_manager.into(), ui.into(), delta_sec.into());

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IViewContent::INTERFACE_ID)
    }
}

pub trait IViewContentImpl {
    fn render(
        &self,
        scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> crosscom::Void;
}

impl crosscom::ComInterface for IViewContent {
    // 6ac46481-7efa-45ff-a279-687b4603c746
    const INTERFACE_ID: [u8; 16] = [
        106u8, 196u8, 100u8, 129u8, 126u8, 250u8, 69u8, 255u8, 162u8, 121u8, 104u8, 123u8, 70u8,
        3u8, 199u8, 70u8,
    ];
}

// Class ResourceViewContent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_ResourceViewContent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod ResourceViewContent_crosscom_impl {
            use crate as radiance_editor;
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
            use radiance_editor::comdef::IViewContentImpl;

            #[repr(C)]
            pub struct ResourceViewContentCcw {
                IViewContent: radiance_editor::comdef::IViewContent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ResourceViewContentCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance_editor::comdef::IViewContent::INTERFACE_ID => {
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
                let object = crosscom::get_object::<ResourceViewContentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ResourceViewContentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut ResourceViewContentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn render(
                this: *const *const std::os::raw::c_void,
                scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
                ui: &imgui::Ui,
                delta_sec: f32,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<ResourceViewContentCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .render(scene_manager, ui, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IViewContentVirtualTable_CCW_FOR_ResourceViewContent:
                radiance_editor::comdef::IViewContentVirtualTableCcw =
                radiance_editor::comdef::IViewContentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance_editor::comdef::IViewContentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        render,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = ResourceViewContentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IViewContent: radiance_editor::comdef::IViewContent {
                            vtable: &GLOBAL_IViewContentVirtualTable_CCW_FOR_ResourceViewContent
                                .vtable
                                as *const radiance_editor::comdef::IViewContentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(
                            -(crosscom::offset_of!(ResourceViewContentCcw, inner) as isize),
                        );
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_ResourceViewContent;

// Class EditorApplicationLoaderComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_EditorApplicationLoaderComponent {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod EditorApplicationLoaderComponent_crosscom_impl {
    use crate as radiance_editor;
    use crosscom::ComInterface;
use radiance_editor::comdef::IViewContentImpl;
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
    pub struct EditorApplicationLoaderComponentCcw {
        IApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponent,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<EditorApplicationLoaderComponentCcw>(this);
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
        let object = crosscom::get_object::<EditorApplicationLoaderComponentCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<EditorApplicationLoaderComponentCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut EditorApplicationLoaderComponentCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }



    unsafe extern "system" fn on_loading (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<EditorApplicationLoaderComponentCcw>(this);
        (*__crosscom_object).inner.on_loading().into()
    }



    unsafe extern "system" fn on_updating (this: *const *const std::os::raw::c_void, delta_sec: std::os::raw::c_float,
) -> () {
        let delta_sec: f32 = delta_sec.into()
;

        let __crosscom_object = crosscom::get_object::<EditorApplicationLoaderComponentCcw>(this);
        (*__crosscom_object).inner.on_updating(delta_sec.into()).into()
    }






#[allow(non_upper_case_globals)]
pub const GLOBAL_IApplicationLoaderComponentVirtualTable_CCW_FOR_EditorApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponentVirtualTableCcw
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
        type CcwType = EditorApplicationLoaderComponentCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {

IApplicationLoaderComponent: radiance::comdef::IApplicationLoaderComponent {
    vtable: &GLOBAL_IApplicationLoaderComponentVirtualTable_CCW_FOR_EditorApplicationLoaderComponent.vtable
        as *const radiance::comdef::IApplicationLoaderComponentVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(EditorApplicationLoaderComponentCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_EditorApplicationLoaderComponent;

// Class MainPageDirector

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_MainPageDirector {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod MainPageDirector_crosscom_impl {
            use crate as radiance_editor;
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
            use radiance_editor::comdef::IViewContentImpl;

            #[repr(C)]
            pub struct MainPageDirectorCcw {
                IDirector: radiance::comdef::IDirector,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MainPageDirectorCcw>(this);
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
                let object = crosscom::get_object::<MainPageDirectorCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MainPageDirectorCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut MainPageDirectorCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn activate(
                this: *const *const std::os::raw::c_void,
                scene_manager: *const *const std::os::raw::c_void,
            ) -> () {
                let scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager> =
                    scene_manager.into();

                let __crosscom_object = crosscom::get_object::<MainPageDirectorCcw>(this);
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
                    let __crosscom_object = crosscom::get_object::<MainPageDirectorCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .update(scene_manager, ui, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IDirectorVirtualTable_CCW_FOR_MainPageDirector:
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
                type CcwType = MainPageDirectorCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IDirector: radiance::comdef::IDirector {
                            vtable: &GLOBAL_IDirectorVirtualTable_CCW_FOR_MainPageDirector.vtable
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
                            .offset(-(crosscom::offset_of!(MainPageDirectorCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_MainPageDirector;
