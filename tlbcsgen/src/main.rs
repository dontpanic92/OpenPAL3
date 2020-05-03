use std::path::Path;
use intercom::{*, raw::*, type_system::*, typelib::*};

#[derive(Serialize, XmlRead, PartialEq, Debug)]
#[xml(tag = "class")]
pub struct Class {
    name: String,
    iid: String,
}

#[derive(Serialize, XmlRead, PartialEq, Debug)]
#[xml(tag = "interface")]
pub struct Intrerface {
    name: String,
    iid: String,
}


#[derive(Serialize, XmlRead, PartialEq, Debug)]
#[xml(tag = "tlbxml")]
pub struct TlbXml {
    #[xml(child = "interface")]
    pub interfaces: Vec<XmlSomInterface>,

    #[xml(child = "class")]
    pub classes: Vec<Class>,
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let path = Path::new(&args[0]);
    let lib = libloading::Library::new(path).unwrap();
    let typelib = unsafe {
        let intercom_typelib = lib.get::<unsafe extern "C" fn(
            TypeSystemName,
            *mut RawComPtr,
        ) -> HRESULT>(b"IntercomTypeLib").unwrap();

        let mut ptr: RawComPtr = std::ptr::null_mut();
        intercom_typelib(TypeSystemName::Automation, &mut ptr as *mut _);

        let com_ptr = InterfacePtr::<AutomationTypeSystem, dyn IIntercomTypeLib>::new(ptr).unwrap();
        intercom::typelib::TypeLib::from_comrc(&ComRc::wrap(com_ptr)).unwrap()
    };

    println!("Hello, world!");
}
