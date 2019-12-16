mod application;
mod rendering;
mod constants;
mod math;

#[macro_use]
extern crate memoffset;

#[macro_use]
extern crate lazy_static;

fn main() {
   let mut application = application::Application::new();
   application.initialize();
   application.run();
}
