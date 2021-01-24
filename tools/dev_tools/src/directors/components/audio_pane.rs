use crate::directors::DevToolsState;

use super::ContentPane;
use audio::{AudioSourceState, Codec};
use imgui::im_str;
use radiance::audio::{self, AudioEngine, AudioSource};
use std::path::PathBuf;

pub struct AudioPane {
    source: Option<Box<dyn AudioSource>>,
    path: PathBuf,
}

impl AudioPane {
    pub fn new(
        audio_engine: &dyn AudioEngine,
        data: Vec<u8>,
        codec: Option<Codec>,
        path: PathBuf,
    ) -> Self {
        let source = codec.map(|c| {
            let mut source = audio_engine.create_source();
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

            ui.text(im_str!("Audio: {}", self.path.to_str().unwrap()));
            let label = if source.state() == AudioSourceState::Playing {
                im_str!("Pause")
            } else {
                im_str!("Play")
            };

            if ui.button(label, [80., 32.]) {
                self.toggle_play_stop();
            }
        } else {
            ui.text(im_str!("Audio format not supported"));
        }

        None
    }
}
