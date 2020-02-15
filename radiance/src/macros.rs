macro_rules! callback {
    ($self: ident, $x: ident $(, $params: expr)*) => {
        {
            let callbacks = $self.callbacks.clone();
            let mut ref_mut = (*callbacks).borrow_mut();
            ref_mut.$x($self $(, $params)*);
        }
    };
}

macro_rules! define_callback_fn {
    ($name: ident, $struct: ident, $callback_trait: ident $(, $var_name: ident : $var_type: ty)*) => {
        fn $name<T: $callback_trait>(&mut self, _app: &mut $struct<T> $(, $var_name: $var_type)*) {}
    };
}

