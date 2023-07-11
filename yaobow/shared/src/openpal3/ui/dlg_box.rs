use std::rc::Rc;

use common::store_ext::StoreExt2;
use imgui::{Condition, Ui};
use radiance::rendering::Sprite;

use crate::openpal3::asset_manager::AssetManager;

pub struct DialogBox {
    dialog_pic: Vec<Sprite>,
    click_indicator_pic: Vec<Sprite>,
    click_indicator_interval: f32,
    asset_mgr: Rc<AssetManager>,
    avator: Option<Sprite>,
    avator_at_right: bool,
}

impl DialogBox {
    const DLG_HEIGHT_FACTOR: f32 = 0.25;
    const DLG_Y_POSITION_FACTOR: f32 = 1. - Self::DLG_HEIGHT_FACTOR;
    const DLG_AVATOR_WIDTH_FACTOR: f32 = 1. / 3.;
    const DLG_CLICK_INDICATOR_PADDING_FACTOR: f32 = 1.4;

    // hard code for decoration at bottom-center
    const DLG_DECORATION_WIDTH_TRIM: f32 = 66.;
    const DLG_DECORATION_HEIGHT_TRIM: f32 = 32.;
    const DLG_DECORATION_BOTTOM_OVER_DIALOG: f32 = 6.;

    pub fn new(asset_mgr: Rc<AssetManager>) -> Self {
        // dialog box resources
        let mut dialog_pic = Vec::new();
        for i in 0..9 {
            dialog_pic.push(Self::load_sprite(&format!("/basedata/basedata/ui/flex/dlg{}.tga", i), asset_mgr.as_ref()));
        }
        // a small decoration at bottom-center
        dialog_pic.push(Self::load_sprite("/basedata/basedata/ui/scene/timeclose.tga", asset_mgr.as_ref()));

        // click indicator at bottom-right of the dialog box
        let mut click_indicator_pic = Vec::new();
        for i in 0..2 {
            click_indicator_pic.push(Self::load_sprite(&format!("/basedata/basedata/ui/scene/page{}.tga", i), asset_mgr.as_ref()),)
        }

        Self {
            dialog_pic,
            click_indicator_pic,
            click_indicator_interval: 0.0,
            asset_mgr: asset_mgr.clone(),
            avator: None,
            avator_at_right: false,
        }
    }

    pub fn set_avator(&mut self, face_name: &str, left_or_right: i32) {
        let role_id = face_name[..3].to_string();
        let path = format!("/basedata/basedata/ROLE/{}/{}.tga", role_id, face_name);
        self.avator = Some(Self::load_sprite(&path, self.asset_mgr.as_ref()));
        self.avator_at_right = left_or_right == 1;
    }

    pub fn clear_avator(&mut self) {
        self.avator = None;
    }

    pub fn fade_window(&mut self, ui: &Ui, fade_to_white: bool, opacity: f32) {
        let window_size = ui.io().display_size;
        ui.window("fade")
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .draw_background(false)
            .no_decoration()
            .size(window_size, Condition::Appearing)
            .position(window_size, Condition::Appearing)
            .build(|| {
                let list = ui.get_background_draw_list();
                let mut color = if fade_to_white {
                    imgui::ImColor32::WHITE
                } else {
                    imgui::ImColor32::BLACK
                };

                color.a = (255. * opacity) as u8;
                list.add_rect(
                    [0., 0.],
                    window_size,
                    color
                )
                .filled(true)
                .build();
            });
    }

    pub fn draw(&mut self, text: &str, ui: &Ui, delta_sec: f32) {
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
        let dialog_y = window_height * Self::DLG_Y_POSITION_FACTOR - Self::DLG_DECORATION_BOTTOM_OVER_DIALOG;
        let avator_width = window_height * Self::DLG_AVATOR_WIDTH_FACTOR;
        let click_indicator_right_offset = if self.avator_at_right && self.avator.is_some() {
            // avator has set and avator is at right side
            avator_width
        } else {
            0.
        };

        // FIXME: seem not a good method to get font size
        let font = ui.fonts().fonts()[1];
        let font_size = ui.fonts().get_font(font).unwrap().font_size;

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
                    dialog_x + self.dialog_pic[0].width() as f32,
                    dialog_y + self.dialog_pic[0].height() as f32,
                );

                let bottom_left_inner = (
                    dialog_x + self.dialog_pic[1].width() as f32,
                    dialog_y + dialog_height - self.dialog_pic[1].height() as f32,
                );

                let bottom_right_inner = (
                    dialog_x + dialog_width - self.dialog_pic[2].width() as f32,
                    dialog_y + dialog_height - self.dialog_pic[2].height() as f32,
                );

                let top_right_inner = (
                    dialog_x + dialog_width - self.dialog_pic[3].width() as f32,
                    dialog_y + self.dialog_pic[3].height() as f32,
                );

                let decoration_min = (
                    dialog_x + dialog_width / 2.0 - Self::DLG_DECORATION_WIDTH_TRIM,
                    window_height - Self::DLG_DECORATION_HEIGHT_TRIM
                );

                let click_indicator_min = [
                    dialog_x + dialog_width - self.click_indicator_pic[0].width() as f32 * Self::DLG_CLICK_INDICATOR_PADDING_FACTOR - click_indicator_right_offset,
                    dialog_y + dialog_height - self.click_indicator_pic[0].height() as f32 * Self::DLG_CLICK_INDICATOR_PADDING_FACTOR
                ];

                let list = ui.get_background_draw_list();

                // top-left
                list.add_image(
                    self.dialog_pic[0].imgui_texture_id(),
                    [dialog_x, dialog_y],
                    [top_left_inner.0, top_left_inner.1],
                )
                .build();

                // bottom-left
                list.add_image(
                    self.dialog_pic[1].imgui_texture_id(),
                    [dialog_x, bottom_left_inner.1],
                    [bottom_left_inner.0, dialog_y + dialog_height],
                )
                .build();

                // bottom-right
                list.add_image(
                    self.dialog_pic[2].imgui_texture_id(),
                    [bottom_right_inner.0, bottom_right_inner.1],
                    [dialog_x + dialog_width, dialog_y + dialog_height],
                )
                .build();

                // top-right
                list.add_image(
                    self.dialog_pic[3].imgui_texture_id(),
                    [top_right_inner.0, dialog_y],
                    [dialog_x + dialog_width, top_right_inner.1],
                )
                .build();

                // left
                list.add_image(
                    self.dialog_pic[4].imgui_texture_id(),
                    [dialog_x, top_left_inner.1],
                    [dialog_x + self.dialog_pic[4].width() as f32, bottom_left_inner.1],
                )
                .build();

                // bottom
                list.add_image(
                    self.dialog_pic[5].imgui_texture_id(),
                    [
                        bottom_left_inner.0,
                        dialog_y + dialog_height - self.dialog_pic[5].height() as f32,
                    ],
                    [bottom_right_inner.0, dialog_y + dialog_height],
                )
                .build();

                // right
                list.add_image(
                    self.dialog_pic[6].imgui_texture_id(),
                    [
                        dialog_x + dialog_width - self.dialog_pic[6].width() as f32,
                        top_right_inner.1,
                    ],
                    [dialog_x + dialog_width, bottom_right_inner.1],
                )
                .build();

                // top
                list.add_image(
                    self.dialog_pic[7].imgui_texture_id(),
                    [top_left_inner.0, dialog_y],
                    [top_right_inner.0, dialog_y + self.dialog_pic[7].height() as f32],
                )
                .build();

                // center
                list.add_image(
                    self.dialog_pic[8].imgui_texture_id(),
                    [top_left_inner.0, top_left_inner.1],
                    [bottom_right_inner.0, bottom_right_inner.1],
                )
                .build();

                // decoration at bottom-center
                list.add_image(
                    self.dialog_pic[9].imgui_texture_id(),
                    [decoration_min.0, decoration_min.1],
                    [decoration_min.0 + self.dialog_pic[9].width() as f32, decoration_min.1 + self.dialog_pic[9].height() as f32]
                )
                .build();

                // avator
                if let Some(avator) = &self.avator {
                    let avator_height = avator_width / avator.width() as f32 * avator.height() as f32;
                    let (min, max) = if !self.avator_at_right {
                        (
                            [dialog_x + avator_width, window_height - avator_height],
                            [dialog_x, window_height]
                        )
                    } else {
                        (
                            [dialog_x + dialog_width - avator_width, window_height - avator_height],
                            [dialog_x + dialog_width, window_height]
                        )
                    };
                    list.add_image(
                        avator.imgui_texture_id(),
                        min,
                        max
                    )
                    .build();
                }

                // draw click indicator
                self.click_indicator_interval += delta_sec;
                if self.click_indicator_interval > 1.0 {
                    self.click_indicator_interval = 0.0;
                }
                list.add_image(
                    if self.click_indicator_interval > 0.5 {
                        self.click_indicator_pic[0].imgui_texture_id()
                    } else {
                        self.click_indicator_pic[1].imgui_texture_id()
                    },
                    [click_indicator_min[0], click_indicator_min[1]],
                    [click_indicator_min[0] + self.click_indicator_pic[0].width() as f32, click_indicator_min[1] + self.click_indicator_pic[0].width() as f32]
                ).build();

                // draw text
                let mut text_x = 0.;
                let mut text_y = 0.;
                let mut text_color = imgui::ImColor32::WHITE;
                for c in text.chars() {
                    if dialog_width - avator_width * 2. - text_x < 0. {
                        text_y += font_size * 1.5;
                        text_x = 0.;
                    }
                    match c {
                        '\n' => {
                            if text_x == 0. {
                                // already at the beginning, don't change to a new line
                                continue;
                            }
                            text_y += font_size * 1.5;
                            text_x = 0.;
                            continue;
                        }
                        '\\' => {
                            continue;
                        }
                        'i' => {
                            // set color to YELLOW
                            text_color = imgui::ImColor32::from_rgb(0xff, 0xff, 0x00);
                        }
                        'r' => {
                            // set color to default
                            text_color = imgui::ImColor32::WHITE;
                        }
                        _ => {
                            list.add_text([dialog_x + avator_width + text_x, dialog_y + 18. + text_y], text_color ,&c.to_string());
                            text_x += font_size * 1.1;
                        }
                    }
                }
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
