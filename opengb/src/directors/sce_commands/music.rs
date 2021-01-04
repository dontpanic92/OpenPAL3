use crate::directors::sce_director::SceCommand;
use crate::directors::sce_state::SceState;
use imgui::Ui;
use radiance::{audio::Codec, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandMusic {
    name: String,
}

impl SceCommand for SceCommandMusic {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let data = state.asset_mgr().load_music_data(&self.name);
        state.bgm_source().play(data, Codec::Mp3, true);
        true
    }
}

impl SceCommandMusic {
    pub fn new(name: String, unknown: i32) -> Self {
        Self { name }
    }
}
