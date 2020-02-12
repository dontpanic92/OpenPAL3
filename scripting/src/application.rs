use gobject_gen::gobject_gen;
use glib::Object as GObject;
use std::cell::{Cell, RefCell};
use radiance::application as ra_app;

#[cfg(feature = "test-generated")]
include!("../generated/application.rs");

struct GiApplicationCallbacks {
    pub g_object: *mut imp::ApplicationFfi,
}

impl ra_app::ApplicationCallbacks for GiApplicationCallbacks {
    fn on_updated<T: ra_app::ApplicationCallbacks>(&mut self, app: &mut ra_app::Application<T>, delta_sec: f32) {
        let vtable = unsafe {
            use gobject_class::gobject_sys as gobject_ffi;
            let klass = (*(self.g_object as *const _ as *const gobject_ffi::GTypeInstance)).g_class;
            &*(klass as *const imp::ApplicationClass)
        };
        let ret = unsafe { (vtable.on_updated.as_ref().unwrap())(self.g_object, (delta_sec * 1000000.) as i32) };
        ret
    }
}

struct GiApplication(RefCell<ra_app::Application<GiApplicationCallbacks>>);

impl Default for GiApplication {
    fn default() -> Self {
        let app = ra_app::Application::<GiApplicationCallbacks>::new(RefCell::new(GiApplicationCallbacks { g_object: 0 as *mut imp::ApplicationFfi }));
        Self { 0: RefCell::new(app) }
    }
}

#[cfg(not(feature = "test-generated"))]
gobject_gen! {
    #[generate("generated/application.rs")]
    #[generate("generated/application.gir")]
    class Application: GObject {
        application: GiApplication,
    }

    impl Application {
        pub fn initialize(&self) {
            self.get_priv().application.0.borrow_mut().callbacks_mut().g_object 
                = <Self as glib::translate::ToGlibPtr<'_, *mut ApplicationFfi,>>::to_glib_none(self).0;
            self.get_priv().application.0.borrow_mut().initialize();
        }

        virtual fn on_updated(&self, _delta_time: i32) {
            println!("in application on_updated");
        }

        pub fn run(&self) {
            self.get_priv().application.0.borrow_mut().run();
        }
    }
}
