use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::resource_manager::ResourceManager;
use imgui::Ui;
use radiance::audio::Codec;
use radiance::scene::Scene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandMusic {
    res_man: Rc<ResourceManager>,
    data: Option<Vec<u8>>,
}

impl SceCommand for SceCommandMusic {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let data = self.data.take().unwrap();
        state.bgm_source().play(data, Codec::Mp3, true);

        true
    }
}

impl SceCommandMusic {
    pub fn new(res_man: &Rc<ResourceManager>, name: &str) -> Self {
        Self {
            res_man: res_man.clone(),
            data: Some(res_man.load_music_data(name)),
        }
    }
}
