use super::ast::*;

pub trait AstVisitor {
    fn visit_idl(&mut self, _idl: &mut Idl) {}
    fn visit_toplevelitem(&mut self, _item: &mut TopLevelItem) {}
    fn visit_interface(&mut self, _iface: &mut Interface) {}
    fn visit_class(&mut self, _class: &mut Class) {}
    fn visit_method(&mut self, _method: &mut Method) {}
    fn visit_method_parameter(&mut self, _parameter: &mut MethodParameter) {}
}

pub fn traverse_ast(idl: &mut Idl, visitor: &mut dyn AstVisitor) {
    visitor.visit_idl(idl);

    for item in &mut idl.items {
        traverse_toplevelitem(item, visitor);
    }
}

fn traverse_toplevelitem(item: &mut TopLevelItem, visitor: &mut dyn AstVisitor) {
    visitor.visit_toplevelitem(item);

    match &mut item.definition {
        TopLevelItemDefinition::Interface(iface) => traverse_interface(iface, visitor),
        TopLevelItemDefinition::Class(class) => traverse_class(class, visitor),
    }
}

fn traverse_interface(iface: &mut Interface, visitor: &mut dyn AstVisitor) {
    visitor.visit_interface(iface);

    for method in &mut iface.methods {
        traverse_method(method, visitor);
    }
}

fn traverse_class(class: &mut Class, visitor: &mut dyn AstVisitor) {
    visitor.visit_class(class);
}

fn traverse_method(method: &mut Method, visitor: &mut dyn AstVisitor) {
    visitor.visit_method(method);

    for parameter in &mut method.parameters {
        traverse_method_parameter(parameter, visitor);
    }
}

fn traverse_method_parameter(parameter: &mut MethodParameter, visitor: &mut dyn AstVisitor) {
    visitor.visit_method_parameter(parameter);
}
