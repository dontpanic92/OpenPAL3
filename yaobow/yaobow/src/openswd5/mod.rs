use self::application::OpenSwd5ApplicationLoader;

mod application;

pub fn run_openswd5() {
    let app = OpenSwd5ApplicationLoader::create_application();
    app.initialize();
    app.run();
}
