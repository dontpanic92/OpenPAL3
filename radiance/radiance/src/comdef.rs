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
    pub on_loading: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub on_updating: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> (),
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
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IComponent::INTERFACE_ID)
    }
}

pub trait IComponentImpl {
    fn on_loading(&self) -> ();
    fn on_updating(&self, delta_sec: f32) -> ();
}

impl crosscom::ComInterface for IComponent {
    // 03748ce3-689d-4325-b1de-59de516b576b
    const INTERFACE_ID: [u8; 16] = [
        3u8, 116u8, 140u8, 227u8, 104u8, 157u8, 67u8, 37u8, 177u8, 222u8, 89u8, 222u8, 81u8, 107u8,
        87u8, 107u8,
    ];
}

// Interface IComponentContainer

#[repr(C)]
#[allow(non_snake_case)]
pub struct IComponentContainerVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
        component: *const *const std::os::raw::c_void,
    ) -> (),
    pub get_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub remove_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IComponentContainerVirtualTableCcw {
    pub offset: isize,
    pub vtable: IComponentContainerVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IComponentContainer {
    pub vtable: *const IComponentContainerVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IComponentContainer {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
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
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn add_component(
        &self,
        uuid: uuid::Uuid,
        component: crosscom::ComRc<radiance::comdef::IComponent>,
    ) -> () {
        unsafe {
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_component)(this, uuid.into(), component.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IComponentContainer::INTERFACE_ID)
    }
}

pub trait IComponentContainerImpl {
    fn add_component(
        &self,
        uuid: uuid::Uuid,
        component: crosscom::ComRc<radiance::comdef::IComponent>,
    ) -> ();
    fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>>;
    fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>>;
}

impl crosscom::ComInterface for IComponentContainer {
    // b875bf54-8c4c-4926-a2bd-6ad6f7038cfe
    const INTERFACE_ID: [u8; 16] = [
        184u8, 117u8, 191u8, 84u8, 140u8, 76u8, 73u8, 38u8, 162u8, 189u8, 106u8, 214u8, 247u8, 3u8,
        140u8, 254u8,
    ];
}

// Interface IApplication

#[repr(C)]
#[allow(non_snake_case)]
pub struct IApplicationVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
        component: *const *const std::os::raw::c_void,
    ) -> (),
    pub get_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub remove_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub initialize: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub run: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub set_title: fn(this: *const *const std::os::raw::c_void, title: &str) -> crosscom::Void,
    pub engine: fn(
        this: *const *const std::os::raw::c_void,
    ) -> std::rc::Rc<std::cell::RefCell<radiance::radiance::CoreRadianceEngine>>,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IApplicationVirtualTableCcw {
    pub offset: isize,
    pub vtable: IApplicationVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IApplication {
    pub vtable: *const IApplicationVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IApplication {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IApplication as *const *const std::os::raw::c_void;
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
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn add_component(
        &self,
        uuid: uuid::Uuid,
        component: crosscom::ComRc<radiance::comdef::IComponent>,
    ) -> () {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_component)(this, uuid.into(), component.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn initialize(&self) -> () {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).initialize)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn run(&self) -> () {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).run)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn set_title(&self, title: &str) -> crosscom::Void {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_title)(this, title.into());

            ret
        }
    }

    pub fn engine(
        &self,
    ) -> std::rc::Rc<std::cell::RefCell<radiance::radiance::CoreRadianceEngine>> {
        unsafe {
            let this = self as *const IApplication as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).engine)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IApplication::INTERFACE_ID)
    }
}

pub trait IApplicationImpl {
    fn initialize(&self) -> ();
    fn run(&self) -> ();
    fn set_title(&self, title: &str) -> crosscom::Void;
    fn engine(&self) -> std::rc::Rc<std::cell::RefCell<radiance::radiance::CoreRadianceEngine>>;
}

impl crosscom::ComInterface for IApplication {
    // fd2f7f28-c3ea-442c-a6dc-18e370de001a
    const INTERFACE_ID: [u8; 16] = [
        253u8, 47u8, 127u8, 40u8, 195u8, 234u8, 68u8, 44u8, 166u8, 220u8, 24u8, 227u8, 112u8,
        222u8, 0u8, 26u8,
    ];
}

// Class Application

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Application {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod Application_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct ApplicationCcw {
                IApplication: radiance::comdef::IApplication,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ApplicationCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IComponentContainer::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IApplication::INTERFACE_ID => {
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
                let object = crosscom::get_object::<ApplicationCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<ApplicationCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut ApplicationCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn initialize(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                (*__crosscom_object).inner.initialize().into()
            }

            unsafe extern "system" fn run(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                (*__crosscom_object).inner.run().into()
            }

            fn set_title(this: *const *const std::os::raw::c_void, title: &str) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                    (*__crosscom_object).inner.set_title(title)
                }
            }

            fn engine(
                this: *const *const std::os::raw::c_void,
            ) -> std::rc::Rc<std::cell::RefCell<radiance::radiance::CoreRadianceEngine>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                    (*__crosscom_object).inner.engine()
                }
            }

            unsafe extern "system" fn add_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
                component: *const *const std::os::raw::c_void,
            ) -> () {
                let uuid: uuid::Uuid = uuid.into();
                let component: crosscom::ComRc<radiance::comdef::IComponent> = component.into();

                let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                (*__crosscom_object)
                    .inner
                    .add_component(uuid.into(), component.into())
                    .into()
            }

            unsafe extern "system" fn get_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
            ) -> crosscom::RawPointer {
                let uuid: uuid::Uuid = uuid.into();

                let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                (*__crosscom_object).inner.get_component(uuid.into()).into()
            }

            unsafe extern "system" fn remove_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
            ) -> crosscom::RawPointer {
                let uuid: uuid::Uuid = uuid.into();

                let __crosscom_object = crosscom::get_object::<ApplicationCcw>(this);
                (*__crosscom_object)
                    .inner
                    .remove_component(uuid.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IApplicationVirtualTable_CCW_FOR_Application:
                radiance::comdef::IApplicationVirtualTableCcw =
                radiance::comdef::IApplicationVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::IApplicationVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        add_component,
                        get_component,
                        remove_component,
                        initialize,
                        run,
                        set_title,
                        engine,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = ApplicationCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IApplication: radiance::comdef::IApplication {
                            vtable: &GLOBAL_IApplicationVirtualTable_CCW_FOR_Application.vtable
                                as *const radiance::comdef::IApplicationVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this =
                            this.offset(-(crosscom::offset_of!(ApplicationCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_Application;

// Interface IApplicationLoaderComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IApplicationLoaderComponentVirtualTable {
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
pub struct IApplicationLoaderComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IApplicationLoaderComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IApplicationLoaderComponent {
    pub vtable: *const IApplicationLoaderComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IApplicationLoaderComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IApplicationLoaderComponent as *const *const std::os::raw::c_void;
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
            let this =
                self as *const IApplicationLoaderComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this =
                self as *const IApplicationLoaderComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this =
                self as *const IApplicationLoaderComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this =
                self as *const IApplicationLoaderComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IApplicationLoaderComponent::INTERFACE_ID)
    }
}

pub trait IApplicationLoaderComponentImpl {}

impl crosscom::ComInterface for IApplicationLoaderComponent {
    // 3afe8052-b675-4939-aafb-2a4fca8f2cf2
    const INTERFACE_ID: [u8; 16] = [
        58u8, 254u8, 128u8, 82u8, 182u8, 117u8, 73u8, 57u8, 170u8, 251u8, 42u8, 79u8, 202u8, 143u8,
        44u8, 242u8,
    ];
}

// Interface IScene

#[repr(C)]
#[allow(non_snake_case)]
pub struct ISceneVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
        component: *const *const std::os::raw::c_void,
    ) -> (),
    pub get_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub remove_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub load: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub visible:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
    pub update: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> (),
    pub draw_ui: fn(this: *const *const std::os::raw::c_void, ui: &mut imgui::Ui) -> crosscom::Void,
    pub unload: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub add_entity: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        entity: *const *const std::os::raw::c_void,
    ) -> (),
    pub entities: fn(
        this: *const *const std::os::raw::c_void,
    ) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>>,
    pub root_entities: fn(
        this: *const *const std::os::raw::c_void,
    ) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>>,
    pub camera: fn(
        this: *const *const std::os::raw::c_void,
    ) -> std::rc::Rc<std::cell::RefCell<radiance::scene::Camera>>,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ISceneVirtualTableCcw {
    pub offset: isize,
    pub vtable: ISceneVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IScene {
    pub vtable: *const ISceneVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IScene {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IScene as *const *const std::os::raw::c_void;
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
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn add_component(
        &self,
        uuid: uuid::Uuid,
        component: crosscom::ComRc<radiance::comdef::IComponent>,
    ) -> () {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_component)(this, uuid.into(), component.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn load(&self) -> () {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).load)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn visible(&self) -> bool {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).visible)(this);
            let ret: bool = ret != 0;

            ret
        }
    }

    pub fn update(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).update)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn draw_ui(&self, ui: &mut imgui::Ui) -> crosscom::Void {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).draw_ui)(this, ui.into());

            ret
        }
    }

    pub fn unload(&self) -> () {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).unload)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn add_entity(&self, entity: crosscom::ComRc<radiance::comdef::IEntity>) -> () {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_entity)(this, entity.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn entities(&self) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).entities)(this);

            ret
        }
    }

    pub fn root_entities(&self) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).root_entities)(this);

            ret
        }
    }

    pub fn camera(&self) -> std::rc::Rc<std::cell::RefCell<radiance::scene::Camera>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).camera)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IScene::INTERFACE_ID)
    }
}

pub trait ISceneImpl {
    fn load(&self) -> ();
    fn visible(&self) -> bool;
    fn update(&self, delta_sec: f32) -> ();
    fn draw_ui(&self, ui: &mut imgui::Ui) -> crosscom::Void;
    fn unload(&self) -> ();
    fn add_entity(&self, entity: crosscom::ComRc<radiance::comdef::IEntity>) -> ();
    fn entities(&self) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>>;
    fn root_entities(&self) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>>;
    fn camera(&self) -> std::rc::Rc<std::cell::RefCell<radiance::scene::Camera>>;
}

impl crosscom::ComInterface for IScene {
    // 27e705f1-d035-4e91-8735-3a006fab870d
    const INTERFACE_ID: [u8; 16] = [
        39u8, 231u8, 5u8, 241u8, 208u8, 53u8, 78u8, 145u8, 135u8, 53u8, 58u8, 0u8, 111u8, 171u8,
        135u8, 13u8,
    ];
}

// Class Scene

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Scene {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod Scene_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct SceneCcw {
                IScene: radiance::comdef::IScene,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<SceneCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IComponentContainer::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IScene::INTERFACE_ID => {
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
                let object = crosscom::get_object::<SceneCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<SceneCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut SceneCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn load(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.load().into()
            }

            unsafe extern "system" fn visible(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_int {
                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.visible().into()
            }

            unsafe extern "system" fn update(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.update(delta_sec.into()).into()
            }

            fn draw_ui(
                this: *const *const std::os::raw::c_void,
                ui: &mut imgui::Ui,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                    (*__crosscom_object).inner.draw_ui(ui)
                }
            }

            unsafe extern "system" fn unload(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.unload().into()
            }

            unsafe extern "system" fn add_entity(
                this: *const *const std::os::raw::c_void,
                entity: *const *const std::os::raw::c_void,
            ) -> () {
                let entity: crosscom::ComRc<radiance::comdef::IEntity> = entity.into();

                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.add_entity(entity.into()).into()
            }

            fn entities(
                this: *const *const std::os::raw::c_void,
            ) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                    (*__crosscom_object).inner.entities()
                }
            }

            fn root_entities(
                this: *const *const std::os::raw::c_void,
            ) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                    (*__crosscom_object).inner.root_entities()
                }
            }

            fn camera(
                this: *const *const std::os::raw::c_void,
            ) -> std::rc::Rc<std::cell::RefCell<radiance::scene::Camera>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                    (*__crosscom_object).inner.camera()
                }
            }

            unsafe extern "system" fn add_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
                component: *const *const std::os::raw::c_void,
            ) -> () {
                let uuid: uuid::Uuid = uuid.into();
                let component: crosscom::ComRc<radiance::comdef::IComponent> = component.into();

                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object)
                    .inner
                    .add_component(uuid.into(), component.into())
                    .into()
            }

            unsafe extern "system" fn get_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
            ) -> crosscom::RawPointer {
                let uuid: uuid::Uuid = uuid.into();

                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.get_component(uuid.into()).into()
            }

            unsafe extern "system" fn remove_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
            ) -> crosscom::RawPointer {
                let uuid: uuid::Uuid = uuid.into();

                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object)
                    .inner
                    .remove_component(uuid.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ISceneVirtualTable_CCW_FOR_Scene:
                radiance::comdef::ISceneVirtualTableCcw = radiance::comdef::ISceneVirtualTableCcw {
                offset: 0,
                vtable: radiance::comdef::ISceneVirtualTable {
                    query_interface,
                    add_ref,
                    release,
                    add_component,
                    get_component,
                    remove_component,
                    load,
                    visible,
                    update,
                    draw_ui,
                    unload,
                    add_entity,
                    entities,
                    root_entities,
                    camera,
                },
            };

            impl crosscom::ComObject for $impl_type {
                type CcwType = SceneCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IScene: radiance::comdef::IScene {
                            vtable: &GLOBAL_ISceneVirtualTable_CCW_FOR_Scene.vtable
                                as *const radiance::comdef::ISceneVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(-(crosscom::offset_of!(SceneCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_Scene;

// Interface IEntity

#[repr(C)]
#[allow(non_snake_case)]
pub struct IEntityVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
        component: *const *const std::os::raw::c_void,
    ) -> (),
    pub get_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub remove_component: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        uuid: uuid::Uuid,
    ) -> crosscom::RawPointer,
    pub name: fn(this: *const *const std::os::raw::c_void) -> String,
    pub set_name: fn(this: *const *const std::os::raw::c_void, name: &str) -> crosscom::Void,
    pub load: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub unload: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub update: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        delta_sec: std::os::raw::c_float,
    ) -> (),
    pub transform: fn(
        this: *const *const std::os::raw::c_void,
    ) -> std::rc::Rc<std::cell::RefCell<radiance::math::Transform>>,
    pub world_transform: fn(this: *const *const std::os::raw::c_void) -> radiance::math::Transform,
    pub update_world_transform: fn(
        this: *const *const std::os::raw::c_void,
        parent_transform: &radiance::math::Transform,
    ) -> crosscom::Void,
    pub children: fn(
        this: *const *const std::os::raw::c_void,
    ) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>>,
    pub visible:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
    pub set_visible: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        visible: std::os::raw::c_int,
    ) -> (),
    pub get_rendering_component: fn(
        this: *const *const std::os::raw::c_void,
    )
        -> Option<std::rc::Rc<radiance::rendering::RenderingComponent>>,
    pub set_rendering_component: fn(
        this: *const *const std::os::raw::c_void,
        component: Option<std::rc::Rc<radiance::rendering::RenderingComponent>>,
    ) -> crosscom::Void,
    pub attach: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        child: *const *const std::os::raw::c_void,
    ) -> (),
}

#[repr(C)]
#[allow(dead_code)]
pub struct IEntityVirtualTableCcw {
    pub offset: isize,
    pub vtable: IEntityVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IEntity {
    pub vtable: *const IEntityVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IEntity {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IEntity as *const *const std::os::raw::c_void;
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
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn add_component(
        &self,
        uuid: uuid::Uuid,
        component: crosscom::ComRc<radiance::comdef::IComponent>,
    ) -> () {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_component)(this, uuid.into(), component.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::comdef::IComponent>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::comdef::IComponent>> = ret.into();

            ret
        }
    }

    pub fn name(&self) -> String {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).name)(this);

            ret
        }
    }

    pub fn set_name(&self, name: &str) -> crosscom::Void {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_name)(this, name.into());

            ret
        }
    }

    pub fn load(&self) -> () {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).load)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn unload(&self) -> () {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).unload)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn update(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).update)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn transform(&self) -> std::rc::Rc<std::cell::RefCell<radiance::math::Transform>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).transform)(this);

            ret
        }
    }

    pub fn world_transform(&self) -> radiance::math::Transform {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).world_transform)(this);

            ret
        }
    }

    pub fn update_world_transform(
        &self,
        parent_transform: &radiance::math::Transform,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).update_world_transform)(this, parent_transform.into());

            ret
        }
    }

    pub fn children(&self) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).children)(this);

            ret
        }
    }

    pub fn visible(&self) -> bool {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).visible)(this);
            let ret: bool = ret != 0;

            ret
        }
    }

    pub fn set_visible(&self, visible: bool) -> () {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_visible)(this, visible.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn get_rendering_component(
        &self,
    ) -> Option<std::rc::Rc<radiance::rendering::RenderingComponent>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_rendering_component)(this);

            ret
        }
    }

    pub fn set_rendering_component(
        &self,
        component: Option<std::rc::Rc<radiance::rendering::RenderingComponent>>,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_rendering_component)(this, component.into());

            ret
        }
    }

    pub fn attach(&self, child: crosscom::ComRc<radiance::comdef::IEntity>) -> () {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).attach)(this, child.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IEntity::INTERFACE_ID)
    }
}

pub trait IEntityImpl {
    fn name(&self) -> String;
    fn set_name(&self, name: &str) -> crosscom::Void;
    fn load(&self) -> ();
    fn unload(&self) -> ();
    fn update(&self, delta_sec: f32) -> ();
    fn transform(&self) -> std::rc::Rc<std::cell::RefCell<radiance::math::Transform>>;
    fn world_transform(&self) -> radiance::math::Transform;
    fn update_world_transform(
        &self,
        parent_transform: &radiance::math::Transform,
    ) -> crosscom::Void;
    fn children(&self) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>>;
    fn visible(&self) -> bool;
    fn set_visible(&self, visible: bool) -> ();
    fn get_rendering_component(
        &self,
    ) -> Option<std::rc::Rc<radiance::rendering::RenderingComponent>>;
    fn set_rendering_component(
        &self,
        component: Option<std::rc::Rc<radiance::rendering::RenderingComponent>>,
    ) -> crosscom::Void;
    fn attach(&self, child: crosscom::ComRc<radiance::comdef::IEntity>) -> ();
}

impl crosscom::ComInterface for IEntity {
    // 95099190-580e-439f-be36-8d1345cf4dec
    const INTERFACE_ID: [u8; 16] = [
        149u8, 9u8, 145u8, 144u8, 88u8, 14u8, 67u8, 159u8, 190u8, 54u8, 141u8, 19u8, 69u8, 207u8,
        77u8, 236u8,
    ];
}

// Class Entity

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Entity {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod Entity_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct EntityCcw {
                IEntity: radiance::comdef::IEntity,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<EntityCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IComponentContainer::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::IEntity::INTERFACE_ID => {
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
                let object = crosscom::get_object::<EntityCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<EntityCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut EntityCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn name(this: *const *const std::os::raw::c_void) -> String {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object).inner.name()
                }
            }

            fn set_name(this: *const *const std::os::raw::c_void, name: &str) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object).inner.set_name(name)
                }
            }

            unsafe extern "system" fn load(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.load().into()
            }

            unsafe extern "system" fn unload(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.unload().into()
            }

            unsafe extern "system" fn update(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.update(delta_sec.into()).into()
            }

            fn transform(
                this: *const *const std::os::raw::c_void,
            ) -> std::rc::Rc<std::cell::RefCell<radiance::math::Transform>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object).inner.transform()
                }
            }

            fn world_transform(
                this: *const *const std::os::raw::c_void,
            ) -> radiance::math::Transform {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object).inner.world_transform()
                }
            }

            fn update_world_transform(
                this: *const *const std::os::raw::c_void,
                parent_transform: &radiance::math::Transform,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .update_world_transform(parent_transform)
                }
            }

            fn children(
                this: *const *const std::os::raw::c_void,
            ) -> Vec<crosscom::ComRc<radiance::comdef::IEntity>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object).inner.children()
                }
            }

            unsafe extern "system" fn visible(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_int {
                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.visible().into()
            }

            unsafe extern "system" fn set_visible(
                this: *const *const std::os::raw::c_void,
                visible: std::os::raw::c_int,
            ) -> () {
                let visible: bool = visible != 0;

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object)
                    .inner
                    .set_visible(visible.into())
                    .into()
            }

            fn get_rendering_component(
                this: *const *const std::os::raw::c_void,
            ) -> Option<std::rc::Rc<radiance::rendering::RenderingComponent>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object).inner.get_rendering_component()
                }
            }

            fn set_rendering_component(
                this: *const *const std::os::raw::c_void,
                component: Option<std::rc::Rc<radiance::rendering::RenderingComponent>>,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .set_rendering_component(component)
                }
            }

            unsafe extern "system" fn attach(
                this: *const *const std::os::raw::c_void,
                child: *const *const std::os::raw::c_void,
            ) -> () {
                let child: crosscom::ComRc<radiance::comdef::IEntity> = child.into();

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.attach(child.into()).into()
            }

            unsafe extern "system" fn add_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
                component: *const *const std::os::raw::c_void,
            ) -> () {
                let uuid: uuid::Uuid = uuid.into();
                let component: crosscom::ComRc<radiance::comdef::IComponent> = component.into();

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object)
                    .inner
                    .add_component(uuid.into(), component.into())
                    .into()
            }

            unsafe extern "system" fn get_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
            ) -> crosscom::RawPointer {
                let uuid: uuid::Uuid = uuid.into();

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.get_component(uuid.into()).into()
            }

            unsafe extern "system" fn remove_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
            ) -> crosscom::RawPointer {
                let uuid: uuid::Uuid = uuid.into();

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object)
                    .inner
                    .remove_component(uuid.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IEntityVirtualTable_CCW_FOR_Entity:
                radiance::comdef::IEntityVirtualTableCcw =
                radiance::comdef::IEntityVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::IEntityVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        add_component,
                        get_component,
                        remove_component,
                        name,
                        set_name,
                        load,
                        unload,
                        update,
                        transform,
                        world_transform,
                        update_world_transform,
                        children,
                        visible,
                        set_visible,
                        get_rendering_component,
                        set_rendering_component,
                        attach,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = EntityCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IEntity: radiance::comdef::IEntity {
                            vtable: &GLOBAL_IEntityVirtualTable_CCW_FOR_Entity.vtable
                                as *const radiance::comdef::IEntityVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(-(crosscom::offset_of!(EntityCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_Entity;

// Interface IStaticMeshComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IStaticMeshComponentVirtualTable {
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
pub struct IStaticMeshComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IStaticMeshComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IStaticMeshComponent {
    pub vtable: *const IStaticMeshComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IStaticMeshComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IStaticMeshComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const IStaticMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IStaticMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IStaticMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IStaticMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IStaticMeshComponent::INTERFACE_ID)
    }
}

pub trait IStaticMeshComponentImpl {}

impl crosscom::ComInterface for IStaticMeshComponent {
    // 8dd91852-476b-401b-8668-ba9cc331b7a1
    const INTERFACE_ID: [u8; 16] = [
        141u8, 217u8, 24u8, 82u8, 71u8, 107u8, 64u8, 27u8, 134u8, 104u8, 186u8, 156u8, 195u8, 49u8,
        183u8, 161u8,
    ];
}

// Class StaticMeshComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_StaticMeshComponent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod StaticMeshComponent_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct StaticMeshComponentCcw {
                IStaticMeshComponent: radiance::comdef::IStaticMeshComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<StaticMeshComponentCcw>(this);
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

                    &radiance::comdef::IStaticMeshComponent::INTERFACE_ID => {
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
                let object = crosscom::get_object::<StaticMeshComponentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<StaticMeshComponentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut StaticMeshComponentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<StaticMeshComponentCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<StaticMeshComponentCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IStaticMeshComponentVirtualTable_CCW_FOR_StaticMeshComponent:
                radiance::comdef::IStaticMeshComponentVirtualTableCcw =
                radiance::comdef::IStaticMeshComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::IStaticMeshComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = StaticMeshComponentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IStaticMeshComponent: radiance::comdef::IStaticMeshComponent {
                            vtable:
                                &GLOBAL_IStaticMeshComponentVirtualTable_CCW_FOR_StaticMeshComponent
                                    .vtable
                                    as *const radiance::comdef::IStaticMeshComponentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(
                            -(crosscom::offset_of!(StaticMeshComponentCcw, inner) as isize),
                        );
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_StaticMeshComponent;

// Interface IAnimatedMeshComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IAnimatedMeshComponentVirtualTable {
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
    pub morph_animation_state: fn(
        this: *const *const std::os::raw::c_void,
    ) -> radiance::components::mesh::MorphAnimationState,
    pub replay: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
}

#[repr(C)]
#[allow(dead_code)]
pub struct IAnimatedMeshComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IAnimatedMeshComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IAnimatedMeshComponent {
    pub vtable: *const IAnimatedMeshComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IAnimatedMeshComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn morph_animation_state(&self) -> radiance::components::mesh::MorphAnimationState {
        unsafe {
            let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).morph_animation_state)(this);

            ret
        }
    }

    pub fn replay(&self) -> () {
        unsafe {
            let this = self as *const IAnimatedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).replay)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IAnimatedMeshComponent::INTERFACE_ID)
    }
}

pub trait IAnimatedMeshComponentImpl {
    fn morph_animation_state(&self) -> radiance::components::mesh::MorphAnimationState;
    fn replay(&self) -> ();
}

impl crosscom::ComInterface for IAnimatedMeshComponent {
    // 5c56adbc-bc22-4275-b99a-09973a3ffff0
    const INTERFACE_ID: [u8; 16] = [
        92u8, 86u8, 173u8, 188u8, 188u8, 34u8, 66u8, 117u8, 185u8, 154u8, 9u8, 151u8, 58u8, 63u8,
        255u8, 240u8,
    ];
}

// Class AnimatedMeshComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_AnimatedMeshComponent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod AnimatedMeshComponent_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct AnimatedMeshComponentCcw {
                IAnimatedMeshComponent: radiance::comdef::IAnimatedMeshComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);
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

                    &radiance::comdef::IAnimatedMeshComponent::INTERFACE_ID => {
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
                let object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut AnimatedMeshComponentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn morph_animation_state(
                this: *const *const std::os::raw::c_void,
            ) -> radiance::components::mesh::MorphAnimationState {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);
                    (*__crosscom_object).inner.morph_animation_state()
                }
            }

            unsafe extern "system" fn replay(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);
                (*__crosscom_object).inner.replay().into()
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<AnimatedMeshComponentCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IAnimatedMeshComponentVirtualTable_CCW_FOR_AnimatedMeshComponent:
                radiance::comdef::IAnimatedMeshComponentVirtualTableCcw =
                radiance::comdef::IAnimatedMeshComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::IAnimatedMeshComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                        morph_animation_state,
                        replay,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = AnimatedMeshComponentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {

        IAnimatedMeshComponent: radiance::comdef::IAnimatedMeshComponent {
            vtable: &GLOBAL_IAnimatedMeshComponentVirtualTable_CCW_FOR_AnimatedMeshComponent.vtable
                as *const radiance::comdef::IAnimatedMeshComponentVirtualTable,
        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(
                            -(crosscom::offset_of!(AnimatedMeshComponentCcw, inner) as isize),
                        );
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_AnimatedMeshComponent;

// Interface IDirector

#[repr(C)]
#[allow(non_snake_case)]
pub struct IDirectorVirtualTable {
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
}

#[repr(C)]
#[allow(dead_code)]
pub struct IDirectorVirtualTableCcw {
    pub offset: isize,
    pub vtable: IDirectorVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IDirector {
    pub vtable: *const IDirectorVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IDirector {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IDirector as *const *const std::os::raw::c_void;
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
            let this = self as *const IDirector as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IDirector as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn activate(&self, scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>) -> () {
        unsafe {
            let this = self as *const IDirector as *const *const std::os::raw::c_void;
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
            let this = self as *const IDirector as *const *const std::os::raw::c_void;
            let ret =
                ((*self.vtable).update)(this, scene_manager.into(), ui.into(), delta_sec.into());

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IDirector::INTERFACE_ID)
    }
}

pub trait IDirectorImpl {
    fn activate(&self, scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>) -> ();
    fn update(
        &self,
        scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<crosscom::ComRc<radiance::comdef::IDirector>>;
}

impl crosscom::ComInterface for IDirector {
    // 6dedae32-8339-482e-9f66-c30d557cacb4
    const INTERFACE_ID: [u8; 16] = [
        109u8, 237u8, 174u8, 50u8, 131u8, 57u8, 72u8, 46u8, 159u8, 102u8, 195u8, 13u8, 85u8, 124u8,
        172u8, 180u8,
    ];
}

// Interface ISceneManager

#[repr(C)]
#[allow(non_snake_case)]
pub struct ISceneManagerVirtualTable {
    pub query_interface: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long,
    pub add_ref:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub update: fn(
        this: *const *const std::os::raw::c_void,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> crosscom::Void,
    pub scene:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> crosscom::RawPointer,
    pub scenes: fn(
        this: *const *const std::os::raw::c_void,
    ) -> Vec<crosscom::ComRc<radiance::comdef::IScene>>,
    pub director:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> crosscom::RawPointer,
    pub set_director: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        director: *const *const std::os::raw::c_void,
    ) -> (),
    pub push_scene: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        scene: *const *const std::os::raw::c_void,
    ) -> (),
    pub pop_scene:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> crosscom::RawPointer,
    pub unload_all_scenes:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub unset_director: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
}

#[repr(C)]
#[allow(dead_code)]
pub struct ISceneManagerVirtualTableCcw {
    pub offset: isize,
    pub vtable: ISceneManagerVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ISceneManager {
    pub vtable: *const ISceneManagerVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ISceneManager {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
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
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn update(&self, ui: &imgui::Ui, delta_sec: f32) -> crosscom::Void {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).update)(this, ui.into(), delta_sec.into());

            ret
        }
    }

    pub fn scene(&self) -> Option<crosscom::ComRc<radiance::comdef::IScene>> {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).scene)(this);
            let ret: Option<crosscom::ComRc<radiance::comdef::IScene>> = ret.into();

            ret
        }
    }

    pub fn scenes(&self) -> Vec<crosscom::ComRc<radiance::comdef::IScene>> {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).scenes)(this);

            ret
        }
    }

    pub fn director(&self) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).director)(this);
            let ret: Option<crosscom::ComRc<radiance::comdef::IDirector>> = ret.into();

            ret
        }
    }

    pub fn set_director(&self, director: crosscom::ComRc<radiance::comdef::IDirector>) -> () {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_director)(this, director.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn push_scene(&self, scene: crosscom::ComRc<radiance::comdef::IScene>) -> () {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).push_scene)(this, scene.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn pop_scene(&self) -> Option<crosscom::ComRc<radiance::comdef::IScene>> {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).pop_scene)(this);
            let ret: Option<crosscom::ComRc<radiance::comdef::IScene>> = ret.into();

            ret
        }
    }

    pub fn unload_all_scenes(&self) -> () {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).unload_all_scenes)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn unset_director(&self) -> () {
        unsafe {
            let this = self as *const ISceneManager as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).unset_director)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ISceneManager::INTERFACE_ID)
    }
}

pub trait ISceneManagerImpl {
    fn update(&self, ui: &imgui::Ui, delta_sec: f32) -> crosscom::Void;
    fn scene(&self) -> Option<crosscom::ComRc<radiance::comdef::IScene>>;
    fn scenes(&self) -> Vec<crosscom::ComRc<radiance::comdef::IScene>>;
    fn director(&self) -> Option<crosscom::ComRc<radiance::comdef::IDirector>>;
    fn set_director(&self, director: crosscom::ComRc<radiance::comdef::IDirector>) -> ();
    fn push_scene(&self, scene: crosscom::ComRc<radiance::comdef::IScene>) -> ();
    fn pop_scene(&self) -> Option<crosscom::ComRc<radiance::comdef::IScene>>;
    fn unload_all_scenes(&self) -> ();
    fn unset_director(&self) -> ();
}

impl crosscom::ComInterface for ISceneManager {
    // a12c44d5-f5bc-4268-bd00-ab3b6270a829
    const INTERFACE_ID: [u8; 16] = [
        161u8, 44u8, 68u8, 213u8, 245u8, 188u8, 66u8, 104u8, 189u8, 0u8, 171u8, 59u8, 98u8, 112u8,
        168u8, 41u8,
    ];
}

// Class SceneManager

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_SceneManager {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod SceneManager_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct SceneManagerCcw {
                ISceneManager: radiance::comdef::ISceneManager,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<SceneManagerCcw>(this);
                match guid.as_bytes() {
                    &crosscom::IUnknown::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as std::os::raw::c_long
                    }

                    &radiance::comdef::ISceneManager::INTERFACE_ID => {
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
                let object = crosscom::get_object::<SceneManagerCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<SceneManagerCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut SceneManagerCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn update(
                this: *const *const std::os::raw::c_void,
                ui: &imgui::Ui,
                delta_sec: f32,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                    (*__crosscom_object).inner.update(ui, delta_sec)
                }
            }

            unsafe extern "system" fn scene(
                this: *const *const std::os::raw::c_void,
            ) -> crosscom::RawPointer {
                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object).inner.scene().into()
            }

            fn scenes(
                this: *const *const std::os::raw::c_void,
            ) -> Vec<crosscom::ComRc<radiance::comdef::IScene>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                    (*__crosscom_object).inner.scenes()
                }
            }

            unsafe extern "system" fn director(
                this: *const *const std::os::raw::c_void,
            ) -> crosscom::RawPointer {
                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object).inner.director().into()
            }

            unsafe extern "system" fn set_director(
                this: *const *const std::os::raw::c_void,
                director: *const *const std::os::raw::c_void,
            ) -> () {
                let director: crosscom::ComRc<radiance::comdef::IDirector> = director.into();

                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object)
                    .inner
                    .set_director(director.into())
                    .into()
            }

            unsafe extern "system" fn push_scene(
                this: *const *const std::os::raw::c_void,
                scene: *const *const std::os::raw::c_void,
            ) -> () {
                let scene: crosscom::ComRc<radiance::comdef::IScene> = scene.into();

                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object).inner.push_scene(scene.into()).into()
            }

            unsafe extern "system" fn pop_scene(
                this: *const *const std::os::raw::c_void,
            ) -> crosscom::RawPointer {
                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object).inner.pop_scene().into()
            }

            unsafe extern "system" fn unload_all_scenes(
                this: *const *const std::os::raw::c_void,
            ) -> () {
                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object).inner.unload_all_scenes().into()
            }

            unsafe extern "system" fn unset_director(
                this: *const *const std::os::raw::c_void,
            ) -> () {
                let __crosscom_object = crosscom::get_object::<SceneManagerCcw>(this);
                (*__crosscom_object).inner.unset_director().into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ISceneManagerVirtualTable_CCW_FOR_SceneManager:
                radiance::comdef::ISceneManagerVirtualTableCcw =
                radiance::comdef::ISceneManagerVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::ISceneManagerVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        update,
                        scene,
                        scenes,
                        director,
                        set_director,
                        push_scene,
                        pop_scene,
                        unload_all_scenes,
                        unset_director,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = SceneManagerCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        ISceneManager: radiance::comdef::ISceneManager {
                            vtable: &GLOBAL_ISceneManagerVirtualTable_CCW_FOR_SceneManager.vtable
                                as *const radiance::comdef::ISceneManagerVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this =
                            this.offset(-(crosscom::offset_of!(SceneManagerCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_SceneManager;

// Interface ISkinnedMeshComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct ISkinnedMeshComponentVirtualTable {
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
pub struct ISkinnedMeshComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: ISkinnedMeshComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct ISkinnedMeshComponent {
    pub vtable: *const ISkinnedMeshComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl ISkinnedMeshComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const ISkinnedMeshComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const ISkinnedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const ISkinnedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const ISkinnedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const ISkinnedMeshComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(ISkinnedMeshComponent::INTERFACE_ID)
    }
}

pub trait ISkinnedMeshComponentImpl {}

impl crosscom::ComInterface for ISkinnedMeshComponent {
    // 19ff0435-8a22-486c-b16a-69c2e1ffd0ae
    const INTERFACE_ID: [u8; 16] = [
        25u8, 255u8, 4u8, 53u8, 138u8, 34u8, 72u8, 108u8, 177u8, 106u8, 105u8, 194u8, 225u8, 255u8,
        208u8, 174u8,
    ];
}

// Class SkinnedMeshComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_SkinnedMeshComponent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod SkinnedMeshComponent_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct SkinnedMeshComponentCcw {
                ISkinnedMeshComponent: radiance::comdef::ISkinnedMeshComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<SkinnedMeshComponentCcw>(this);
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

                    &radiance::comdef::ISkinnedMeshComponent::INTERFACE_ID => {
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
                let object = crosscom::get_object::<SkinnedMeshComponentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<SkinnedMeshComponentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut SkinnedMeshComponentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<SkinnedMeshComponentCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<SkinnedMeshComponentCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_ISkinnedMeshComponentVirtualTable_CCW_FOR_SkinnedMeshComponent:
                radiance::comdef::ISkinnedMeshComponentVirtualTableCcw =
                radiance::comdef::ISkinnedMeshComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::ISkinnedMeshComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = SkinnedMeshComponentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {

        ISkinnedMeshComponent: radiance::comdef::ISkinnedMeshComponent {
            vtable: &GLOBAL_ISkinnedMeshComponentVirtualTable_CCW_FOR_SkinnedMeshComponent.vtable
                as *const radiance::comdef::ISkinnedMeshComponentVirtualTable,
        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(
                            -(crosscom::offset_of!(SkinnedMeshComponentCcw, inner) as isize),
                        );
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_SkinnedMeshComponent;

// Interface IHAnimBoneComponent

#[repr(C)]
#[allow(non_snake_case)]
pub struct IHAnimBoneComponentVirtualTable {
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
    pub set_keyframes: fn(
        this: *const *const std::os::raw::c_void,
        keyframes: Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>,
    ) -> crosscom::Void,
    pub set_bond_pose: fn(
        this: *const *const std::os::raw::c_void,
        matrix: radiance::math::Mat44,
    ) -> crosscom::Void,
    pub bond_pose: fn(this: *const *const std::os::raw::c_void) -> radiance::math::Mat44,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IHAnimBoneComponentVirtualTableCcw {
    pub offset: isize,
    pub vtable: IHAnimBoneComponentVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IHAnimBoneComponent {
    pub vtable: *const IHAnimBoneComponentVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IHAnimBoneComponent {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
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
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn set_keyframes(
        &self,
        keyframes: Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>,
    ) -> crosscom::Void {
        unsafe {
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_keyframes)(this, keyframes.into());

            ret
        }
    }

    pub fn set_bond_pose(&self, matrix: radiance::math::Mat44) -> crosscom::Void {
        unsafe {
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_bond_pose)(this, matrix.into());

            ret
        }
    }

    pub fn bond_pose(&self) -> radiance::math::Mat44 {
        unsafe {
            let this = self as *const IHAnimBoneComponent as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).bond_pose)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IHAnimBoneComponent::INTERFACE_ID)
    }
}

pub trait IHAnimBoneComponentImpl {
    fn set_keyframes(
        &self,
        keyframes: Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>,
    ) -> crosscom::Void;
    fn set_bond_pose(&self, matrix: radiance::math::Mat44) -> crosscom::Void;
    fn bond_pose(&self) -> radiance::math::Mat44;
}

impl crosscom::ComInterface for IHAnimBoneComponent {
    // 1b4b89da-94cb-4dd8-a1e1-493763f14ee3
    const INTERFACE_ID: [u8; 16] = [
        27u8, 75u8, 137u8, 218u8, 148u8, 203u8, 77u8, 216u8, 161u8, 225u8, 73u8, 55u8, 99u8, 241u8,
        78u8, 227u8,
    ];
}

// Class HAnimBoneComponent

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_HAnimBoneComponent {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod HAnimBoneComponent_crosscom_impl {
            use crate as radiance;
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

            #[repr(C)]
            pub struct HAnimBoneComponentCcw {
                IHAnimBoneComponent: radiance::comdef::IHAnimBoneComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
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

                    &radiance::comdef::IHAnimBoneComponent::INTERFACE_ID => {
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
                let object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<HAnimBoneComponentCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut HAnimBoneComponentCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn set_keyframes(
                this: *const *const std::os::raw::c_void,
                keyframes: Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
                    (*__crosscom_object).inner.set_keyframes(keyframes)
                }
            }

            fn set_bond_pose(
                this: *const *const std::os::raw::c_void,
                matrix: radiance::math::Mat44,
            ) -> crosscom::Void {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
                    (*__crosscom_object).inner.set_bond_pose(matrix)
                }
            }

            fn bond_pose(this: *const *const std::os::raw::c_void) -> radiance::math::Mat44 {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
                    (*__crosscom_object).inner.bond_pose()
                }
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<HAnimBoneComponentCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IHAnimBoneComponentVirtualTable_CCW_FOR_HAnimBoneComponent:
                radiance::comdef::IHAnimBoneComponentVirtualTableCcw =
                radiance::comdef::IHAnimBoneComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::comdef::IHAnimBoneComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                        set_keyframes,
                        set_bond_pose,
                        bond_pose,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = HAnimBoneComponentCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IHAnimBoneComponent: radiance::comdef::IHAnimBoneComponent {
                            vtable:
                                &GLOBAL_IHAnimBoneComponentVirtualTable_CCW_FOR_HAnimBoneComponent
                                    .vtable
                                    as *const radiance::comdef::IHAnimBoneComponentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this
                            .offset(-(crosscom::offset_of!(HAnimBoneComponentCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_HAnimBoneComponent;
