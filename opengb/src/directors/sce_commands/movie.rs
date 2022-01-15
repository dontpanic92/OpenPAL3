use crate::directors::sce_vm::{SceCommand, SceState};

use imgui::{Condition, Image, TextureId, Ui, Window};
use log::warn;
use radiance::{input::Key, scene::SceneManager, video::VideoStreamState};

#[derive(Debug, Clone)]
pub struct SceCommandMovie {
    name: String,
    source_size: Option<(u32, u32)>,
    texture_id: Option<TextureId>,
}

impl SceCommand for SceCommandMovie {
    fn initialize(&mut self, scene_manager: &mut dyn SceneManager, state: &mut SceState) {
        state.global_state_mut().set_adv_input_enabled(false);
        state.global_state_mut().bgm_source().stop();
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
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
        let window = Window::new(" ")
            .size(window_size, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .always_auto_resize(false)
            .draw_background(false)
            .scrollable(false)
            .no_decoration()
            .movable(false);

        let mut target_size = window_size;
        if cfg!(feature = "movies-keep-aspect-ratio") {
            let w_scale = window_size[0] / source_w as f32;
            let h_scale = window_size[1] / source_h as f32;
            let scale = w_scale.min(h_scale);
            target_size = [source_w as f32 * scale, source_h as f32 * scale];
        }

        window.build(ui, || {
            let video_player = state.global_state_mut().video_player();
            if let Some(texture_id) = video_player.get_texture(self.texture_id) {
                self.texture_id = Some(texture_id);
                ui.set_cursor_pos([
                    (window_size[0] - target_size[0]) * 0.5,
                    (window_size[1] - target_size[1]) * 0.5,
                ]);
                Image::new(texture_id, target_size).build(ui);
            }
        });

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
