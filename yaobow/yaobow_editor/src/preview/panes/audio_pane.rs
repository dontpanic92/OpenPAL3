use crate::directors::DevToolsState;

use super::ContentPane;
use audio::{AudioSourceState, Codec};
use radiance::audio::{self, AudioEngine, AudioSource};
use std::path::PathBuf;

pub struct AudioPane {
    source: Box<dyn AudioSource>,
    path: PathBuf,
}

impl AudioPane {
    pub fn new(audio_engine: &dyn AudioEngine, data: Vec<u8>, codec: Codec, path: PathBuf) -> Self {
        let mut source = audio_engine.create_source();
        source.play(data, codec, true);

        Self { source, path }
    }

    pub fn toggle_play_stop(&mut self) {
        if self.source.state() == AudioSourceState::Playing {
            self.source.pause();
        } else {
            self.source.resume();
        }
    }
}

impl ContentPane for AudioPane {
    fn render(&mut self, ui: &imgui::Ui) -> Option<DevToolsState> {
        self.source.update();

        ui.text(format!("Audio: {}", self.path.to_str().unwrap()));
        let label = if self.source.state() == AudioSourceState::Playing {
            "Pause"
        } else {
            "Play"
        };

        if ui.button(label) {
            self.toggle_play_stop();
        }

        None
    }
}
