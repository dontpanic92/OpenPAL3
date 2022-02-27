use radiance::scene::{CoreScene, SceneExtension};

pub struct EditorScene {}

impl SceneExtension for EditorScene {
    fn on_loading(self: &mut CoreScene<Self>) {}
    fn on_updating(self: &mut CoreScene<Self>, _delta_sec: f32) {}
}

impl EditorScene {
    pub fn new() -> Self {
        Self {}
    }
}
