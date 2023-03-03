use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent},
};
use radiance_editor::{application, director};

fn main() {
    let application = ComRc::<IApplication>::from_object(Application::new());

    let input = application.engine().borrow().input_engine();
    let director = director::MainPageDirector::create(None, input);

    application.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(application::EditorApplicationLoader::new(
            application.clone(),
            director,
        )),
    );

    application.initialize();
    application.run();
}
