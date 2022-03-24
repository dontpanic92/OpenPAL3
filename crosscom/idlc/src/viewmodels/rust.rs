use serde::Serialize;

use crate::{
    analysis::symbols::{Ancestor, Symbol, SymbolTable, SymbolType},
    cidl::ast::Method,
};

#[derive(Serialize)]
pub struct RustViewModel {
    pub interfaces: Vec<InterfaceViewModel>,
    pub classes: Vec<ClassViewModel>,
}

impl RustViewModel {
    pub fn from_symbols(symbols: &SymbolTable) -> Self {
        let mut interfaces = vec![];
        let mut classes = vec![];

        for (_, symbol) in &symbols.0 {
            let symbol_ref = symbol.borrow();
            if symbol_ref.name == "IUnknown" {
                continue;
            }

            match symbol_ref.symbol_type {
                SymbolType::Class => {
                    classes.push(ClassViewModel::from_symbol(&symbol_ref, symbols))
                }
                SymbolType::Interface => {
                    interfaces.push(InterfaceViewModel::from_symbol(&symbol_ref, symbols))
                }
            }
        }

        Self {
            interfaces,
            classes,
        }
    }
}

#[derive(Serialize)]
pub struct InterfaceViewModel {
    pub name: String,
    pub uuid: String,
    pub methods: Vec<InterfaceMethodViewModel>,
    pub all_methods: Vec<InterfaceMethodViewModel>,
    pub parent: Option<Box<InterfaceViewModel>>,
}

impl InterfaceViewModel {
    pub fn from_symbol(symbol: &Symbol, symbols: &SymbolTable) -> Self {
        if symbol.name == "IUnknown" {
            return Self::get_iunknown();
        }

        let methods = Self::methods_from_symbol(symbol, symbols);

        let parent_symbol = symbols.0.get(&symbol.parents[0]).unwrap().borrow();

        let parent = Self::from_symbol(&parent_symbol, symbols);
        let mut all_methods = parent.all_methods.clone();
        all_methods.extend(methods.clone());

        Self {
            name: symbol.name.clone(),
            uuid: symbol.uuid.clone(),
            parent: Some(Box::new(parent)),
            methods,
            all_methods,
        }
    }

    fn methods_from_symbol(
        symbol: &Symbol,
        symbols: &SymbolTable,
    ) -> Vec<InterfaceMethodViewModel> {
        symbol
            .methods
            .iter()
            .map(|m| InterfaceMethodViewModel::from_method(m, symbols))
            .collect()
    }

    fn get_iunknown() -> Self {
        let methods = vec![
            InterfaceMethodViewModel {
                name: "query_interface".to_string(),
                raw_signature: "unsafe extern \"system\" fn (this: *const std::os::raw::c_void, guid: uuid::Uuid, retval: *mut std::os::raw::c_void,) -> std::os::raw::c_long".to_string(),
                raw_signature_with_name:"unsafe extern \"system\" fn query_interface(this: *const std::os::raw::c_void, guid: uuid::Uuid, retval: *mut std::os::raw::c_void,) -> std::os::raw::c_long".to_string(),
                signature_with_name: "fn query_interface(&self, guid: Uuid, retval: *mut c_void) -> c_long".to_string(),
                argument_list: "guid, retval".to_string(),
            },
            InterfaceMethodViewModel {
                name: "add_ref".to_string(),
                raw_signature: "unsafe extern \"system\" fn (this: *const std::os::raw::c_void,) -> std::os::raw::c_long".to_string(),
                raw_signature_with_name: "unsafe extern \"system\" fn add_ref (this: *const std::os::raw::c_void,) -> std::os::raw::c_long".to_string(),
                signature_with_name: "fn add_ref(&self) -> c_long".to_string(),
                argument_list: "".to_string(),
            },
            InterfaceMethodViewModel {
                name: "release".to_string(),
                raw_signature: "unsafe extern \"system\" fn (this: *const std::os::raw::c_void,) -> std::os::raw::c_long".to_string(),
                raw_signature_with_name: "unsafe extern \"system\" fn release (this: *const std::os::raw::c_void,) -> std::os::raw::c_long".to_string(),
                signature_with_name: "fn release(&self) -> c_long".to_string(),
                argument_list: "".to_string(),
            }];

        Self {
            name: "IUnknown".to_string(),
            uuid: "00000000-0000-0000-C000-000000000046".to_string(),
            parent: None,
            methods: methods.clone(),
            all_methods: methods,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct InterfaceMethodViewModel {
    pub name: String,
    pub raw_signature: String,
    pub raw_signature_with_name: String,
    pub signature_with_name: String,
    pub argument_list: String,
}

impl InterfaceMethodViewModel {
    pub fn from_method(method: &Method, symbols: &SymbolTable) -> Self {
        let (raw_signature, raw_signature_with_name) = gen_raw_method_signature(method, symbols);
        let signature_with_name = gen_method_signature(method, symbols);
        let argument_list = method
            .parameters
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>()
            .join(",");

        Self {
            name: method.name.clone(),
            raw_signature,
            raw_signature_with_name,
            signature_with_name,
            argument_list,
        }
    }
}

#[derive(Serialize)]
pub struct ClassViewModel {
    pub name: String,
    pub uuid: String,
    pub ancestors: Vec<Ancestor>,
    pub parents: Vec<InterfaceViewModel>,
    pub methods: Vec<ClassMethodViewModel>,

    pub methods_to_implement: Vec<InterfaceMethodViewModel>,
}

impl ClassViewModel {
    pub fn from_symbol(symbol: &Symbol, symbols: &SymbolTable) -> Self {
        let methods = symbol
            .methods
            .iter()
            .map(|m| ClassMethodViewModel::from_method(m, symbols))
            .collect();

        let parents = symbol
            .parents
            .iter()
            .map(|p| {
                let symbol_ref = symbols.0.get(p).unwrap().borrow();
                InterfaceViewModel::from_symbol(&symbol_ref, symbols)
            })
            .collect();

        let methods_to_implement = symbol
            .ancestors
            .iter()
            .filter(|a| a.name.as_str() != "IUnknown")
            .flat_map(|a| {
                let symbol_ref = symbols.0.get(&a.name).unwrap().borrow();
                let iface = InterfaceViewModel::from_symbol(&symbol_ref, symbols);
                iface.methods
            })
            .collect();

        Self {
            name: symbol.name.clone(),
            uuid: symbol.uuid.clone(),
            ancestors: symbol.ancestors.clone(),
            parents,
            methods,
            methods_to_implement,
        }
    }
}

#[derive(Serialize)]
pub struct ClassMethodViewModel {
    pub name: String,
    pub raw_signature_with_name: String,
}

impl ClassMethodViewModel {
    pub fn from_method(method: &Method, symbols: &SymbolTable) -> Self {
        let (_, raw_signature_with_name) = gen_raw_method_signature(method, symbols);

        Self {
            name: method.name.clone(),
            raw_signature_with_name,
        }
    }
}

fn gen_raw_method_signature(method: &Method, symbols: &SymbolTable) -> (String, String) {
    let parameter_list = &method
        .parameters
        .iter()
        .map(|p| {
            let rust_raw_ty = idl_type_to_raw_type(&p.ty, symbols);
            format!("{}: {}", p.name, rust_raw_ty)
        })
        .collect::<Vec<String>>()
        .join(",");
    let return_type = idl_type_to_raw_type(&method.return_type, symbols);

    let raw_signature = format!(
        "unsafe extern \"system\" fn (this: *const std::os::raw::c_void, {}) -> {}",
        parameter_list, return_type,
    );

    let raw_signature_with_name = format!(
        "unsafe extern \"system\" fn {} (this: *const std::os::raw::c_void, {}) -> {}",
        method.name, parameter_list, return_type,
    );

    (raw_signature, raw_signature_with_name)
}

fn gen_method_signature(method: &Method, symbols: &SymbolTable) -> String {
    let parameter_list = &method
        .parameters
        .iter()
        .map(|p| {
            let rust_rust_ty = idl_type_to_rust_type(&p.ty, symbols);
            format!("{}: {}", p.name, rust_rust_ty)
        })
        .collect::<Vec<String>>()
        .join(",");
    let return_type = idl_type_to_rust_type(&method.return_type, symbols);

    let signature_with_name = format!(
        "fn {} (&self, {}) -> {}",
        method.name, parameter_list, return_type,
    );

    signature_with_name
}

fn idl_type_to_raw_type(idl_type: &str, symbols: &SymbolTable) -> String {
    match idl_type {
        "long" => "std::os::raw::c_long".to_string(),
        "int" => "std::os::raw::c_int".to_string(),
        "void" => "()".to_string(),
        ty => {
            if let Some(symbol) = symbols.0.get(ty) {
                let symbol_ref = symbol.borrow();
                match symbol_ref.symbol_type {
                    SymbolType::Interface => {
                        return "*const std::os::raw::c_void".to_string();
                    }
                    SymbolType::Class => {
                        panic!(
                            "Cannot use class type as parameter type:  {}",
                            symbol_ref.name
                        )
                    }
                }
            } else {
                panic!("Unknown symbol: {}", ty);
            }
        }
    }
}

fn idl_type_to_rust_type(idl_type: &str, symbols: &SymbolTable) -> String {
    match idl_type {
        "long" => "i64".to_string(),
        "int" => "i32".to_string(),
        "void" => "()".to_string(),
        ty => {
            if let Some(symbol) = symbols.0.get(ty) {
                let symbol_ref = symbol.borrow();
                match symbol_ref.symbol_type {
                    SymbolType::Interface => {
                        return format!("ComRc<{}>", symbol_ref.name);
                    }
                    SymbolType::Class => {
                        panic!(
                            "Cannot use class type as parameter type:  {}",
                            symbol_ref.name
                        )
                    }
                }
            } else {
                panic!("Unknown symbol: {}", ty);
            }
        }
    }
}
