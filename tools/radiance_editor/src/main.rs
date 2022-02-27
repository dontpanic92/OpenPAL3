mod application;
mod scene;

fn main() {
    let mut application = Application::new(ApplicationCallbacks::new());
    application.initialize();
    application.run();
}
