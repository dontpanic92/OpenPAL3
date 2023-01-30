use std::{path::Path, rc::Rc};

use common::store_ext::StoreExt2;
use image::ImageFormat;
use mini_fs::MiniFs;
use radiance::rendering::ComponentFactory;

use crate::{directors::main_content::ContentTab, preview::panes::ImagePane};

use super::Previewer;

pub struct ImagePreviewer {
    factory: Rc<dyn ComponentFactory>,
}

impl ImagePreviewer {
    pub fn new(factory: Rc<dyn ComponentFactory>) -> Self {
        Self { factory }
    }
}

impl Previewer for ImagePreviewer {
    fn open(&self, vfs: &MiniFs, path: &Path) -> Option<ContentTab> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())?;

        match extension.as_str() {
            "tga" | "png" | "dds" => {}
            _ => None?,
        }

        let image = vfs
            .read_to_end(&path)
            .ok()
            .and_then(|b| {
                image::load_from_memory(&b)
                    .or_else(|_| image::load_from_memory_with_format(&b, ImageFormat::Tga))
                    .or_else(|err| Err(err))
                    .ok()
            })
            .and_then(|img| Some(img.to_rgba8()));

        Some(ContentTab::new(
            path.to_string_lossy().to_string(),
            Box::new(ImagePane::new(self.factory.clone(), image)),
        ))
    }
}
