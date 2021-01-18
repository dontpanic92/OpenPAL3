use radiance::scene::{CoreScene, SceneExtension};

pub struct DevToolsScene {}

impl SceneExtension for DevToolsScene {
    fn on_loading(self: &mut CoreScene<Self>) {}
    fn on_updating(self: &mut CoreScene<Self>, delta_sec: f32) {}
}

impl DevToolsScene {
    pub fn new() -> Self {
        Self {}
    }
}
