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
    pub render: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        scene_manager: std::os::raw::c_longlong,
        ui: std::os::raw::c_longlong,
        delta_sec: std::os::raw::c_float,
    ) -> (),
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

    pub fn add_ref(&self) -> i32 {
        unsafe {
            let this = self as *const IViewContent as *const *const std::os::raw::c_void;
            ((*self.vtable).add_ref)(this).into()
        }
    }

    pub fn release(&self) -> i32 {
        unsafe {
            let this = self as *const IViewContent as *const *const std::os::raw::c_void;
            ((*self.vtable).release)(this).into()
        }
    }

    pub fn render(&self, scene_manager: i64, ui: i64, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IViewContent as *const *const std::os::raw::c_void;
            ((*self.vtable).render)(this, scene_manager, ui, delta_sec).into()
        }
    }
}

pub trait IViewContentImpl {
    fn render(&self, scene_manager: i64, ui: i64, delta_sec: f32) -> ();
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
            use crosscom::ComInterface;
            use radiance_editor::core::IViewContentImpl;

            #[repr(C)]
            pub struct ResourceViewContentCcw {
                IViewContent: radiance_editor::core::IViewContent,

                ref_count: std::sync::atomic::AtomicU32,
                inner: $impl_type,
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
                        crosscom::ResultCode::Ok as i32
                    }

                    &radiance_editor::core::IViewContent::INTERFACE_ID => {
                        *retval = (object as *const *const std::os::raw::c_void).offset(0);
                        add_ref(object as *const *const std::os::raw::c_void);
                        crosscom::ResultCode::Ok as i32
                    }

                    _ => crosscom::ResultCode::ENoInterface as i32,
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

            unsafe extern "system" fn render(
                this: *const *const std::os::raw::c_void,
                scene_manager: std::os::raw::c_longlong,
                ui: std::os::raw::c_longlong,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let object = crosscom::get_object::<ResourceViewContentCcw>(this);
                (*object).inner.render(scene_manager, ui, delta_sec)
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IViewContentVirtualTable_CCW_FOR_ResourceViewContent:
                radiance_editor::core::IViewContentVirtualTableCcw =
                radiance_editor::core::IViewContentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance_editor::core::IViewContentVirtualTable {
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
                        IViewContent: radiance_editor::core::IViewContent {
                            vtable: &GLOBAL_IViewContentVirtualTable_CCW_FOR_ResourceViewContent
                                .vtable
                                as *const radiance_editor::core::IViewContentVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }
            }
        }
    };
}
