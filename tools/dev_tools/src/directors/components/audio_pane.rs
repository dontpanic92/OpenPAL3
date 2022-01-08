use crate::directors::DevToolsState;

use super::ContentPane;
use audio::{AudioSourceState, Codec};
use radiance::{
    audio::{self, AudioSource},
    media::MediaEngine,
};
use std::path::PathBuf;

pub struct AudioPane {
    source: Option<Box<dyn AudioSource>>,
    path: PathBuf,
}

impl AudioPane {
    pub fn new(
        media_engine: &dyn MediaEngine,
        data: Vec<u8>,
        codec: Option<Codec>,
        path: PathBuf,
    ) -> Self {
        let source = codec.map(|c| {
            let mut source = media_engine.create_audio_source();
            source.play(data, c, true);
            source
        });

        Self { source, path }
    }

    pub fn toggle_play_stop(&mut self) {
        if let Some(source) = &mut self.source {
            if source.state() == AudioSourceState::Playing {
                source.pause();
            } else {
                source.resume();
            }
        }
    }
}

impl ContentPane for AudioPane {
    fn render(&mut self, ui: &imgui::Ui) -> Option<DevToolsState> {
        if let Some(source) = &mut self.source {
            source.update();

            ui.text(format!("Audio: {}", self.path.to_str().unwrap()));
            let label = if source.state() == AudioSourceState::Playing {
                "Pause"
            } else {
                "Play"
            };

            if ui.button(label) {
                self.toggle_play_stop();
            }
        } else {
            ui.text("Audio format not supported");
        }

        None
    }
}
