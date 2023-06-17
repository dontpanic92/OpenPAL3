use std::rc::Rc;

use common::store_ext::StoreExt2;
use imgui::{Condition, Ui};
use radiance::rendering::Sprite;

use crate::openpal3::asset_manager::AssetManager;

pub struct DialogBox {
    top_left: Sprite,
    bottom_left: Sprite,
    bottom_right: Sprite,
    top_right: Sprite,
    left: Sprite,
    bottom: Sprite,
    right: Sprite,
    top: Sprite,
    background: Sprite,
}

impl DialogBox {
    const DLG_HEIGHT_FACTOR: f32 = 0.25;
    const DLG_Y_POSITION_FACTOR: f32 = 1. - Self::DLG_HEIGHT_FACTOR;

    pub fn new(asset_mgr: Rc<AssetManager>) -> Self {
        let top_left = Self::load_sprite("/basedata/basedata/ui/flex/dlg0.tga", asset_mgr.as_ref());
        let bottom_left =
            Self::load_sprite("/basedata/basedata/ui/flex/dlg1.tga", asset_mgr.as_ref());
        let bottom_right =
            Self::load_sprite("/basedata/basedata/ui/flex/dlg2.tga", asset_mgr.as_ref());
        let top_right =
            Self::load_sprite("/basedata/basedata/ui/flex/dlg3.tga", asset_mgr.as_ref());
        let left = Self::load_sprite("/basedata/basedata/ui/flex/dlg4.tga", asset_mgr.as_ref());
        let bottom = Self::load_sprite("/basedata/basedata/ui/flex/dlg5.tga", asset_mgr.as_ref());
        let right = Self::load_sprite("/basedata/basedata/ui/flex/dlg6.tga", asset_mgr.as_ref());
        let top = Self::load_sprite("/basedata/basedata/ui/flex/dlg7.tga", asset_mgr.as_ref());
        let background =
            Self::load_sprite("/basedata/basedata/ui/flex/dlg8.tga", asset_mgr.as_ref());

        Self {
            top_left,
            bottom_left,
            bottom_right,
            top_right,
            left,
            bottom,
            right,
            top,
            background,
        }
    }

    pub fn draw(&mut self, text: &str, ui: &Ui, _delta_sec: f32) {
        let [window_width, window_height] = ui.io().display_size;
        let (dialog_x, dialog_width) = {
            if window_width / window_height > 4. / 3. {
                let dialog_width = window_height / 3. * 4.;
                let dialog_x = (window_width - dialog_width) / 2.;
                (dialog_x, dialog_width)
            } else {
                (0., window_width)
            }
        };

        let dialog_height = window_height * Self::DLG_HEIGHT_FACTOR;
        let dialog_y = window_height * Self::DLG_Y_POSITION_FACTOR;

        ui.window("dlg_box")
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .draw_background(false)
            .no_decoration()
            .size([dialog_width, dialog_height], Condition::Appearing)
            .position([dialog_x, dialog_y], Condition::Appearing)
            .build(|| {
                let top_left_inner = (
                    dialog_x + self.top_left.width() as f32,
                    dialog_y + self.top_left.height() as f32,
                );

                let bottom_left_inner = (
                    dialog_x + self.bottom_left.width() as f32,
                    dialog_y + dialog_height - self.bottom_left.height() as f32,
                );

                let bottom_right_inner = (
                    dialog_x + dialog_width - self.bottom_right.width() as f32,
                    dialog_y + dialog_height - self.bottom_right.height() as f32,
                );

                let top_right_inner = (
                    dialog_x + dialog_width - self.top_right.width() as f32,
                    dialog_y + self.top_right.height() as f32,
                );

                let list = ui.get_background_draw_list();

                list.add_image(
                    self.top_left.imgui_texture_id(),
                    [dialog_x, dialog_y],
                    [top_left_inner.0, top_left_inner.1],
                )
                .build();

                list.add_image(
                    self.bottom_left.imgui_texture_id(),
                    [dialog_x, bottom_left_inner.1],
                    [bottom_left_inner.0, dialog_y + dialog_height],
                )
                .build();

                list.add_image(
                    self.bottom_right.imgui_texture_id(),
                    [bottom_right_inner.0, bottom_right_inner.1],
                    [dialog_x + dialog_width, dialog_y + dialog_height],
                )
                .build();

                list.add_image(
                    self.top_right.imgui_texture_id(),
                    [top_right_inner.0, dialog_y],
                    [dialog_x + dialog_width, top_right_inner.1],
                )
                .build();

                list.add_image(
                    self.left.imgui_texture_id(),
                    [dialog_x, top_left_inner.1],
                    [dialog_x + self.left.width() as f32, bottom_left_inner.1],
                )
                .build();

                list.add_image(
                    self.bottom.imgui_texture_id(),
                    [
                        bottom_left_inner.0,
                        dialog_y + dialog_height - self.bottom.height() as f32,
                    ],
                    [bottom_right_inner.0, dialog_y + dialog_height],
                )
                .build();

                list.add_image(
                    self.right.imgui_texture_id(),
                    [
                        dialog_x + dialog_width - self.right.width() as f32,
                        top_right_inner.1,
                    ],
                    [dialog_x + dialog_width, bottom_right_inner.1],
                )
                .build();

                list.add_image(
                    self.top.imgui_texture_id(),
                    [top_left_inner.0, dialog_y],
                    [top_right_inner.0, dialog_y + self.top.height() as f32],
                )
                .build();

                list.add_image(
                    self.background.imgui_texture_id(),
                    [top_left_inner.0, top_left_inner.1],
                    [bottom_right_inner.0, bottom_right_inner.1],
                )
                .build();
                ui.text_wrapped(text);
            });
    }

    fn load_sprite(path: &str, asset_mgr: &AssetManager) -> Sprite {
        Sprite::load_from_buffer(
            &asset_mgr.vfs().read_to_end(path).unwrap(),
            image::ImageFormat::Tga,
            asset_mgr.component_factory().as_ref(),
        )
    }
}
