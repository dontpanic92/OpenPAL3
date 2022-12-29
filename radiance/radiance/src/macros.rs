macro_rules! ext_call {
    ($self: ident, $x: ident $(, $params: expr)*) => {
        {
            let ext = $self.extension.clone();
            let mut ref_mut = (*ext).borrow_mut();
            ref_mut.$x($self $(, $params)*);
        }
    };
}

macro_rules! define_ext_fn {
    ($name: ident, $struct: ident, $extension_type: ty $(, $var_name: ident : $var_type: ty)*) => {
        fn $name(&mut self, _app: &mut $struct<$extension_type> $(, $var_name: $var_type)*) {}
    };
}
