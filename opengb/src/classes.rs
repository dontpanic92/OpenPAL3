// Class PolModel

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_PolModel {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod PolModel_crosscom_impl {
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use radiance::interfaces::IComponentImpl;

            #[repr(C)]
            pub struct PolModelCcw {
                IComponent: radiance::interfaces::IComponent,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<PolModelCcw>(this);
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
                let object = crosscom::get_object::<PolModelCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<PolModelCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut PolModelCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn on_loading(
                this: *const *const std::os::raw::c_void,
                entity: &mut dyn radiance::scene::Entity,
            ) -> crosscom::Void {
                unsafe {
                    let object = crosscom::get_object::<PolModelCcw>(this);
                    (*object).inner.on_loading(entity)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IComponentVirtualTable_CCW_FOR_PolModel:
                radiance::interfaces::IComponentVirtualTableCcw =
                radiance::interfaces::IComponentVirtualTableCcw {
                    offset: 0,
                    vtable: radiance::interfaces::IComponentVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = PolModelCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IComponent: radiance::interfaces::IComponent {
                            vtable: &GLOBAL_IComponentVirtualTable_CCW_FOR_PolModel.vtable
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

pub use ComObject_PolModel;
