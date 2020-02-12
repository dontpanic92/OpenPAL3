
use gobject_gen::gobject_gen;
use std::cell::{Cell, RefCell};

fn test() {}

#[cfg(feature = "test-generated")]
include!("../generated/MyClass-1.0.rs");

#[cfg(feature = "test-generated")]
include!("../generated/MyClass2-1.0.rs");

#[cfg(not(feature = "test-generated"))]
gobject_gen! {
    #[generate("generated/MyClass-1.0.rs")]
    #[generate("generated/MyClass-1.0.gir")]
    class MyClass: glib::Object {
        foo: Cell<i32>,
        bar: RefCell<String>,
    }

    impl MyClass {
        virtual fn my_virtual_method(&self, x: i32) {
            println!("in my class");
        }

        pub fn my_method(&self, x: i32) {
            println!("in my class {}", x);
        }
    }

    #[generate("generated/MyClass2-1.0.rs")]
    #[generate("generated/MyClass2-1.0.gir")]
    class MyClass2: glib::Object {
    }

    impl MyClass2 {
        pub fn my_method(&self, cls: &MyClass) {
            println!("in my class2. Calling cls->my_virtual_method 42");
            cls.my_virtual_method(42);
        }
    }
}
