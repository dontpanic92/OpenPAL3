use std::cell::RefCell;

pub mod symbols;

use crate::cidl::{
    ast::Idl,
    traveler::{traverse_ast, AstVisitor},
};

use self::symbols::{Symbol, SymbolTable, SymbolType};

pub struct SemanticAnalyzer {
    symbols: SymbolTable,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
        }
    }

    pub fn analyze(&mut self, idl: &mut Idl) {
        traverse_ast(idl, self);

        for (_, symbol) in &self.symbols.0 {
            let mut symbol_ref = symbol.borrow_mut();
            symbol_ref.analyze_ancestors(&self.symbols);
        }
    }

    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }
}

impl AstVisitor for SemanticAnalyzer {
    fn visit_idl(&mut self, _idl: &mut Idl) {}

    fn visit_toplevelitem(&mut self, _item: &mut crate::cidl::ast::TopLevelItem) {}

    fn visit_interface(&mut self, iface: &mut crate::cidl::ast::Interface) {
        self.symbols.0.insert(
            iface.name.clone(),
            RefCell::new(Symbol::new(
                iface.name.clone(),
                SymbolType::Interface,
                iface.extends.clone(),
                iface.methods.clone(),
                &iface.attributes,
            )),
        );
    }

    fn visit_class(&mut self, class: &mut crate::cidl::ast::Class) {
        self.symbols.0.insert(
            class.name.clone(),
            RefCell::new(Symbol::new(
                class.name.clone(),
                SymbolType::Class,
                class.implements.clone(),
                vec![],
                &class.attributes,
            )),
        );
    }

    fn visit_method(&mut self, _method: &mut crate::cidl::ast::Method) {}

    fn visit_method_parameter(&mut self, _parameter: &mut crate::cidl::ast::MethodParameter) {}
}
