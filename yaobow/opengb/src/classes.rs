use crate as opengb;
// Interface IRoleController

#[repr(C)]
#[allow(non_snake_case)]
pub struct IRoleControllerVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub on_loading: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub on_updating: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> (),
    pub get: fn(this: *const *const std::os::raw::c_void) -> &'static opengb::scene::RoleController,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IRoleControllerVirtualTableCcw {
    pub offset: isize,
    pub vtable: IRoleControllerVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IRoleController {
    pub vtable: *const IRoleControllerVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IRoleController {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IRoleController as *const *const std::os::raw::c_void;
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
            let this = self as *const IRoleController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IRoleController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IRoleController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IRoleController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get(&self) -> &'static opengb::scene::RoleController {
        unsafe {
            let this = self as *const IRoleController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IRoleController::INTERFACE_ID)
    }
}

pub trait IRoleControllerImpl {
    fn get(&self) -> &'static opengb::scene::RoleController;
}

impl crosscom::ComInterface for IRoleController {
    // e11fe493-654a-4072-b883-a7ee1a35a24a
    const INTERFACE_ID: [u8; 16] = [
        225u8, 31u8, 228u8, 147u8, 101u8, 74u8, 64u8, 114u8, 184u8, 131u8, 167u8, 238u8, 26u8,
        53u8, 162u8, 74u8,
    ];
}

// Class RoleController

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_RoleController {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod RoleController_crosscom_impl {
            use crate as opengb;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use opengb::classes::ICvdModelImpl;
            use opengb::classes::IRoleControllerImpl;
            use opengb::classes::IScnSceneComponentImpl;
            use radiance::interfaces::IAnimatedMeshComponentImpl;
            use radiance::interfaces::IComponentContainerImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IEntityImpl;
            use radiance::interfaces::ISceneImpl;
            use radiance::interfaces::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct RoleControllerCcw {
                IRoleController: opengb::classes::IRoleController,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<RoleControllerCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IComponent::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &opengb::classes::IRoleController::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<RoleControllerCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<RoleControllerCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut RoleControllerCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn get(
                this: *const *const std::os::raw::c_void,
            ) -> &'static opengb::scene::RoleController {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<RoleControllerCcw>(this);
                    (*__crosscom_object).inner.get()
                }
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<RoleControllerCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<RoleControllerCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IRoleControllerVirtualTable_CCW_FOR_RoleController:
                opengb::classes::IRoleControllerVirtualTableCcw =
                opengb::classes::IRoleControllerVirtualTableCcw {
                    offset: 0,
                    vtable: opengb::classes::IRoleControllerVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = RoleControllerCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IRoleController: opengb::classes::IRoleController {
                            vtable: &GLOBAL_IRoleControllerVirtualTable_CCW_FOR_RoleController
                                .vtable
                                as *const opengb::classes::IRoleControllerVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this =
                            this.offset(-(crosscom::offset_of!(RoleControllerCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_RoleController;

// Interface ICvdModel

#[repr(C)]
#[allow(non_snake_case)]
pub struct ICvdModelVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub on_loading: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub on_updating: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> (),
}

#[repr(C)]
#[allow(dead_code)]
pub struct ICvdModelVirtualTableCcw {
    pub offset: isize,
    pub vtable: ICvdModelVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ICvdModel {
    pub vtable: *const ICvdModelVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ICvdModel {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
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
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ICvdModel::INTERFACE_ID)
    }
}

pub trait ICvdModelImpl {}

impl crosscom::ComInterface for ICvdModel {
    // 9c6dc3a5-d858-40c0-960b-b3527ad4516f
    const INTERFACE_ID: [u8; 16] = [
        156u8, 109u8, 195u8, 165u8, 216u8, 88u8, 64u8, 192u8, 150u8, 11u8, 179u8, 82u8, 122u8,
        212u8, 81u8, 111u8,
    ];
}

// Class CvdModel

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_CvdModel {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod CvdModel_crosscom_impl {
            use crate as opengb;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use opengb::classes::ICvdModelImpl;
            use opengb::classes::IRoleControllerImpl;
            use opengb::classes::IScnSceneComponentImpl;
            use radiance::interfaces::IAnimatedMeshComponentImpl;
            use radiance::interfaces::IComponentContainerImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IEntityImpl;
            use radiance::interfaces::ISceneImpl;
            use radiance::interfaces::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct CvdModelCcw {
                IComponent: radiance::interfaces::IComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<CvdModelCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IComponent::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<CvdModelCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<CvdModelCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut CvdModelCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<CvdModelCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<CvdModelCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IComponentVirtualTable_CCW_FOR_CvdModel:
                radiance::interfaces::IComponentVirtualTableCcw =
                radiance::interfaces::IComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::interfaces::IComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = CvdModelCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IComponent: radiance::interfaces::IComponent {
                            vtable: &GLOBAL_IComponentVirtualTable_CCW_FOR_CvdModel.vtable
                                as *const radiance::interfaces::IComponentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this =
                            this.offset(-(crosscom::offset_of!(CvdModelCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_CvdModel;

// Interface IScnSceneComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IScnSceneComponentVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub on_loading: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub on_updating: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> (),
    pub get: fn(this: *const *const std::os::raw::c_void) -> &'static opengb::scene::ScnScene,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IScnSceneComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IScnSceneComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IScnSceneComponent {
    pub vtable: *const IScnSceneComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IScnSceneComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IScnSceneComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const IScnSceneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IScnSceneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IScnSceneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IScnSceneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get(&self) -> &'static opengb::scene::ScnScene {
        unsafe {
            let this = self as *const IScnSceneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IScnSceneComponent::INTERFACE_ID)
    }
}

pub trait IScnSceneComponentImpl {
    fn get(&self) -> &'static opengb::scene::ScnScene;
}

impl crosscom::ComInterface for IScnSceneComponent {
    // 77fe1a3d-05cf-47f9-b80a-08be6d19b0a4
    const INTERFACE_ID: [u8; 16] = [
        119u8, 254u8, 26u8, 61u8, 5u8, 207u8, 71u8, 249u8, 184u8, 10u8, 8u8, 190u8, 109u8, 25u8,
        176u8, 164u8,
    ];
}

// Class ScnSceneComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_ScnSceneComponent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod ScnSceneComponent_crosscom_impl {
            use crate as opengb;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use opengb::classes::ICvdModelImpl;
            use opengb::classes::IRoleControllerImpl;
            use opengb::classes::IScnSceneComponentImpl;
            use radiance::interfaces::IAnimatedMeshComponentImpl;
            use radiance::interfaces::IComponentContainerImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IEntityImpl;
            use radiance::interfaces::ISceneImpl;
            use radiance::interfaces::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct ScnSceneComponentCcw {
                IScnSceneComponent: opengb::classes::IScnSceneComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ScnSceneComponentCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IComponent::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &opengb::classes::IScnSceneComponent::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
                }
            }

            unsafe extern "system" fn add_ref(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ScnSceneComponentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ScnSceneComponentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut ScnSceneComponentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn get(this: *const *const std::os::raw::c_void) -> &'static opengb::scene::ScnScene {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<ScnSceneComponentCcw>(this);
                    (*__crosscom_object).inner.get()
                }
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<ScnSceneComponentCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<ScnSceneComponentCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IScnSceneComponentVirtualTable_CCW_FOR_ScnSceneComponent:
                opengb::classes::IScnSceneComponentVirtualTableCcw =
                opengb::classes::IScnSceneComponentVirtualTableCcw {
                    offset: 0,
                    vtable: opengb::classes::IScnSceneComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = ScnSceneComponentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IScnSceneComponent: opengb::classes::IScnSceneComponent {
                            vtable: &GLOBAL_IScnSceneComponentVirtualTable_CCW_FOR_ScnSceneComponent
                                .vtable
                                as *const opengb::classes::IScnSceneComponentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this
                            .offset(-(crosscom::offset_of!(ScnSceneComponentCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_ScnSceneComponent;
