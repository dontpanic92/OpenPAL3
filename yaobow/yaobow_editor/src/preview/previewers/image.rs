use std::{path::Path, rc::Rc};

use byteorder::{LittleEndian, ReadBytesExt};
use common::store_ext::StoreExt2;
use image::ImageFormat;
use mini_fs::MiniFs;
use radiance::rendering::ComponentFactory;

use crate::{directors::main_content::ContentTab, preview::panes::ImagePane, GameType};

use super::Previewer;

pub struct ImagePreviewer {
    factory: Rc<dyn ComponentFactory>,
    game_type: GameType,
}

impl ImagePreviewer {
    pub fn new(factory: Rc<dyn ComponentFactory>, game_type: GameType) -> Self {
        Self { factory, game_type }
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
            .map_err(|err| {
                log::error!("failed to read image: {:?}", err);
            })
            .ok()
            .and_then(|b| match (extension.as_str(), self.game_type) {
                ("png", GameType::SWD5 | GameType::SWDCF | GameType::SWDHC) => {
                    let width = (&b[0..4]).read_u32::<LittleEndian>().unwrap();
                    let height = (&b[4..8]).read_u32::<LittleEndian>().unwrap();
                    let data = &b[8..];
                    image::RgbaImage::from_raw(width, height, data.to_vec())
                        .map(|img| image::DynamicImage::ImageRgba8(img))
                }
                _ => image::load_from_memory(&b)
                    .or_else(|_| image::load_from_memory_with_format(&b, ImageFormat::Tga))
                    .or_else(|err| Err(err))
                    .ok(),
            })
            .and_then(|img| Some(img.to_rgba8()));

        Some(ContentTab::new(
            path.to_string_lossy().to_string(),
            Box::new(ImagePane::new(self.factory.clone(), image)),
        ))
    }
}
