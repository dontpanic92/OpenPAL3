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
