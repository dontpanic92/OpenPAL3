use std::{path::Path, rc::Rc};

use common::store_ext::StoreExt2;
use mini_fs::MiniFs;
use radiance::{audio::AudioEngine, audio::Codec as AudioCodec};
use shared::fs::cpk::CpkArchive;

use crate::{directors::main_content::ContentTab, preview::panes::AudioPane};

use super::Previewer;

pub struct AudioPreviewer {
    audio_engine: Rc<dyn AudioEngine>,
}

impl AudioPreviewer {
    pub fn new(audio_engine: Rc<dyn AudioEngine>) -> Self {
        Self { audio_engine }
    }
}

impl Previewer for AudioPreviewer {
    fn open(&self, vfs: &MiniFs, path: &Path) -> Option<ContentTab> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let codec = match extension.as_deref() {
            Some("mp3") => Some(AudioCodec::Mp3),
            Some("smp") => Some(AudioCodec::Mp3),
            Some("wav") => Some(AudioCodec::Wav),
            _ => None,
        };

        if codec.is_none() {
            return None;
        }

        let extension = extension.unwrap();
        let codec = codec.unwrap();

        if let Ok(mut data) = vfs.read_to_end(&path) {
            if extension == "smp" {
                let mut cpk = CpkArchive::load(std::io::Cursor::new(&data)).unwrap();
                let name = cpk.file_names[0].clone();
                let mut content = cpk.open_str(&name).unwrap().content();
                let size = content.len() & 0xFFFFFFFC;
                content.resize(size, 0);

                data = xxtea::decrypt_raw(
                    &content,
                    "Vampire.C.J at Softstar Technology (ShangHai) Co., Ltd",
                );
            }

            Some(ContentTab::new(
                "audio".to_string(),
                Box::new(AudioPane::new(
                    self.audio_engine.as_ref(),
                    data,
                    codec,
                    path.to_owned(),
                )),
            ))
        } else {
            None
        }
    }
}
