use application::OpenPal5ApplicationLoader;

mod application;

pub fn run_openpal5() {
    let app = OpenPal5ApplicationLoader::create_application();
    app.initialize();
    app.run();
}
