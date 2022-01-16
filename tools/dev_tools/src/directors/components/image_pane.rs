use crate::directors::DevToolsState;

use super::ContentPane;
use image::RgbaImage;
use imgui::{Image, TextureId};
use radiance::rendering::{ComponentFactory, Texture};
use std::rc::Rc;

pub struct ImagePane {
    image: Option<RgbaImage>,
    factory: Rc<dyn ComponentFactory>,
    texture_id: Option<TextureId>,
    texture: Option<Box<dyn Texture>>,
}

impl ImagePane {
    pub fn new(factory: Rc<dyn ComponentFactory>, image: Option<RgbaImage>) -> Self {
        Self {
            image,
            factory: factory.clone(),
            texture_id: None,
            texture: None,
        }
    }
}

impl ContentPane for ImagePane {
    fn render(&mut self, ui: &imgui::Ui) -> Option<DevToolsState> {
        if let Some(image) = self.image.as_ref() {
            let w = image.width();
            let h = image.height();
            let [avail_width, avail_height] = ui.content_region_avail();
            let (w_scale, h_scale) = (avail_width / w as f32, avail_height / h as f32);
            let scale = w_scale.min(h_scale);
            let target_size = [w as f32 * scale, h as f32 * scale];

            let (texture, texture_id) =
                self.factory
                    .create_imgui_texture(image.as_raw(), 0, w, h, self.texture_id);

            self.texture = Some(texture);
            self.texture_id = Some(texture_id);
            ui.set_cursor_pos([
                ui.cursor_pos()[0] + (avail_width - target_size[0]) * 0.5,
                ui.cursor_pos()[1] + (avail_height - target_size[1]) * 0.5,
            ]);
            Image::new(texture_id, target_size).build(ui);
        } else {
            ui.text("Unable to load image");
        }

        None
    }
}
