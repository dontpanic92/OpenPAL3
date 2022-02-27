struct ApplicationCallbacks {}

impl ApplicationExtension<ApplicationCallbacks> for ApplicationCallbacks {
    fn on_initialized(&mut self, app: &mut Application<ApplicationCallbacks>) {
        let logger = simple_logger::SimpleLogger::new();
        logger.init().unwrap();
        let factory = app.engine_mut().rendering_component_factory();

        let asset_mgr = AssetManager::new(factory, &self.config.asset_path);
        let input_engine = app.engine_mut().input_engine();
        let audio_engine = app.engine_mut().audio_engine();

        app.engine_mut()
            .scene_manager()
            .push_scene(Box::new(CoreScene::new(EditorScene::new())));
        app.engine_mut()
            .scene_manager()
            .set_director(directors::DevToolsDirector::new(
                input_engine,
                audio_engine,
                Rc::new(asset_mgr),
            ))
    }

    fn on_updating(&mut self, _app: &mut Application<ApplicationCallbacks>, _delta_sec: f32) {}
}

impl ApplicationCallbacks {
    pub fn new() -> Self {
        ApplicationCallbacks {}
    }
}
