use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent},
};
use radiance_editor::application::EditorApplicationLoader;

fn main() {
    let application = ComRc::<IApplication>::from_object(Application::new());
    application.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(EditorApplicationLoader::new(application.clone(), None)),
    );

    application.initialize();
    application.run();
}
