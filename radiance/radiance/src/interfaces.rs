use crate as radiance;
// Interface IComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IComponentVirtualTable {
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
pub struct IComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IComponent {
    pub vtable: *const IComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).add_ref)(this).into()
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).release)(this).into()
        }
    }

    pub fn on_loading(&self, entity: &mut dyn radiance::scene::Entity) -> crosscom::Void {
        unsafe {
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).on_loading)(this, entity.into()).into()
        }
    }

    pub fn on_updating(
        &self,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).on_updating)(this, entity.into(), delta_sec.into()).into()
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IComponent::INTERFACE_ID)
    }
}

pub trait IComponentImpl {
    fn on_loading(&self, entity: &mut dyn radiance::scene::Entity) -> crosscom::Void;
    fn on_updating(
        &self,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void;
}

impl crosscom::ComInterface for IComponent {
    // 03748ce3-689d-4325-b1de-59de516b576b
    const INTERFACE_ID: [u8; 16] = [
        3u8, 116u8, 140u8, 227u8, 104u8, 157u8, 67u8, 37u8, 177u8, 222u8, 89u8, 222u8, 81u8, 107u8,
        87u8, 107u8,
    ];
}

// Interface IMeshComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IMeshComponentVirtualTable {
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
pub struct IMeshComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IMeshComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IMeshComponent {
    pub vtable: *const IMeshComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IMeshComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IMeshComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const IMeshComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).add_ref)(this).into()
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IMeshComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).release)(this).into()
        }
    }

    pub fn on_loading(&self, entity: &mut dyn radiance::scene::Entity) -> crosscom::Void {
        unsafe {
            let this = self as *const IMeshComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).on_loading)(this, entity.into()).into()
        }
    }

    pub fn on_updating(
        &self,
        entity: &mut dyn radiance::scene::Entity,
        delta_sec: f32,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IMeshComponent as *const *const std::os::raw::c_void;
            ((*self.vtable).on_updating)(this, entity.into(), delta_sec.into()).into()
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IMeshComponent::INTERFACE_ID)
    }
}

pub trait IMeshComponentImpl {}

impl crosscom::ComInterface for IMeshComponent {
    // 8dd91852-476b-401b-8668-ba9cc331b7a1
    const INTERFACE_ID: [u8; 16] = [
        141u8, 217u8, 24u8, 82u8, 71u8, 107u8, 64u8, 27u8, 134u8, 104u8, 186u8, 156u8, 195u8, 49u8,
        183u8, 161u8,
    ];
}

// Class MeshComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_MeshComponent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod MeshComponent_crosscom_impl {
            use crate as radiance;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IMeshComponentImpl;

            #[repr(C)]
            pub struct MeshComponentCcw {
                IMeshComponent: radiance::interfaces::IMeshComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MeshComponentCcw>(this);
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

                    &radiance::interfaces::IMeshComponent::INTERFACE_ID => {
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
                let object = crosscom::get_object::<MeshComponentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<MeshComponentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut MeshComponentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn on_loading(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<MeshComponentCcw>(this);
                    (*object).inner.on_loading(entity)
                }
            }

            fn on_updating(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
                delta_sec: f32,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<MeshComponentCcw>(this);
                    (*object).inner.on_updating(entity, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IMeshComponentVirtualTable_CCW_FOR_MeshComponent:
                radiance::interfaces::IMeshComponentVirtualTableCcw =
                radiance::interfaces::IMeshComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::interfaces::IMeshComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = MeshComponentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IMeshComponent: radiance::interfaces::IMeshComponent {
                            vtable: &GLOBAL_IMeshComponentVirtualTable_CCW_FOR_MeshComponent.vtable
                                as *const radiance::interfaces::IMeshComponentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }
            }
        }
    };
}

pub use ComObject_MeshComponent;
