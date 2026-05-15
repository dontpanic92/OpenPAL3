#[allow(unused_imports)]
use crate as crosscom;
// Interface IObjectArray

#[repr(C)]
#[allow(non_snake_case)]
pub struct IObjectArrayVirtualTable {
    pub query_interface: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, guid: uuid::Uuid, retval: &mut *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_ref: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub len: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int,
    pub get: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, index: std::os::raw::c_int) -> *const *const std::os::raw::c_void,
}


#[repr(C)]
#[allow(dead_code)]
pub struct IObjectArrayVirtualTableCcw {
    pub offset: isize,
    pub vtable: IObjectArrayVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IObjectArray {
    pub vtable: *const IObjectArrayVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IObjectArray {
    
pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
    let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
    if ret_val != 0 {
        None
    } else {
        Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
    }
}

pub fn add_ref(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).add_ref)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn release(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).release)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn len(&self, ) -> std::os::raw::c_int {
    unsafe {
        let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).len)(this);
        let ret: std::os::raw::c_int = ret.into();
        ret
    }
}

pub fn get(&self, index: std::os::raw::c_int) -> crosscom::ComRc<crosscom::IUnknown> {
    unsafe {
        let this = self as *const IObjectArray as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).get)(this, index.into());
        let ret: crosscom::ComRc<crosscom::IUnknown> = ret.into();
        ret
    }
}


    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IObjectArray::INTERFACE_ID)
    }
}
pub trait IObjectArrayImpl {
    fn len(&self, ) -> std::os::raw::c_int;
    fn get(&self, index: std::os::raw::c_int) -> crosscom::ComRc<crosscom::IUnknown>;
}

impl crosscom::ComInterface for IObjectArray {
    // 928e03ea-0017-4741-80f9-c70a93b16702
    const INTERFACE_ID: [u8; 16] = [146u8,142u8,3u8,234u8,0u8,23u8,71u8,65u8,128u8,249u8,199u8,10u8,147u8,177u8,103u8,2u8];
}


// Class ObjectArray

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_ObjectArray {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod ObjectArray_crosscom_impl {
    use crate as crosscom;
    use crosscom::ComInterface;
    use crosscom::IUnknownImpl;
    use crosscom::IObjectArrayImpl;
    use crosscom::IActionImpl;
    use crosscom::IIntActionImpl;
    use crosscom::IFloatActionImpl;
    use crosscom::IStrActionImpl;


    #[repr(C)]
    pub struct ObjectArrayCcw {
        IObjectArray: crosscom::IObjectArray,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<ObjectArrayCcw>(this);
        match guid.as_bytes() {
            
&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

&crosscom::IObjectArray::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<ObjectArrayCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<ObjectArrayCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut ObjectArrayCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }

    
    unsafe extern "system" fn len(this: *const *const std::os::raw::c_void) -> std::os::raw::c_int {
        
        let __crosscom_object = crosscom::get_object::<ObjectArrayCcw>(this);
        let ret = (*__crosscom_object).inner.len();
        ret.into()
    }

    unsafe extern "system" fn get(this: *const *const std::os::raw::c_void, index: std::os::raw::c_int) -> *const *const std::os::raw::c_void {
        let index: std::os::raw::c_int = index.into();

        let __crosscom_object = crosscom::get_object::<ObjectArrayCcw>(this);
        let ret = (*__crosscom_object).inner.get(index.into());
        ret.into()
    }


    
#[allow(non_upper_case_globals)]
pub const GLOBAL_IObjectArrayVirtualTable_CCW_FOR_ObjectArray: crosscom::IObjectArrayVirtualTableCcw
    = crosscom::IObjectArrayVirtualTableCcw {
    offset: 0,
    vtable: crosscom::IObjectArrayVirtualTable {
            query_interface,
            add_ref,
            release,
            len,
            get,

    },
};


    impl crosscom::ComObject for $impl_type {
        type CcwType = ObjectArrayCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {
                
IObjectArray: crosscom::IObjectArray {
    vtable: &GLOBAL_IObjectArrayVirtualTable_CCW_FOR_ObjectArray.vtable
        as *const crosscom::IObjectArrayVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(ObjectArrayCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_ObjectArray;
// Interface IAction

#[repr(C)]
#[allow(non_snake_case)]
pub struct IActionVirtualTable {
    pub query_interface: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, guid: uuid::Uuid, retval: &mut *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_ref: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub invoke: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> (),
}


#[repr(C)]
#[allow(dead_code)]
pub struct IActionVirtualTableCcw {
    pub offset: isize,
    pub vtable: IActionVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IAction {
    pub vtable: *const IActionVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IAction {
    
pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
    let this = self as *const IAction as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
    if ret_val != 0 {
        None
    } else {
        Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
    }
}

pub fn add_ref(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).add_ref)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn release(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).release)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn invoke(&self, ) -> () {
    unsafe {
        let this = self as *const IAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).invoke)(this);
        let ret: () = ret.into();
        ret
    }
}


    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IAction::INTERFACE_ID)
    }
}
pub trait IActionImpl {
    fn invoke(&self, ) -> ();
}

impl crosscom::ComInterface for IAction {
    // 5a8b1d3f-7a26-4d4b-9c41-1e9b4f0a0c01
    const INTERFACE_ID: [u8; 16] = [90u8,139u8,29u8,63u8,122u8,38u8,77u8,75u8,156u8,65u8,30u8,155u8,79u8,10u8,12u8,1u8];
}


// Class Action

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_Action {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod Action_crosscom_impl {
    use crate as crosscom;
    use crosscom::ComInterface;
    use crosscom::IUnknownImpl;
    use crosscom::IObjectArrayImpl;
    use crosscom::IActionImpl;
    use crosscom::IIntActionImpl;
    use crosscom::IFloatActionImpl;
    use crosscom::IStrActionImpl;


    #[repr(C)]
    pub struct ActionCcw {
        IAction: crosscom::IAction,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<ActionCcw>(this);
        match guid.as_bytes() {
            
&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

&crosscom::IAction::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<ActionCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<ActionCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut ActionCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }

    
    unsafe extern "system" fn invoke(this: *const *const std::os::raw::c_void) -> () {
        
        let __crosscom_object = crosscom::get_object::<ActionCcw>(this);
        let ret = (*__crosscom_object).inner.invoke();
        ret.into()
    }


    
#[allow(non_upper_case_globals)]
pub const GLOBAL_IActionVirtualTable_CCW_FOR_Action: crosscom::IActionVirtualTableCcw
    = crosscom::IActionVirtualTableCcw {
    offset: 0,
    vtable: crosscom::IActionVirtualTable {
            query_interface,
            add_ref,
            release,
            invoke,

    },
};


    impl crosscom::ComObject for $impl_type {
        type CcwType = ActionCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {
                
IAction: crosscom::IAction {
    vtable: &GLOBAL_IActionVirtualTable_CCW_FOR_Action.vtable
        as *const crosscom::IActionVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(ActionCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_Action;
// Interface IIntAction

#[repr(C)]
#[allow(non_snake_case)]
pub struct IIntActionVirtualTable {
    pub query_interface: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, guid: uuid::Uuid, retval: &mut *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_ref: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub invoke: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, value: std::os::raw::c_int) -> (),
}


#[repr(C)]
#[allow(dead_code)]
pub struct IIntActionVirtualTableCcw {
    pub offset: isize,
    pub vtable: IIntActionVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IIntAction {
    pub vtable: *const IIntActionVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IIntAction {
    
pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
    let this = self as *const IIntAction as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
    if ret_val != 0 {
        None
    } else {
        Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
    }
}

pub fn add_ref(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IIntAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).add_ref)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn release(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IIntAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).release)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn invoke(&self, value: std::os::raw::c_int) -> () {
    unsafe {
        let this = self as *const IIntAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).invoke)(this, value.into());
        let ret: () = ret.into();
        ret
    }
}


    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IIntAction::INTERFACE_ID)
    }
}
pub trait IIntActionImpl {
    fn invoke(&self, value: std::os::raw::c_int) -> ();
}

impl crosscom::ComInterface for IIntAction {
    // 5a8b1d3f-7a26-4d4b-9c41-1e9b4f0a0c03
    const INTERFACE_ID: [u8; 16] = [90u8,139u8,29u8,63u8,122u8,38u8,77u8,75u8,156u8,65u8,30u8,155u8,79u8,10u8,12u8,3u8];
}


// Class IntAction

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_IntAction {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod IntAction_crosscom_impl {
    use crate as crosscom;
    use crosscom::ComInterface;
    use crosscom::IUnknownImpl;
    use crosscom::IObjectArrayImpl;
    use crosscom::IActionImpl;
    use crosscom::IIntActionImpl;
    use crosscom::IFloatActionImpl;
    use crosscom::IStrActionImpl;


    #[repr(C)]
    pub struct IntActionCcw {
        IIntAction: crosscom::IIntAction,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<IntActionCcw>(this);
        match guid.as_bytes() {
            
&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

&crosscom::IIntAction::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<IntActionCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<IntActionCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut IntActionCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }

    
    unsafe extern "system" fn invoke(this: *const *const std::os::raw::c_void, value: std::os::raw::c_int) -> () {
        let value: std::os::raw::c_int = value.into();

        let __crosscom_object = crosscom::get_object::<IntActionCcw>(this);
        let ret = (*__crosscom_object).inner.invoke(value.into());
        ret.into()
    }


    
#[allow(non_upper_case_globals)]
pub const GLOBAL_IIntActionVirtualTable_CCW_FOR_IntAction: crosscom::IIntActionVirtualTableCcw
    = crosscom::IIntActionVirtualTableCcw {
    offset: 0,
    vtable: crosscom::IIntActionVirtualTable {
            query_interface,
            add_ref,
            release,
            invoke,

    },
};


    impl crosscom::ComObject for $impl_type {
        type CcwType = IntActionCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {
                
IIntAction: crosscom::IIntAction {
    vtable: &GLOBAL_IIntActionVirtualTable_CCW_FOR_IntAction.vtable
        as *const crosscom::IIntActionVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(IntActionCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_IntAction;
// Interface IFloatAction

#[repr(C)]
#[allow(non_snake_case)]
pub struct IFloatActionVirtualTable {
    pub query_interface: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, guid: uuid::Uuid, retval: &mut *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_ref: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub invoke: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, value: std::os::raw::c_float) -> (),
}


#[repr(C)]
#[allow(dead_code)]
pub struct IFloatActionVirtualTableCcw {
    pub offset: isize,
    pub vtable: IFloatActionVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IFloatAction {
    pub vtable: *const IFloatActionVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IFloatAction {
    
pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
    let this = self as *const IFloatAction as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
    if ret_val != 0 {
        None
    } else {
        Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
    }
}

pub fn add_ref(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IFloatAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).add_ref)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn release(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IFloatAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).release)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn invoke(&self, value: f32) -> () {
    unsafe {
        let this = self as *const IFloatAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).invoke)(this, value.into());
        let ret: () = ret.into();
        ret
    }
}


    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IFloatAction::INTERFACE_ID)
    }
}
pub trait IFloatActionImpl {
    fn invoke(&self, value: f32) -> ();
}

impl crosscom::ComInterface for IFloatAction {
    // 5a8b1d3f-7a26-4d4b-9c41-1e9b4f0a0c05
    const INTERFACE_ID: [u8; 16] = [90u8,139u8,29u8,63u8,122u8,38u8,77u8,75u8,156u8,65u8,30u8,155u8,79u8,10u8,12u8,5u8];
}


// Class FloatAction

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_FloatAction {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod FloatAction_crosscom_impl {
    use crate as crosscom;
    use crosscom::ComInterface;
    use crosscom::IUnknownImpl;
    use crosscom::IObjectArrayImpl;
    use crosscom::IActionImpl;
    use crosscom::IIntActionImpl;
    use crosscom::IFloatActionImpl;
    use crosscom::IStrActionImpl;


    #[repr(C)]
    pub struct FloatActionCcw {
        IFloatAction: crosscom::IFloatAction,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<FloatActionCcw>(this);
        match guid.as_bytes() {
            
&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

&crosscom::IFloatAction::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<FloatActionCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<FloatActionCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut FloatActionCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }

    
    unsafe extern "system" fn invoke(this: *const *const std::os::raw::c_void, value: std::os::raw::c_float) -> () {
        let value: f32 = value.into();

        let __crosscom_object = crosscom::get_object::<FloatActionCcw>(this);
        let ret = (*__crosscom_object).inner.invoke(value.into());
        ret.into()
    }


    
#[allow(non_upper_case_globals)]
pub const GLOBAL_IFloatActionVirtualTable_CCW_FOR_FloatAction: crosscom::IFloatActionVirtualTableCcw
    = crosscom::IFloatActionVirtualTableCcw {
    offset: 0,
    vtable: crosscom::IFloatActionVirtualTable {
            query_interface,
            add_ref,
            release,
            invoke,

    },
};


    impl crosscom::ComObject for $impl_type {
        type CcwType = FloatActionCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {
                
IFloatAction: crosscom::IFloatAction {
    vtable: &GLOBAL_IFloatActionVirtualTable_CCW_FOR_FloatAction.vtable
        as *const crosscom::IFloatActionVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(FloatActionCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_FloatAction;
// Interface IStrAction

#[repr(C)]
#[allow(non_snake_case)]
pub struct IStrActionVirtualTable {
    pub query_interface: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, guid: uuid::Uuid, retval: &mut *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub add_ref: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub release: unsafe extern "system" fn(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long,
    pub invoke: unsafe extern "system" fn(this: *const *const std::os::raw::c_void, value: *const std::os::raw::c_char) -> (),
}


#[repr(C)]
#[allow(dead_code)]
pub struct IStrActionVirtualTableCcw {
    pub offset: isize,
    pub vtable: IStrActionVirtualTable,
}

#[repr(C)]
#[allow(dead_code)]
pub struct IStrAction {
    pub vtable: *const IStrActionVirtualTable,
}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl IStrAction {
    
pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {
    let this = self as *const IStrAction as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe { ((*self.vtable).query_interface)(this, guid, &mut raw) };
    if ret_val != 0 {
        None
    } else {
        Some(unsafe { crosscom::ComRc::<T>::from_raw_pointer(raw) })
    }
}

pub fn add_ref(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IStrAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).add_ref)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn release(&self, ) -> std::os::raw::c_long {
    unsafe {
        let this = self as *const IStrAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).release)(this);
        let ret: std::os::raw::c_long = ret.into();
        ret
    }
}

pub fn invoke(&self, value: &str) -> () {
    unsafe {
        let this = self as *const IStrAction as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).invoke)(this, std::ffi::CString::new(value).unwrap().as_ptr());
        let ret: () = ret.into();
        ret
    }
}


    pub fn uuid() -> uuid::Uuid {
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes(IStrAction::INTERFACE_ID)
    }
}
pub trait IStrActionImpl {
    fn invoke(&self, value: &str) -> ();
}

impl crosscom::ComInterface for IStrAction {
    // 5a8b1d3f-7a26-4d4b-9c41-1e9b4f0a0c07
    const INTERFACE_ID: [u8; 16] = [90u8,139u8,29u8,63u8,122u8,38u8,77u8,75u8,156u8,65u8,30u8,155u8,79u8,10u8,12u8,7u8];
}


// Class StrAction

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_StrAction {
    ($impl_type: ty) => {

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod StrAction_crosscom_impl {
    use crate as crosscom;
    use crosscom::ComInterface;
    use crosscom::IUnknownImpl;
    use crosscom::IObjectArrayImpl;
    use crosscom::IActionImpl;
    use crosscom::IIntActionImpl;
    use crosscom::IFloatActionImpl;
    use crosscom::IStrActionImpl;


    #[repr(C)]
    pub struct StrActionCcw {
        IStrAction: crosscom::IStrAction,

        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {
        let object = crosscom::get_object::<StrActionCcw>(this);
        match guid.as_bytes() {
            
&crosscom::IUnknown::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

&crosscom::IStrAction::INTERFACE_ID => {
    *retval = (object as *const *const std::os::raw::c_void).offset(0);
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as std::os::raw::c_long
}

            _ => crosscom::ResultCode::ENoInterface as std::os::raw::c_long,
        }
    }

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<StrActionCcw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {
        let object = crosscom::get_object::<StrActionCcw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {
            Box::from_raw(object as *mut StrActionCcw);
        }

        (previous - 1) as std::os::raw::c_long
    }

    
    unsafe extern "system" fn invoke(this: *const *const std::os::raw::c_void, value: *const std::os::raw::c_char) -> () {
        let value: &str = unsafe { std::ffi::CStr::from_ptr(value).to_str().unwrap() };

        let __crosscom_object = crosscom::get_object::<StrActionCcw>(this);
        let ret = (*__crosscom_object).inner.invoke(value.into());
        ret.into()
    }


    
#[allow(non_upper_case_globals)]
pub const GLOBAL_IStrActionVirtualTable_CCW_FOR_StrAction: crosscom::IStrActionVirtualTableCcw
    = crosscom::IStrActionVirtualTableCcw {
    offset: 0,
    vtable: crosscom::IStrActionVirtualTable {
            query_interface,
            add_ref,
            release,
            invoke,

    },
};


    impl crosscom::ComObject for $impl_type {
        type CcwType = StrActionCcw;

        fn create_ccw(self) -> Self::CcwType {
            Self::CcwType {
                
IStrAction: crosscom::IStrAction {
    vtable: &GLOBAL_IStrActionVirtualTable_CCW_FOR_StrAction.vtable
        as *const crosscom::IStrActionVirtualTable,
},

                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }
        }

        fn get_ccw(&self) -> &Self::CcwType {
            unsafe {
                let this = self as *const _ as *const u8;
                let this = this.offset(-(crosscom::offset_of!(StrActionCcw, inner) as isize));
                &*(this as *const Self::CcwType)
            }
        }
    }
}
    }
}

// pub use ComObject_StrAction;
