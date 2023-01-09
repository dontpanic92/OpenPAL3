use crate as opengb;
// Interface IRoleModel

#[repr(C)]
#[allow(non_snake_case)]
pub struct IRoleModelVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub on_loading: fn(
        this: *const *const std::os::raw::c_void,
        entity: &mut dyn radiance::scene::Entity,
    ) -> crosscom::Void,
    pub on_updating: fn(
        this: *const *const std::os::raw::c_void,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void,
    pub get: fn(this: *const *const std::os::raw::c_void) -> &'static opengb::scene::RoleController,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IRoleModelVirtualTableCcw {
    pub offset: isize,
    pub vtable: IRoleModelVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IRoleModel {
    pub vtable: *const IRoleModelVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IRoleModel {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IRoleModel as *const *const std::os::raw::c_void;
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
            let this = self as *const IRoleModel as *const *const std::os::raw::c_void;
            ((*self.vtable).add_ref)(this).into()
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IRoleModel as *const *const std::os::raw::c_void;
            ((*self.vtable).release)(this).into()
        }
    }

    pub fn on_loading(&self, entity: &mut dyn radiance::scene::Entity) -> crosscom::Void {
        unsafe {
            let this = self as *const IRoleModel as *const *const std::os::raw::c_void;
            ((*self.vtable).on_loading)(this, entity.into()).into()
        }
    }

    pub fn on_updating(
        &self,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IRoleModel as *const *const std::os::raw::c_void;
            ((*self.vtable).on_updating)(this, entity.into(), delta_sec.into()).into()
        }
    }

    pub fn get(&self) -> &opengb::scene::RoleController {
        unsafe {
            let this = self as *const IRoleModel as *const *const std::os::raw::c_void;
            ((*self.vtable).get)(this).into()
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IRoleModel::INTERFACE_ID)
    }
}

pub trait IRoleModelImpl {
    fn get(&self) -> &opengb::scene::RoleController;
}

impl crosscom::ComInterface for IRoleModel {
    // e11fe493-654a-4072-b883-a7ee1a35a24a
    const INTERFACE_ID: [u8; 16] = [
        225u8, 31u8, 228u8, 147u8, 101u8, 74u8, 64u8, 114u8, 184u8, 131u8, 167u8, 238u8, 26u8,
        53u8, 162u8, 74u8,
    ];
}

// Class RoleModel

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_RoleModel {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod RoleModel_crosscom_impl {
            use crate as opengb;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use opengb::classes::ICvdModelImpl;
            use opengb::classes::IRoleModelImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IMeshComponentImpl;

            #[repr(C)]
            pub struct RoleModelCcw {
                IRoleModel: opengb::classes::IRoleModel,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<RoleModelCcw>(this);
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

                    &opengb::classes::IRoleModel::INTERFACE_ID => {
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
                let object = crosscom::get_object::<RoleModelCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<RoleModelCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut RoleModelCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn get(
                this: *const *const std::os::raw::c_void,
            ) -> &'static opengb::scene::RoleController {
                unsafe {
                    let object = crosscom::get_object::<RoleModelCcw>(this);
                    (*object).inner.get()
                }
            }

            fn on_loading(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<RoleModelCcw>(this);
                    (*object).inner.on_loading(entity)
                }
            }

            fn on_updating(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
                delta_sec: f32,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<RoleModelCcw>(this);
                    (*object).inner.on_updating(entity, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IRoleModelVirtualTable_CCW_FOR_RoleModel:
                opengb::classes::IRoleModelVirtualTableCcw =
                opengb::classes::IRoleModelVirtualTableCcw {
                    offset: 0,
                    vtable: opengb::classes::IRoleModelVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = RoleModelCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IRoleModel: opengb::classes::IRoleModel {
                            vtable: &GLOBAL_IRoleModelVirtualTable_CCW_FOR_RoleModel.vtable
                                as *const opengb::classes::IRoleModelVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }
            }
        }
    };
}

pub use ComObject_RoleModel;

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
    pub on_loading: fn(
        this: *const *const std::os::raw::c_void,
        entity: &mut dyn radiance::scene::Entity,
    ) -> crosscom::Void,
    pub on_updating: fn(
        this: *const *const std::os::raw::c_void,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void,
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
            ((*self.vtable).add_ref)(this).into()
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            ((*self.vtable).release)(this).into()
        }
    }

    pub fn on_loading(&self, entity: &mut dyn radiance::scene::Entity) -> crosscom::Void {
        unsafe {
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            ((*self.vtable).on_loading)(this, entity.into()).into()
        }
    }

    pub fn on_updating(
        &self,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const ICvdModel as *const *const std::os::raw::c_void;
            ((*self.vtable).on_updating)(this, entity.into(), delta_sec.into()).into()
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
            use opengb::classes::IRoleModelImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IMeshComponentImpl;

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

            fn on_loading(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<CvdModelCcw>(this);
                    (*object).inner.on_loading(entity)
                }
            }

            fn on_updating(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
                delta_sec: f32,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<CvdModelCcw>(this);
                    (*object).inner.on_updating(entity, delta_sec)
                }
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
            }
        }
    };
}

pub use ComObject_CvdModel;
