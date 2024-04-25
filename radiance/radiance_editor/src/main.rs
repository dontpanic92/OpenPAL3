use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationLoaderComponent},
};
use radiance_editor::{application, director};

fn main() {
    let application = ComRc::<IApplication>::from_object(Application::new());

    let input = application.engine().borrow().input_engine();
    let ui = application.engine().borrow().ui_manager();
    let scene_manager = application.engine().borrow().scene_manager();
    let director = director::MainPageDirector::create(None, ui, input, scene_manager);

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
