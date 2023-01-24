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
        component: crosscom::ComRc<radiance::interfaces::IComponent>,
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
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>> {
        unsafe {
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::interfaces::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>> {
        unsafe {
            let this = self as *const IComponentContainer as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::interfaces::IComponent>> = ret.into();

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
        component: crosscom::ComRc<radiance::interfaces::IComponent>,
    ) -> ();
    fn get_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>>;
    fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>>;
}

impl crosscom::ComInterface for IComponentContainer {
    // b875bf54-8c4c-4926-a2bd-6ad6f7038cfe
    const INTERFACE_ID: [u8; 16] = [
        184u8, 117u8, 191u8, 84u8, 140u8, 76u8, 73u8, 38u8, 162u8, 189u8, 106u8, 214u8, 247u8, 3u8,
        140u8, 254u8,
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
    ) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>>,
    pub root_entities: fn(
        this: *const *const std::os::raw::c_void,
    ) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>>,
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
        component: crosscom::ComRc<radiance::interfaces::IComponent>,
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
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::interfaces::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::interfaces::IComponent>> = ret.into();

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

    pub fn add_entity(&self, entity: crosscom::ComRc<radiance::interfaces::IEntity>) -> () {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_entity)(this, entity.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn entities(&self) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>> {
        unsafe {
            let this = self as *const IScene as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).entities)(this);

            ret
        }
    }

    pub fn root_entities(&self) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>> {
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
    fn add_entity(&self, entity: crosscom::ComRc<radiance::interfaces::IEntity>) -> ();
    fn entities(&self) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>>;
    fn root_entities(&self) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>>;
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
            use radiance::interfaces::IAnimatedMeshComponentImpl;
            use radiance::interfaces::IComponentContainerImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IEntityImpl;
            use radiance::interfaces::ISceneImpl;
            use radiance::interfaces::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct SceneCcw {
                IScene: radiance::interfaces::IScene,

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
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IComponentContainer::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IScene::INTERFACE_ID => {
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
                let entity: crosscom::ComRc<radiance::interfaces::IEntity> = entity.into();

                let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                (*__crosscom_object).inner.add_entity(entity.into()).into()
            }

            fn entities(
                this: *const *const std::os::raw::c_void,
            ) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>> {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<SceneCcw>(this);
                    (*__crosscom_object).inner.entities()
                }
            }

            fn root_entities(
                this: *const *const std::os::raw::c_void,
            ) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>> {
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
                let component: crosscom::ComRc<radiance::interfaces::IComponent> = component.into();

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
                radiance::interfaces::ISceneVirtualTableCcw =
                radiance::interfaces::ISceneVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::interfaces::ISceneVirtualTable {
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
                        IScene: radiance::interfaces::IScene {
                            vtable: &GLOBAL_ISceneVirtualTable_CCW_FOR_Scene.vtable
                                as *const radiance::interfaces::ISceneVirtualTable,
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
    ) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>>,
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
        component: crosscom::ComRc<radiance::interfaces::IComponent>,
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
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::interfaces::IComponent>> = ret.into();

            ret
        }
    }

    pub fn remove_component(
        &self,
        uuid: uuid::Uuid,
    ) -> Option<crosscom::ComRc<radiance::interfaces::IComponent>> {
        unsafe {
            let this = self as *const IEntity as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).remove_component)(this, uuid.into());
            let ret: Option<crosscom::ComRc<radiance::interfaces::IComponent>> = ret.into();

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

    pub fn children(&self) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>> {
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

    pub fn attach(&self, child: crosscom::ComRc<radiance::interfaces::IEntity>) -> () {
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
    fn children(&self) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>>;
    fn visible(&self) -> bool;
    fn set_visible(&self, visible: bool) -> ();
    fn get_rendering_component(
        &self,
    ) -> Option<std::rc::Rc<radiance::rendering::RenderingComponent>>;
    fn set_rendering_component(
        &self,
        component: Option<std::rc::Rc<radiance::rendering::RenderingComponent>>,
    ) -> crosscom::Void;
    fn attach(&self, child: crosscom::ComRc<radiance::interfaces::IEntity>) -> ();
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
            use radiance::interfaces::IAnimatedMeshComponentImpl;
            use radiance::interfaces::IComponentContainerImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IEntityImpl;
            use radiance::interfaces::ISceneImpl;
            use radiance::interfaces::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct EntityCcw {
                IEntity: radiance::interfaces::IEntity,

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
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IComponentContainer::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IEntity::INTERFACE_ID => {
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
            ) -> Vec<crosscom::ComRc<radiance::interfaces::IEntity>> {
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
                let child: crosscom::ComRc<radiance::interfaces::IEntity> = child.into();

                let __crosscom_object = crosscom::get_object::<EntityCcw>(this);
                (*__crosscom_object).inner.attach(child.into()).into()
            }

            unsafe extern "system" fn add_component(
                this: *const *const std::os::raw::c_void,
                uuid: uuid::Uuid,
                component: *const *const std::os::raw::c_void,
            ) -> () {
                let uuid: uuid::Uuid = uuid.into();
                let component: crosscom::ComRc<radiance::interfaces::IComponent> = component.into();

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
                radiance::interfaces::IEntityVirtualTableCcw =
                radiance::interfaces::IEntityVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::interfaces::IEntityVirtualTable {
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
                        IEntity: radiance::interfaces::IEntity {
                            vtable: &GLOBAL_IEntityVirtualTable_CCW_FOR_Entity.vtable
                                as *const radiance::interfaces::IEntityVirtualTable,
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
use radiance::interfaces::IComponentImpl;
use radiance::interfaces::IComponentContainerImpl;
use radiance::interfaces::ISceneImpl;
use radiance::interfaces::IEntityImpl;
use radiance::interfaces::IStaticMeshComponentImpl;
use radiance::interfaces::IAnimatedMeshComponentImpl;
use crosscom::IUnknownImpl;
use crosscom::IObjectArrayImpl;


    #[repr(C)]
    pub struct StaticMeshComponentCcw {
        IStaticMeshComponent: radiance::interfaces::IStaticMeshComponent,

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
    crosscom::ResultCode::Ok as i32
}


&radiance::interfaces::IComponent::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as i32
}


&radiance::interfaces::IStaticMeshComponent::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as i32
}


            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<StaticMeshComponentCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<StaticMeshComponentCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut StaticMeshComponentCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }



    unsafe extern "system" fn on_loading (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<StaticMeshComponentCcw>(this);
        (*__crosscom_object).inner.on_loading().into()
    }



    unsafe extern "system" fn on_updating (this: *const *const std::os::raw::c_void, delta_sec: std::os::raw::c_float,
) -> () {
        let delta_sec: f32 = delta_sec.into()
;

        let __crosscom_object = crosscom::get_object::<StaticMeshComponentCcw>(this);
        (*__crosscom_object).inner.on_updating(delta_sec.into()).into()
    }






#[allow(non_upper_case_globals)]
pub const GLOBAL_IStaticMeshComponentVirtualTable_CCW_FOR_StaticMeshComponent: radiance::interfaces::IStaticMeshComponentVirtualTableCcw
    = radiance::interfaces::IStaticMeshComponentVirtualTableCcw {
    offset: 0,
    vtable: radiance::interfaces::IStaticMeshComponentVirtualTable {
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

IStaticMeshComponent: radiance::interfaces::IStaticMeshComponent {
    vtable: &GLOBAL_IStaticMeshComponentVirtualTable_CCW_FOR_StaticMeshComponent.vtable
        as *const radiance::interfaces::IStaticMeshComponentVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(StaticMeshComponentCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
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
    pub morph_animation_state:
        fn(this: *const *const std::os::raw::c_void) -> radiance::rendering::MorphAnimationState,
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

    pub fn morph_animation_state(&self) -> radiance::rendering::MorphAnimationState {
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
    fn morph_animation_state(&self) -> radiance::rendering::MorphAnimationState;
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
            use radiance::interfaces::IAnimatedMeshComponentImpl;
            use radiance::interfaces::IComponentContainerImpl;
            use radiance::interfaces::IComponentImpl;
            use radiance::interfaces::IEntityImpl;
            use radiance::interfaces::ISceneImpl;
            use radiance::interfaces::IStaticMeshComponentImpl;

            #[repr(C)]
            pub struct AnimatedMeshComponentCcw {
                IAnimatedMeshComponent: radiance::interfaces::IAnimatedMeshComponent,

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
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IComponent::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance::interfaces::IAnimatedMeshComponent::INTERFACE_ID => {
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
            ) -> radiance::rendering::MorphAnimationState {
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
                radiance::interfaces::IAnimatedMeshComponentVirtualTableCcw =
                radiance::interfaces::IAnimatedMeshComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::interfaces::IAnimatedMeshComponentVirtualTable {
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

        IAnimatedMeshComponent: radiance::interfaces::IAnimatedMeshComponent {
            vtable: &GLOBAL_IAnimatedMeshComponentVirtualTable_CCW_FOR_AnimatedMeshComponent.vtable
                as *const radiance::interfaces::IAnimatedMeshComponentVirtualTable,
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
