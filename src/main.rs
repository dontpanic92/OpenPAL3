mod application;
mod rendering;
mod constants;

fn main() {
   let mut application = application::Application::new();
   application.initialize();
   application.run();
}