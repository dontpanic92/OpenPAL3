use crate::{
    scripting::sce::{SceCommand, SceState},
    utils::show_video_window,
};

use crosscom::ComRc;
use imgui::{TextureId, Ui};
use log::warn;
use radiance::{comdef::ISceneManager, input::Key, video::VideoStreamState};

#[derive(Debug, Clone)]
pub struct SceCommandMovie {
    name: String,
    source_size: Option<(u32, u32)>,
    texture_id: Option<TextureId>,
}

impl SceCommand for SceCommandMovie {
    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        state.global_state_mut().set_adv_input_enabled(false);
        state.global_state_mut().bgm_source().stop();
    }

    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let (source_w, source_h) = if let Some(size) = self.source_size {
            size
        } else {
            match state.global_state_mut().play_movie(&self.name) {
                Some(size) => {
                    self.source_size = Some(size);
                    size
                }
                None => {
                    warn!("Skip movie '{}'", self.name);
                    return true;
                }
            }
        };

        // check state to stop movie
        let movie_skipped = state.input().get_key_state(Key::Escape).pressed();
        let global_state_mut = state.global_state_mut();
        let video_player = global_state_mut.video_player();
        if movie_skipped {
            video_player.stop();
            return true;
        }
        if video_player.get_state() == VideoStreamState::Stopped {
            return true;
        }

        let window_size = ui.io().display_size;

        // Keep aspect ratio
        let w_scale = window_size[0] / source_w as f32;
        let h_scale = window_size[1] / source_h as f32;
        let scale = w_scale.min(h_scale);
        let target_size = [source_w as f32 * scale, source_h as f32 * scale];

        self.texture_id =
            show_video_window(ui, video_player, self.texture_id, window_size, target_size);

        false
    }
}

impl SceCommandMovie {
    pub fn new(name: String) -> Self {
        Self {
            name,
            source_size: None,
            texture_id: None,
        }
    }
}
