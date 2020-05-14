use crate::asset_manager::AssetManager;
use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::Ui;
use radiance::audio::Codec;
use radiance::scene::CoreScene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandMusic {
    name: String,
}

impl SceCommand for SceCommandMusic {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
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
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}
