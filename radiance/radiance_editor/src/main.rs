use radiance_editor::application::EditorApplication;

fn main() {
    let mut application = EditorApplication::new();
    application.initialize();
    application.run();
}
