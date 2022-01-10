use crate::directors::DevToolsState;

use super::ContentPane;
use imgui::{Image, TextureId};
use radiance::{
    rendering::{ComponentFactory, VideoPlayer},
    video::Codec,
    video::VideoStreamState,
};
use std::{path::PathBuf, rc::Rc};

pub struct VideoPane {
    video_player: Option<Box<VideoPlayer>>,
    source_size: Option<(u32, u32)>,
    texture_id: Option<TextureId>,
    path: PathBuf,
}

impl VideoPane {
    pub fn new(
        factory: Rc<dyn ComponentFactory>,
        data: Vec<u8>,
        codec: Option<Codec>,
        path: PathBuf,
    ) -> Self {
        let mut source_size = None;
        let video_player = codec.map(|c| {
            let mut video_player = factory.create_video_player();
            source_size = video_player.play(factory, data, c, true);
            video_player
        });

        Self {
            video_player,
            source_size,
            texture_id: None,
            path,
        }
    }

    pub fn toggle_play_stop(&mut self) {
        if let Some(video_player) = &mut self.video_player {
            if video_player.get_state() == VideoStreamState::Playing {
                video_player.pause();
            } else {
                video_player.resume();
            }
        }
    }
}

impl ContentPane for VideoPane {
    fn render(&mut self, ui: &imgui::Ui) -> Option<DevToolsState> {
        if let Some((w, h)) = self.source_size {
            ui.text(format!("Video: {}", self.path.to_str().unwrap()));
            let video_player = self.video_player.as_ref().unwrap();
            let label = if video_player.get_state() == VideoStreamState::Playing {
                "Pause"
            } else {
                "Play"
            };

            if ui.button(label) {
                self.toggle_play_stop();
            }

            let [avail_width, avail_height] = ui.content_region_avail();
            let (w_scale, h_scale) = (avail_width / w as f32, avail_height / h as f32);
            let scale = w_scale.min(h_scale);
            let target_size = [w as f32 * scale, h as f32 * scale];

            if let Some(texture_id) = self
                .video_player
                .as_ref()
                .unwrap()
                .get_texture(self.texture_id)
            {
                self.texture_id = Some(texture_id);
                ui.set_cursor_pos([
                    ui.cursor_pos()[0] + (avail_width - target_size[0]) * 0.5,
                    ui.cursor_pos()[1] + (avail_height - target_size[1]) * 0.5,
                ]);
                Image::new(texture_id, target_size).build(ui);
            }
        } else {
            ui.text("Video format not supported");
        }

        None
    }
}
