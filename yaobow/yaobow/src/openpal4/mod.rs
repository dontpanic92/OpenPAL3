use self::application::OpenPal4ApplicationLoader;

pub mod application;

pub fn run_openpal4() {
    let app = OpenPal4ApplicationLoader::create_application("OpenPAL4");
    app.initialize();
    app.run();
}
