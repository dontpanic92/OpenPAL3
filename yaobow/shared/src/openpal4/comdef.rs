use crate as shared;
// Interface IOpenPAL4Director

#[repr(C)]
#[allow(non_snake_case)]
pub struct IOpenPAL4DirectorVirtualTable {
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
    pub get: fn(
        this: *const *const std::os::raw::c_void,
    ) -> &'static shared::openpal4::director::OpenPAL4Director,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IOpenPAL4DirectorVirtualTableCcw {
    pub offset: isize,
    pub vtable: IOpenPAL4DirectorVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IOpenPAL4Director {
    pub vtable: *const IOpenPAL4DirectorVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IOpenPAL4Director {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
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
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn activate(&self, scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>) -> () {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
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
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret =
                ((*self.vtable).update)(this, scene_manager.into(), ui.into(), delta_sec.into());

            ret
        }
    }

    pub fn get(&self) -> &'static shared::openpal4::director::OpenPAL4Director {
        unsafe {
            let this = self as *const IOpenPAL4Director as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).get)(this);

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IOpenPAL4Director::INTERFACE_ID)
    }
}

pub trait IOpenPAL4DirectorImpl {
    fn get(&self) -> &'static shared::openpal4::director::OpenPAL4Director;
}

impl crosscom::ComInterface for IOpenPAL4Director {
    // f3d7f0fd-20ca-450c-bd66-ad019b984a54
    const INTERFACE_ID: [u8; 16] = [
        243u8, 215u8, 240u8, 253u8, 32u8, 202u8, 69u8, 12u8, 189u8, 102u8, 173u8, 1u8, 155u8,
        152u8, 74u8, 84u8,
    ];
}

// Class OpenPAL4Director

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_OpenPAL4Director {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod OpenPAL4Director_crosscom_impl {
            use crate as shared;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use radiance::comdef::IAnimatedMeshComponentImpl;
            use radiance::comdef::IAnimationEventObserverImpl;
            use radiance::comdef::IApplicationImpl;
            use radiance::comdef::IApplicationLoaderComponentImpl;
            use radiance::comdef::IArmatureComponentImpl;
            use radiance::comdef::IComponentContainerImpl;
            use radiance::comdef::IComponentImpl;
            use radiance::comdef::IDirectorImpl;
            use radiance::comdef::IEntityImpl;
            use radiance::comdef::IHAnimBoneComponentImpl;
            use radiance::comdef::ISceneImpl;
            use radiance::comdef::ISceneManagerImpl;
            use radiance::comdef::ISkinnedMeshComponentImpl;
            use radiance::comdef::IStaticMeshComponentImpl;
            use shared::openpal4::comdef::IOpenPAL4DirectorImpl;
            use shared::openpal4::comdef::IPal4ActorAnimationControllerImpl;
            use shared::openpal4::comdef::IPal4ActorControllerImpl;

            #[repr(C)]
            pub struct OpenPAL4DirectorCcw {
                IOpenPAL4Director: shared::openpal4::comdef::IOpenPAL4Director,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
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

                    &shared::openpal4::comdef::IOpenPAL4Director::INTERFACE_ID => {
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
                let object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut OpenPAL4DirectorCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            fn get(
                this: *const *const std::os::raw::c_void,
            ) -> &'static shared::openpal4::director::OpenPAL4Director {
                unsafe {
                    let __crosscom_object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                    (*__crosscom_object).inner.get()
                }
            }

            unsafe extern "system" fn activate(
                this: *const *const std::os::raw::c_void,
                scene_manager: *const *const std::os::raw::c_void,
            ) -> () {
                let scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager> =
                    scene_manager.into();

                let __crosscom_object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
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
                    let __crosscom_object = crosscom::get_object::<OpenPAL4DirectorCcw>(this);
                    (*__crosscom_object)
                        .inner
                        .update(scene_manager, ui, delta_sec)
                }
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IOpenPAL4DirectorVirtualTable_CCW_FOR_OpenPAL4Director:
                shared::openpal4::comdef::IOpenPAL4DirectorVirtualTableCcw =
                shared::openpal4::comdef::IOpenPAL4DirectorVirtualTableCcw {
                    offset: 0,
                    vtable: shared::openpal4::comdef::IOpenPAL4DirectorVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        activate,
                        update,
                        get,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = OpenPAL4DirectorCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {
                        IOpenPAL4Director: shared::openpal4::comdef::IOpenPAL4Director {
                            vtable: &GLOBAL_IOpenPAL4DirectorVirtualTable_CCW_FOR_OpenPAL4Director
                                .vtable
                                as *const shared::openpal4::comdef::IOpenPAL4DirectorVirtualTable,
                        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this
                            .offset(-(crosscom::offset_of!(OpenPAL4DirectorCcw, inner) as isize));
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_OpenPAL4Director;

// Interface IPal4ActorAnimationController

#[repr(C)]
#[allow(non_snake_case)]
pub struct IPal4ActorAnimationControllerVirtualTable {
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
    pub on_unloading: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub set_default: fn(
        this: *const *const std::os::raw::c_void,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
    ) -> crosscom::Void,
    pub play_default: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub play: fn(
        this: *const *const std::os::raw::c_void,
        animation: shared::openpal4::actor::Pal4ActorAnimation,
        config: shared::openpal4::actor::Pal4ActorAnimationConfig,
    ) -> crosscom::Void,
    pub current:
        fn(this: *const *const std::os::raw::c_void) -> shared::openpal4::actor::Pal4ActorAnimation,
    pub play_animation: fn(
        this: *const *const std::os::raw::c_void,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
        config: shared::openpal4::actor::Pal4ActorAnimationConfig,
    ) -> crosscom::Void,
    pub unhold: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub animation_completed:
        unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IPal4ActorAnimationControllerVirtualTableCcw {
    pub offset: isize,
    pub vtable: IPal4ActorAnimationControllerVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IPal4ActorAnimationController {
    pub vtable: *const IPal4ActorAnimationControllerVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IPal4ActorAnimationController {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this =
            self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
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
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_unloading(&self) -> () {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_unloading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn set_default(
        &self,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
    ) -> crosscom::Void {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).set_default)(this, keyframes.into(), events.into());

            ret
        }
    }

    pub fn play_default(&self) -> () {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).play_default)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn play(
        &self,
        animation: shared::openpal4::actor::Pal4ActorAnimation,
        config: shared::openpal4::actor::Pal4ActorAnimationConfig,
    ) -> crosscom::Void {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).play)(this, animation.into(), config.into());

            ret
        }
    }

    pub fn current(&self) -> shared::openpal4::actor::Pal4ActorAnimation {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).current)(this);

            ret
        }
    }

    pub fn play_animation(
        &self,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
        config: shared::openpal4::actor::Pal4ActorAnimationConfig,
    ) -> crosscom::Void {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).play_animation)(
                this,
                keyframes.into(),
                events.into(),
                config.into(),
            );

            ret
        }
    }

    pub fn unhold(&self) -> () {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).unhold)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn animation_completed(&self) -> bool {
        unsafe {
            let this =
                self as *const IPal4ActorAnimationController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).animation_completed)(this);
            let ret: bool = ret != 0;

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IPal4ActorAnimationController::INTERFACE_ID)
    }
}

pub trait IPal4ActorAnimationControllerImpl {
    fn set_default(
        &self,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
    ) -> crosscom::Void;
    fn play_default(&self) -> ();
    fn play(
        &self,
        animation: shared::openpal4::actor::Pal4ActorAnimation,
        config: shared::openpal4::actor::Pal4ActorAnimationConfig,
    ) -> crosscom::Void;
    fn current(&self) -> shared::openpal4::actor::Pal4ActorAnimation;
    fn play_animation(
        &self,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
        config: shared::openpal4::actor::Pal4ActorAnimationConfig,
    ) -> crosscom::Void;
    fn unhold(&self) -> ();
    fn animation_completed(&self) -> bool;
}

impl crosscom::ComInterface for IPal4ActorAnimationController {
    // f6d70031-86e7-4efa-b1c5-5196063441ea
    const INTERFACE_ID: [u8; 16] = [
        246u8, 215u8, 0u8, 49u8, 134u8, 231u8, 78u8, 250u8, 177u8, 197u8, 81u8, 150u8, 6u8, 52u8,
        65u8, 234u8,
    ];
}

// Class Pal4ActorAnimationController

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Pal4ActorAnimationController {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod Pal4ActorAnimationController_crosscom_impl {
    use crate as shared;
    use crosscom::ComInterface;
use shared::openpal4::comdef::IOpenPAL4DirectorImpl;
use shared::openpal4::comdef::IPal4ActorAnimationControllerImpl;
use shared::openpal4::comdef::IPal4ActorControllerImpl;
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
use radiance::comdef::IArmatureComponentImpl;
use radiance::comdef::ISkinnedMeshComponentImpl;
use radiance::comdef::IHAnimBoneComponentImpl;
use radiance::comdef::IAnimationEventObserverImpl;


    #[repr(C)]
    pub struct Pal4ActorAnimationControllerCcw {
        IPal4ActorAnimationController: shared::openpal4::comdef::IPal4ActorAnimationController,
IAnimationEventObserver: radiance::comdef::IAnimationEventObserver,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
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


&shared::openpal4::comdef::IPal4ActorAnimationController::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


&radiance::comdef::IAnimationEventObserver::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(1);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}


            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut Pal4ActorAnimationControllerCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }



    fn set_default (this: *const *const std::os::raw::c_void, keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
events: Vec<radiance::components::mesh::event::AnimationEvent>,
) -> crosscom::Void {
        unsafe {
            let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
            (*__crosscom_object).inner.set_default(keyframes,events)
        }
    }



    unsafe extern "system" fn play_default (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        (*__crosscom_object).inner.play_default().into()
    }



    fn play (this: *const *const std::os::raw::c_void, animation: shared::openpal4::actor::Pal4ActorAnimation,
config: shared::openpal4::actor::Pal4ActorAnimationConfig,
) -> crosscom::Void {
        unsafe {
            let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
            (*__crosscom_object).inner.play(animation,config)
        }
    }



    fn current (this: *const *const std::os::raw::c_void, ) -> shared::openpal4::actor::Pal4ActorAnimation {
        unsafe {
            let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
            (*__crosscom_object).inner.current()
        }
    }



    fn play_animation (this: *const *const std::os::raw::c_void, keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
events: Vec<radiance::components::mesh::event::AnimationEvent>,
config: shared::openpal4::actor::Pal4ActorAnimationConfig,
) -> crosscom::Void {
        unsafe {
            let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
            (*__crosscom_object).inner.play_animation(keyframes,events,config)
        }
    }



    unsafe extern "system" fn unhold (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        (*__crosscom_object).inner.unhold().into()
    }



    unsafe extern "system" fn animation_completed (this: *const *const std::os::raw::c_void, ) -> std::os::raw::c_int {

        let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        (*__crosscom_object).inner.animation_completed().into()
    }



    fn on_animation_event (this: *const *const std::os::raw::c_void, event_name: &str,
) -> crosscom::Void {
        unsafe {
            let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
            (*__crosscom_object).inner.on_animation_event(event_name)
        }
    }



    unsafe extern "system" fn on_loading (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        (*__crosscom_object).inner.on_loading().into()
    }



    unsafe extern "system" fn on_updating (this: *const *const std::os::raw::c_void, delta_sec: std::os::raw::c_float,
) -> () {
        let delta_sec: f32 = delta_sec.into()
;

        let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        (*__crosscom_object).inner.on_updating(delta_sec.into()).into()
    }



    unsafe extern "system" fn on_unloading (this: *const *const std::os::raw::c_void, ) -> () {

        let __crosscom_object = crosscom::get_object::<Pal4ActorAnimationControllerCcw>(this);
        (*__crosscom_object).inner.on_unloading().into()
    }






#[allow(non_upper_case_globals)]
pub const GLOBAL_IPal4ActorAnimationControllerVirtualTable_CCW_FOR_Pal4ActorAnimationController: shared::openpal4::comdef::IPal4ActorAnimationControllerVirtualTableCcw
    = shared::openpal4::comdef::IPal4ActorAnimationControllerVirtualTableCcw {
    offset: 0,
    vtable: shared::openpal4::comdef::IPal4ActorAnimationControllerVirtualTable {
        query_interface,
add_ref,
release,
on_loading,
on_updating,
on_unloading,
set_default,
play_default,
play,
current,
play_animation,
unhold,
animation_completed,

    },
};



#[allow(non_upper_case_globals)]
pub const GLOBAL_IAnimationEventObserverVirtualTable_CCW_FOR_Pal4ActorAnimationController: radiance::comdef::IAnimationEventObserverVirtualTableCcw
    = radiance::comdef::IAnimationEventObserverVirtualTableCcw {
    offset: -1,
    vtable: radiance::comdef::IAnimationEventObserverVirtualTable {
        query_interface,
add_ref,
release,
on_animation_event,

    },
};




    impl crosscom::ComObject for $impl_type {
        type CcwType = Pal4ActorAnimationControllerCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {

IPal4ActorAnimationController: shared::openpal4::comdef::IPal4ActorAnimationController {
    vtable: &GLOBAL_IPal4ActorAnimationControllerVirtualTable_CCW_FOR_Pal4ActorAnimationController.vtable
        as *const shared::openpal4::comdef::IPal4ActorAnimationControllerVirtualTable,
},

IAnimationEventObserver: radiance::comdef::IAnimationEventObserver {
    vtable: &GLOBAL_IAnimationEventObserverVirtualTable_CCW_FOR_Pal4ActorAnimationController.vtable
        as *const radiance::comdef::IAnimationEventObserverVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(Pal4ActorAnimationControllerCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_Pal4ActorAnimationController;

// Interface IPal4ActorController

#[repr(C)]
#[allow(non_snake_case)]
pub struct IPal4ActorControllerVirtualTable {
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
    pub on_unloading: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
    pub lock_control: unsafe extern "system" fn(
        this: *const *const std::os::raw::c_void,
        lock: std::os::raw::c_int,
    ) -> (),
}

#[repr(C)]
#[allow(dead_code)]
pub struct IPal4ActorControllerVirtualTableCcw {
    pub offset: isize,
    pub vtable: IPal4ActorControllerVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IPal4ActorController {
    pub vtable: *const IPal4ActorControllerVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IPal4ActorController {
    pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
        let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
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
            let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).add_ref)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn release(&self) -> std::os::raw::c_long {
        unsafe {
            let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).release)(this);
            let ret: std::os::raw::c_long = ret.into();

            ret
        }
    }

    pub fn on_loading(&self) -> () {
        unsafe {
            let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_loading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_updating(&self, delta_sec: f32) -> () {
        unsafe {
            let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_updating)(this, delta_sec.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn on_unloading(&self) -> () {
        unsafe {
            let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).on_unloading)(this);
            let ret: () = ret.into();

            ret
        }
    }

    pub fn lock_control(&self, lock: bool) -> () {
        unsafe {
            let this = self as *const IPal4ActorController as *const *const std::os::raw::c_void;
            let ret = ((*self.vtable).lock_control)(this, lock.into());
            let ret: () = ret.into();

            ret
        }
    }

    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IPal4ActorController::INTERFACE_ID)
    }
}

pub trait IPal4ActorControllerImpl {
    fn lock_control(&self, lock: bool) -> ();
}

impl crosscom::ComInterface for IPal4ActorController {
    // 9ccfa4a1-16f9-483c-95d8-6095fbf24e09
    const INTERFACE_ID: [u8; 16] = [
        156u8, 207u8, 164u8, 161u8, 22u8, 249u8, 72u8, 60u8, 149u8, 216u8, 96u8, 149u8, 251u8,
        242u8, 78u8, 9u8,
    ];
}

// Class Pal4ActorController

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Pal4ActorController {
    ($impl_type: ty) => {
        #[allow(dead_code)]
        #[allow(non_snake_case)]
        #[allow(unused)]
        mod Pal4ActorController_crosscom_impl {
            use crate as shared;
            use crosscom::ComInterface;
            use crosscom::IObjectArrayImpl;
            use crosscom::IUnknownImpl;
            use radiance::comdef::IAnimatedMeshComponentImpl;
            use radiance::comdef::IAnimationEventObserverImpl;
            use radiance::comdef::IApplicationImpl;
            use radiance::comdef::IApplicationLoaderComponentImpl;
            use radiance::comdef::IArmatureComponentImpl;
            use radiance::comdef::IComponentContainerImpl;
            use radiance::comdef::IComponentImpl;
            use radiance::comdef::IDirectorImpl;
            use radiance::comdef::IEntityImpl;
            use radiance::comdef::IHAnimBoneComponentImpl;
            use radiance::comdef::ISceneImpl;
            use radiance::comdef::ISceneManagerImpl;
            use radiance::comdef::ISkinnedMeshComponentImpl;
            use radiance::comdef::IStaticMeshComponentImpl;
            use shared::openpal4::comdef::IOpenPAL4DirectorImpl;
            use shared::openpal4::comdef::IPal4ActorAnimationControllerImpl;
            use shared::openpal4::comdef::IPal4ActorControllerImpl;

            #[repr(C)]
            pub struct Pal4ActorControllerCcw {
                IPal4ActorController: shared::openpal4::comdef::IPal4ActorController,

                ref_count: std::sync::atomic::AtomicU32,
                pub inner: $impl_type,
            }

            unsafe extern "system" fn query_interface(
                this: *const *const std::os::raw::c_void,
                guid: uuid::Uuid,
                retval: &mut *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<Pal4ActorControllerCcw>(this);
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

                    &shared::openpal4::comdef::IPal4ActorController::INTERFACE_ID => {
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
                let object = crosscom::get_object::<Pal4ActorControllerCcw>(this);
                let previous = (*object)
                    .ref_count
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (previous + 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn release(
                this: *const *const std::os::raw::c_void,
            ) -> std::os::raw::c_long {
                let object = crosscom::get_object::<Pal4ActorControllerCcw>(this);

                let previous = (*object)
                    .ref_count
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if previous - 1 == 0 {
                    Box::from_raw(object as *mut Pal4ActorControllerCcw);
                }

                (previous - 1) as std::os::raw::c_long
            }

            unsafe extern "system" fn lock_control(
                this: *const *const std::os::raw::c_void,
                lock: std::os::raw::c_int,
            ) -> () {
                let lock: bool = lock != 0;

                let __crosscom_object = crosscom::get_object::<Pal4ActorControllerCcw>(this);
                (*__crosscom_object).inner.lock_control(lock.into()).into()
            }

            unsafe extern "system" fn on_loading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<Pal4ActorControllerCcw>(this);
                (*__crosscom_object).inner.on_loading().into()
            }

            unsafe extern "system" fn on_updating(
                this: *const *const std::os::raw::c_void,
                delta_sec: std::os::raw::c_float,
            ) -> () {
                let delta_sec: f32 = delta_sec.into();

                let __crosscom_object = crosscom::get_object::<Pal4ActorControllerCcw>(this);
                (*__crosscom_object)
                    .inner
                    .on_updating(delta_sec.into())
                    .into()
            }

            unsafe extern "system" fn on_unloading(this: *const *const std::os::raw::c_void) -> () {
                let __crosscom_object = crosscom::get_object::<Pal4ActorControllerCcw>(this);
                (*__crosscom_object).inner.on_unloading().into()
            }

            #[allow(non_upper_case_globals)]
            pub const GLOBAL_IPal4ActorControllerVirtualTable_CCW_FOR_Pal4ActorController:
                shared::openpal4::comdef::IPal4ActorControllerVirtualTableCcw =
                shared::openpal4::comdef::IPal4ActorControllerVirtualTableCcw {
                    offset: 0,
                    vtable: shared::openpal4::comdef::IPal4ActorControllerVirtualTable {
                        query_interface,
                        add_ref,
                        release,
                        on_loading,
                        on_updating,
                        on_unloading,
                        lock_control,
                    },
                };

            impl crosscom::ComObject for $impl_type {
                type CcwType = Pal4ActorControllerCcw;

                fn create_ccw(self) -> Self::CcwType {
                    Self::CcwType {

        IPal4ActorController: shared::openpal4::comdef::IPal4ActorController {
            vtable: &GLOBAL_IPal4ActorControllerVirtualTable_CCW_FOR_Pal4ActorController.vtable
                as *const shared::openpal4::comdef::IPal4ActorControllerVirtualTable,
        },

                        ref_count: std::sync::atomic::AtomicU32::new(0),
                        inner: self,
                    }
                }

                fn get_ccw(&self) -> &Self::CcwType {
                    unsafe {
                        let this = self as *const _ as *const u8;
                        let this = this.offset(
                            -(crosscom::offset_of!(Pal4ActorControllerCcw, inner) as isize),
                        );
                        &*(this as *const Self::CcwType)
                    }
                }
            }
        }
    };
}

// pub use ComObject_Pal4ActorController;
