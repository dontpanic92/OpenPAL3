mod application;
mod rendering;
mod constants;

fn main() {
   let application = application::Application::new();
   application.initialize();
   application.run();
}