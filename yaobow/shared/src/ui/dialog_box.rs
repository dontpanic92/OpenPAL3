use std::rc::Rc;

use imgui::{Condition, Ui};
use radiance::radiance::UiManager;

use crate::openpal4::asset_loader::ImageSetImage;

pub struct DialogBox {
    ui: Rc<UiManager>,
    avatar: Option<ImageSetImage>,
    avatar_position: AvatarPosition,
    height_factor: f32,
    text: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AvatarPosition {
    Left,
    Right,
}

impl DialogBox {
    pub fn new(ui: Rc<UiManager>) -> Self {
        Self {
            ui,
            avatar: None,
            avatar_position: AvatarPosition::Left,
            height_factor: 0.2,
            text: "".to_string(),
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.replacen("：", "：\n", 1);
    }

    pub fn set_avatar(&mut self, avatar: Option<ImageSetImage>, position: AvatarPosition) {
        self.avatar = avatar;
        self.avatar_position = position;
    }
}

pub struct DialogBoxPresenter;

impl DialogBoxPresenter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&self, dialog_box: &DialogBox, _delta_sec: f32) {
        let ui = dialog_box.ui.ui();

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

        let dialog_height = window_height * dialog_box.height_factor;
        let dialog_y = window_height * (1. - dialog_box.height_factor);

        let avatar_size = if let Some(avatar) = &dialog_box.avatar {
            let height = dialog_height * 1.8;
            let width = height * avatar.width as f32 / avatar.height as f32;
            [width, height]
        } else {
            [0., 0.]
        };

        let text_margins = if dialog_box.avatar_position == AvatarPosition::Left {
            (avatar_size[0] + 10., 10.)
        } else {
            (10., avatar_size[0] + 10.)
        };

        basic_dlg_box(ui, "dlg_box")
            .draw_background(true)
            .position([dialog_x, dialog_y], Condition::Always)
            .size([dialog_width, dialog_height], Condition::Always)
            .build(|| {
                let _ = ui.push_text_wrap_pos_with_pos(dialog_width - text_margins.1);
                ui.set_cursor_pos([text_margins.0, 0.]);
                ui.text_wrapped(&dialog_box.text);
            });

        if let Some(avatar) = &dialog_box.avatar {
            let avatar_position = if dialog_box.avatar_position == AvatarPosition::Left {
                [dialog_x, window_height - avatar_size[1]]
            } else {
                [
                    dialog_x + dialog_width - avatar_size[0],
                    window_height - avatar_size[1],
                ]
            };

            let mut uv0 = [
                avatar.x as f32 / avatar.sprite.width() as f32,
                avatar.y as f32 / avatar.sprite.height() as f32,
            ];
            let mut uv1 = [
                (avatar.x + avatar.width) as f32 / avatar.sprite.width() as f32,
                (avatar.y + avatar.height) as f32 / avatar.sprite.height() as f32,
            ];

            if dialog_box.avatar_position == AvatarPosition::Right {
                std::mem::swap(&mut uv0[0], &mut uv1[0]);
            }

            let _tok = ui.push_style_var(imgui::StyleVar::WindowPadding([0., 0.]));
            basic_dlg_box(ui, "avatar_box")
                .draw_background(false)
                .position(avatar_position, Condition::Always)
                .size(avatar_size, Condition::Always)
                .build(|| {
                    imgui::Image::new(avatar.sprite.imgui_texture_id(), avatar_size)
                        .uv0(uv0)
                        .uv1(uv1)
                        .build(ui);
                });
        }
    }
}

fn basic_dlg_box<'a>(ui: &'a Ui, name: &'static str) -> imgui::Window<'a, 'a, &'static str> {
    ui.window(name)
        .collapsible(false)
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .focused(false)
        .no_decoration()
}
