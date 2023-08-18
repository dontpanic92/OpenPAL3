use std::{io::BufReader, path::Path, rc::Rc};

use mini_fs::{MiniFs, StoreExt};
use radiance::{audio::AudioEngine, rendering::ComponentFactory, video::Codec as VideoCodec};

use crate::{directors::main_content::ContentTab, preview::panes::VideoPane};

use super::Previewer;

pub struct VideoPreviewer {
    factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
}

impl VideoPreviewer {
    pub fn new(factory: Rc<dyn ComponentFactory>, audio_engine: Rc<dyn AudioEngine>) -> Self {
        Self {
            factory,
            audio_engine,
        }
    }
}

impl Previewer for VideoPreviewer {
    fn open(&self, vfs: &MiniFs, path: &Path) -> Option<ContentTab> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())?;

        let codec = match extension.as_str() {
            "bik" => Some(VideoCodec::Bik),
            _ => None,
        }?;

        let reader = Box::new(BufReader::new(vfs.open(path).ok()?));

        Some(ContentTab::new(
            "video".to_string(),
            Box::new(VideoPane::new(
                self.factory.clone(),
                self.audio_engine.clone(),
                reader,
                Some(codec),
                path.to_owned(),
            )),
        ))
    }
}
