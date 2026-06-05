use crosscom::ComRc;
use radiance::{
    application::Application,
    comdef::{IApplication, IApplicationExt, IApplicationLoaderComponent},
};
use radiance_editor::{application, director};

fn main() {
    let application = ComRc::<IApplication>::from_object(Application::new());

    application.add_component(
        IApplicationLoaderComponent::uuid(),
        ComRc::from_object(application::EditorApplicationLoader::new(
            application.clone(),
            // Director construction is deferred to `on_loading` (i.e.
            // after the first-resumed engine bootstrap), so the
            // engine accessors below are safe.
            Box::new(|app| {
                let engine = app.engine();
                let engine = engine.borrow();
                director::MainPageDirector::create(
                    None,
                    engine.ui_manager(),
                    engine.input_engine(),
                    engine.scene_manager(),
                )
            }),
        )),
    );

    application.initialize();
    application.run();
}
