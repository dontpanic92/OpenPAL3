use crate::{
    scripting::sce::{SceCommand, SceState},
    utils::play_movie,
};

use crosscom::ComRc;
use imgui::{TextureId, Ui};
use log::warn;
use radiance::{comdef::ISceneManager, input::Key, video::VideoStreamState};

#[derive(Debug, Clone)]
pub struct SceCommandMovie {
    name: String,
    remove_black_bars: bool,
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
        let source_size = if let Some(size) = self.source_size {
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
        let movie_skipped = state.input().get_key_state(Key::Escape).pressed()
            || state.input().get_key_state(Key::GamePadSouth).pressed();

        let global_state_mut = state.global_state_mut();
        let video_player = global_state_mut.video_player();
        if movie_skipped {
            video_player.stop();
            return true;
        }
        if video_player.get_state() == VideoStreamState::Stopped {
            return true;
        }

        self.texture_id = play_movie(
            ui,
            video_player,
            self.texture_id,
            source_size,
            self.remove_black_bars,
        );

        false
    }
}

impl SceCommandMovie {
    pub fn new(name: String) -> Self {
        let remove_black_bars = MOVIES_CONTAIN_BLACK_BARS
            .iter()
            .any(|&n| name.to_lowercase().as_str() == n);

        Self {
            name,
            remove_black_bars,
            source_size: None,
            texture_id: None,
        }
    }
}

const MOVIES_CONTAIN_BLACK_BARS: &[&str; 1] = &["pal3op"];
