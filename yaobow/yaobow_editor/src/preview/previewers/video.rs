use std::{path::Path, rc::Rc};

use common::store_ext::StoreExt2;
use mini_fs::MiniFs;
use radiance::{rendering::ComponentFactory, video::Codec as VideoCodec};

use crate::{directors::main_content::ContentTab, preview::panes::VideoPane};

use super::Previewer;

pub struct VideoPreviewer {
    factory: Rc<dyn ComponentFactory>,
}

impl VideoPreviewer {
    pub fn new(factory: Rc<dyn ComponentFactory>) -> Self {
        Self { factory }
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

        let data = vfs.read_to_end(path).ok()?;

        Some(ContentTab::new(
            "video".to_string(),
            Box::new(VideoPane::new(
                self.factory.clone(),
                data,
                Some(codec),
                path.to_owned(),
            )),
        ))
    }
}
