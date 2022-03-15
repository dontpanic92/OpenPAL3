use crate::cidl::{ast::Idl, traveler::AstVisitor};


struct SemanticAnalyzer;

impl SemanticAnalyzer {
    pub fn analyze(&mut self, idl: &mut Idl) {
        self.visit_idl(idl);
    }
}

impl AstVisitor for SemanticAnalyzer {
    fn visit_idl(&mut self, _idl: &mut Idl) {}

    fn visit_toplevelitem(&mut self, _item: &mut crate::cidl::ast::TopLevelItem) {}

    fn visit_interface(&mut self, _iface: &mut crate::cidl::ast::Interface) {}

    fn visit_class(&mut self, _class: &mut crate::cidl::ast::Class) {}

    fn visit_method(&mut self, _method: &mut crate::cidl::ast::Method) {}

    fn visit_method_parameter(&mut self, _parameter: &mut crate::cidl::ast::MethodParameter) {}
}
