use opengb::application::OpenGbApplication;

fn main() {
    let mut app = OpenGbApplication::create("E:\\CubeLibrary\\apps\\1000039\\basedata", "OpenPAL3");
    app.initialize();
    app.run();
}
