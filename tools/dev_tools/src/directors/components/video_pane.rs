use crate::directors::DevToolsState;

use super::ContentPane;
use imgui::{Image, TextureId};
use media::VideoSourceState;
use radiance::{
    media::{self, MediaEngine, VideoSource},
    rendering::ComponentFactory,
};
use std::{path::PathBuf, rc::Rc};

pub struct VideoPane {
    source: Box<dyn VideoSource>,
    source_size: (u32, u32),
    texture_id: Option<TextureId>,
    path: PathBuf,
}

impl VideoPane {
    pub fn new(
        factory: Rc<dyn ComponentFactory>,
        media_engine: &dyn MediaEngine,
        data: Vec<u8>,
        path: PathBuf,
    ) -> Self {
        let mut source = media_engine.create_video_source(factory);
        let source_size = source.play(data, true);

        Self {
            source,
            source_size,
            texture_id: None,
            path,
        }
    }

    pub fn toggle_play_stop(&mut self) {
        if self.source.state() == VideoSourceState::Playing {
            self.source.pause();
        } else {
            self.source.resume();
        }
    }
}

impl ContentPane for VideoPane {
    fn render(&mut self, ui: &imgui::Ui) -> Option<DevToolsState> {
        ui.text(format!("Video: {}", self.path.to_str().unwrap()));
        let label = if self.source.state() == VideoSourceState::Playing {
            "Pause"
        } else {
            "Play"
        };

        if ui.button(label) {
            self.toggle_play_stop();
        }

        let (w, h) = self.source_size;
        let [avail_width, avail_height] = ui.content_region_avail();
        let (w_scale, h_scale) = (avail_width / w as f32, avail_height / h as f32);
        let scale = w_scale.min(h_scale);
        let target_size = [w as f32 * scale, h as f32 * scale];

        self.source.update();
        if let Some(texture_id) = self.source.get_texture(self.texture_id) {
            self.texture_id = Some(texture_id);
            ui.set_cursor_pos([
                ui.cursor_pos()[0] + (avail_width - target_size[0]) * 0.5,
                ui.cursor_pos()[1] + (avail_height - target_size[1]) * 0.5,
            ]);
            Image::new(texture_id, target_size).build(ui);
        }

        None
    }
}
