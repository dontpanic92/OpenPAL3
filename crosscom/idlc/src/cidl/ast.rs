use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct Extra {
    pub string: HashMap<String, String>,
    pub array: HashMap<String, Vec<String>>,
}

impl Extra {
    pub fn new() -> Self {
        Self {
            string: HashMap::new(),
            array: HashMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Idl {
    pub items: Vec<TopLevelItem>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Interface {
    pub name: String,
    pub methods: Vec<Method>,
    pub extends: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub extra: Extra,
}

#[derive(Debug, Serialize, Clone)]
pub struct InterfaceBody {
    pub methods: Vec<Method>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Method {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<MethodParameter>,
    pub extra: Extra,
}

#[derive(Debug, Serialize, Clone)]
pub struct MethodParameter {
    pub name: String,
    pub ty: String,
    pub extra: Extra,
}

#[derive(Debug, Serialize, Clone)]
pub struct Class {
    pub name: String,
    pub implements: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub extra: Extra,
}

#[derive(Debug, Serialize, Clone)]
pub struct TopLevelItem {
    pub attributes: HashMap<String, String>,
    pub definition: TopLevelItemDefinition,
}

#[derive(Debug, Serialize, Clone)]
pub enum TopLevelItemDefinition {
    Interface(Interface),
    Class(Class),
}

impl TopLevelItemDefinition {
    pub fn set_attributes(&mut self, attributes: HashMap<String, String>) {
        match self {
            TopLevelItemDefinition::Class(class) => class.attributes.extend(attributes),
            TopLevelItemDefinition::Interface(interface) => interface.attributes.extend(attributes),
        }
    }
}
