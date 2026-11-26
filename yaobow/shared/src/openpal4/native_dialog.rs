use std::{collections::HashMap, rc::Rc};

use anyhow::Context;
use imgui::{MouseButton, Ui};
use radiance::input::{InputEngine, Key};

use crate::{
    loaders::cegui::layout::{self, ImageRef},
    ui::{
        dialog_box::{AvatarPosition, DialogBox, DialogBoxPresenter},
        dialog_markup::{self, Segment},
    },
};

use super::asset_loader::{AssetLoader, ImageSetImage};

type Rect = [f32; 4];

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DialogKind {
    Talk,
    Message,
    Selection,
    Confirmation,
}

struct Widget {
    rect: Rect,
    images: [Option<ImageRef>; 3],
    frame: [Option<ImageRef>; 8],
}

pub(crate) struct DialogArtwork {
    widgets: HashMap<String, Widget>,
    images: HashMap<String, ImageSetImage>,
}

impl DialogArtwork {
    pub(crate) fn load(loader: &AssetLoader) -> anyhow::Result<Self> {
        let mut widgets = HashMap::new();
        for file in ["ScenarioDialog", "CommonDialog", "SelectDialog"] {
            let parsed =
                layout::load_layout(loader.vfs(), &format!("/gamedata/ui/layouts/{file}.xml"))
                    .with_context(|| format!("loading {file}.xml"))?;
            let mut rects: Vec<Rect> = Vec::new();
            for window in &parsed.windows {
                let parent = window
                    .parent
                    .map_or([0., 0., 800., 600.], |id| rects[id as usize]);
                let (x, y, w, h) = window.area.resolve(parent[2], parent[3]);
                let rect = [parent[0] + x, parent[1] + y, w, h];
                rects.push(rect);
                let frame = [
                    "TopLeftFrameImage",
                    "TopFrameImage",
                    "TopRightFrameImage",
                    "LeftFrameImage",
                    "RightFrameImage",
                    "BottomLeftFrameImage",
                    "BottomFrameImage",
                    "BottomRightFrameImage",
                ]
                .map(|name| {
                    window
                        .properties
                        .iter()
                        .find(|p| p.name == name)
                        .and_then(|p| layout::parse_image_ref(&p.value))
                });
                widgets.insert(
                    format!("{file}/{}", window.name),
                    Widget {
                        rect,
                        images: [
                            window
                                .image
                                .clone()
                                .or_else(|| window.overlay_normal_image.clone()),
                            window.overlay_highlight_image.clone(),
                            window.overlay_pushed_image.clone(),
                        ],
                        frame,
                    },
                );
            }
        }
        let mut images = HashMap::new();
        let mut index = crate::loaders::cegui::ui_layout_handle::ImagesetIndex::new(
            "/gamedata/ui/imagesets".into(),
        );
        for set in ["zhujiemian0", "duihuakuang0", "kuang20", "gy0"] {
            let (atlas, png) = index.load(loader.vfs(), set)?;
            let sprite = Rc::new(radiance::rendering::Sprite::load_from_image(
                image::load_from_memory(&png)?.to_rgba8(),
                loader.component_factory().as_ref(),
            ));
            for (name, rect) in atlas.images {
                images.insert(
                    format!("{set}:{name}").to_lowercase(),
                    ImageSetImage {
                        name,
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                        sprite: sprite.clone(),
                    },
                );
            }
        }
        Ok(Self { widgets, images })
    }

    fn rect(&self, name: &str) -> Rect {
        self.widgets.get(name).map_or([0.; 4], |w| w.rect)
    }

    fn image(&self, reference: &ImageRef) -> Option<&ImageSetImage> {
        self.images
            .get(&format!("{}:{}", reference.set, reference.image).to_lowercase())
    }

    fn draw(&self, ui: &Ui, name: &str, rect: Rect, state: usize, canvas: Canvas) {
        let Some(widget) = self.widgets.get(name) else {
            return;
        };
        let Some(image) = widget.images[state]
            .as_ref()
            .or(widget.images[0].as_ref())
            .and_then(|reference| self.image(reference))
        else {
            return;
        };
        if widget.frame.iter().all(Option::is_some) {
            let corners: Vec<_> = widget
                .frame
                .iter()
                .filter_map(|reference| reference.as_ref().and_then(|r| self.image(r)))
                .collect();
            if corners.len() == 8 {
                let left = (corners[0].width as f32).min(rect[2] / 2.);
                let right = (corners[2].width as f32).min(rect[2] / 2.);
                let top = (corners[0].height as f32).min(rect[3] / 2.);
                let bottom = (corners[5].height as f32).min(rect[3] / 2.);
                let xs = [
                    rect[0],
                    rect[0] + left,
                    rect[0] + rect[2] - right,
                    rect[0] + rect[2],
                ];
                let ys = [
                    rect[1],
                    rect[1] + top,
                    rect[1] + rect[3] - bottom,
                    rect[1] + rect[3],
                ];
                let tiles = [
                    corners[0], corners[1], corners[2], corners[3], image, corners[4], corners[5],
                    corners[6], corners[7],
                ];
                for (i, tile) in tiles.into_iter().enumerate() {
                    let (x, y) = (i % 3, i / 3);
                    blit(
                        ui,
                        tile,
                        [xs[x], ys[y], xs[x + 1] - xs[x], ys[y + 1] - ys[y]],
                        canvas,
                        false,
                    );
                }
                return;
            }
        }
        blit(ui, image, rect, canvas, false);
    }
}

#[derive(Clone, Copy)]
struct Canvas {
    x: f32,
    y: f32,
    scale: f32,
}

impl Canvas {
    fn new([width, height]: [f32; 2]) -> Self {
        let scale = (width / 800.).min(height / 600.).max(f32::EPSILON);
        Self {
            x: (width - 800. * scale) / 2.,
            y: (height - 600. * scale) / 2.,
            scale,
        }
    }
    fn point(self, x: f32, y: f32) -> [f32; 2] {
        [self.x + x * self.scale, self.y + y * self.scale]
    }
}

fn blit(ui: &Ui, image: &ImageSetImage, [x, y, w, h]: Rect, canvas: Canvas, mirror: bool) {
    let mut uv0 = [
        image.x as f32 / image.sprite.width() as f32,
        image.y as f32 / image.sprite.height() as f32,
    ];
    let mut uv1 = [
        (image.x + image.width) as f32 / image.sprite.width() as f32,
        (image.y + image.height) as f32 / image.sprite.height() as f32,
    ];
    if mirror {
        std::mem::swap(&mut uv0[0], &mut uv1[0]);
    }
    ui.get_foreground_draw_list()
        .add_image(
            image.sprite.imgui_texture_id(),
            canvas.point(x, y),
            canvas.point(x + w, y + h),
        )
        .uv_min(uv0)
        .uv_max(uv1)
        .build();
}

#[derive(Debug)]
struct Glyph {
    ch: char,
    x: f32,
    y: f32,
    color: [f32; 4],
}

fn paginate(
    segments: &[Segment],
    width: f32,
    height: f32,
    size: f32,
    spacing: f32,
    mut measure: impl FnMut(char) -> f32,
) -> Vec<Vec<Glyph>> {
    let mut pages = vec![Vec::new()];
    let (mut x, mut y) = (0., 0.);
    for segment in segments {
        for ch in segment.text.chars() {
            if ch == '\r' {
                continue;
            }
            if ch == '\n' {
                x = 0.;
                y += size + spacing;
                continue;
            }
            let advance = measure(ch);
            if x > 0. && x + advance > width {
                x = 0.;
                y += size + spacing;
            }
            if y + size > height {
                if !pages.last().unwrap().is_empty() {
                    pages.push(Vec::new());
                }
                x = 0.;
                y = 0.;
            }
            pages.last_mut().unwrap().push(Glyph {
                ch,
                x,
                y,
                color: segment.color,
            });
            x += advance;
        }
    }
    pages
}

fn draw_text(_ui: &Ui, text: &str, x: f32, y: f32, size: f32, color: [f32; 4], canvas: Canvas) {
    let [x, y] = canvas.point(x, y);
    unsafe {
        imgui::sys::ImDrawList_AddText_FontPtr(
            imgui::sys::igGetForegroundDrawList_Nil(),
            imgui::sys::igGetFont(),
            size * canvas.scale,
            imgui::sys::ImVec2 { x, y },
            imgui::ImColor32::from_rgba_f32s(color[0], color[1], color[2], color[3]).to_bits(),
            text.as_ptr().cast(),
            text.as_ptr().add(text.len()).cast(),
            0.,
            std::ptr::null(),
        );
    }
}

pub(crate) struct NativeDialogPresenter {
    art: Option<Rc<DialogArtwork>>,
    kind: DialogKind,
    choices: Vec<String>,
    position: [f32; 2],
    page: usize,
    selected: usize,
    armed: bool,
    held_choice: Option<usize>,
    mouse_down: bool,
    pages: Option<Vec<Vec<Glyph>>>,
}

impl NativeDialogPresenter {
    pub(crate) fn new(
        loader: &AssetLoader,
        kind: DialogKind,
        choices: Vec<String>,
        position: [f32; 2],
        default_selection: usize,
    ) -> Self {
        let selected = default_selection.min(choices.len().saturating_sub(1));
        Self {
            art: loader.dialog_artwork(),
            kind,
            choices,
            position,
            page: 0,
            selected,
            armed: false,
            held_choice: None,
            mouse_down: false,
            pages: None,
        }
    }

    pub(crate) fn choice_count(&self) -> usize {
        self.choices.len()
    }

    pub(crate) fn prime_input(&mut self, ui: &Ui, input: &dyn InputEngine) {
        self.mouse_down = ui.is_mouse_down(MouseButton::Left);
        self.armed = !self.mouse_down
            && ![Key::Space, Key::GamePadEast, Key::GamePadSouth]
                .iter()
                .any(|&key| input.get_key_state(key).is_down());
    }

    pub(crate) fn update(
        &mut self,
        dialog: &DialogBox,
        ui: &Ui,
        input: &dyn InputEngine,
    ) -> Option<i32> {
        let confirms = [Key::Space, Key::GamePadEast, Key::GamePadSouth];
        let down = ui.is_mouse_down(MouseButton::Left);
        let confirm_down = confirms
            .iter()
            .any(|&key| input.get_key_state(key).is_down());
        let confirm = self.armed
            && confirms
                .iter()
                .any(|&key| input.get_key_state(key).pressed());
        let press = self.armed && down && !self.mouse_down;
        let release = self.armed && !down && self.mouse_down;
        let armed = self.armed;
        if !confirm_down && !down {
            self.armed = true;
        }
        self.mouse_down = down;
        let Some(art) = &self.art else {
            DialogBoxPresenter::new().update(dialog, 0.);
            if armed && (!down && release || confirm) {
                return Some(self.selected as i32 + 1);
            }
            return None;
        };
        let canvas = Canvas::new(ui.io().display_size);
        let mouse = [
            (ui.io().mouse_pos[0] - canvas.x) / canvas.scale,
            (ui.io().mouse_pos[1] - canvas.y) / canvas.scale,
        ];
        let _font = radiance::imgui::game_font(radiance::imgui::GameFontSize::LARGE)
            .map(|font| ui.push_font(font));
        let talk = self.kind == DialogKind::Talk;
        let selection = self.kind == DialogKind::Selection;
        let prefix = if talk {
            "ScenarioDialog"
        } else if selection {
            "SelectDialog"
        } else {
            "CommonDialog"
        };
        let text_widget = if talk {
            "EditDialog"
        } else if selection {
            "EditQuestion"
        } else {
            "Message"
        };
        let mut text_rect = art.rect(&format!("{prefix}/{text_widget}"));
        let size = if talk { 20. } else { 13. };
        let spacing = if talk {
            3.
        } else if selection {
            8.
        } else {
            0.
        };
        let measure = |ch: char| {
            let mut buf = [0; 4];
            ui.calc_text_size(ch.encode_utf8(&mut buf))[0] * size
                / unsafe { imgui::sys::igGetFontSize() }
        };
        let portrait = dialog.avatar().filter(|_| talk).map(|avatar| {
            let mut rect = art.rect("ScenarioDialog/ImgPortrait");
            rect[2] = rect[3] * avatar.width as f32 / avatar.height.max(1) as f32;
            let right = dialog.avatar_position() == AvatarPosition::Right;
            if right {
                rect[0] = 800. - rect[2];
                text_rect[2] = text_rect[2].min(rect[0] - 10. - text_rect[0]);
            } else {
                let end = text_rect[0] + text_rect[2];
                text_rect[0] = text_rect[0].max(rect[0] + rect[2] + 10.);
                text_rect[2] = end - text_rect[0];
            }
            (avatar, rect, right)
        });
        if text_rect[2] < size {
            text_rect[2] = size;
        }
        let pages = self.pages.get_or_insert_with(|| {
            paginate(
                dialog.segments(),
                text_rect[2],
                text_rect[3],
                size,
                spacing,
                measure,
            )
        });
        self.page = self.page.min(pages.len() - 1);
        let last = self.page + 1 == pages.len();
        let rows = self.choices.len().clamp(1, 18);
        let (width, height) = if selection {
            (278., 65. + rows as f32 * 24. + 24.)
        } else {
            (273., 107.)
        };
        let offset = if talk {
            [0., 0.]
        } else {
            [
                (800. * self.position[0] - width / 2.).clamp(0., 800. - width),
                (600. * self.position[1] - height / 2.).clamp(0., 600. - height),
            ]
        };
        let translated = |mut rect: Rect| {
            rect[0] += offset[0];
            rect[1] += offset[1];
            rect
        };
        if talk {
            art.draw(
                ui,
                "ScenarioDialog/ScenarioDialog",
                art.rect("ScenarioDialog/ScenarioDialog"),
                0,
                canvas,
            );
            if let Some((avatar, rect, right)) = portrait {
                blit(ui, avatar, rect, canvas, right);
            }
            art.draw(
                ui,
                "ScenarioDialog/ImgClick",
                art.rect("ScenarioDialog/ImgClick"),
                0,
                canvas,
            );
        } else if selection {
            art.draw(
                ui,
                "SelectDialog/Panelhead",
                translated(art.rect("SelectDialog/Panelhead")),
                0,
                canvas,
            );
            for row in 0..rows {
                let mut rect = art.rect("SelectDialog/Panelbody");
                rect[1] += row as f32 * 24.;
                art.draw(ui, "SelectDialog/Panelbody", translated(rect), 0, canvas);
            }
            let mut foot = art.rect("SelectDialog/Panelfoot");
            foot[1] += (rows - 1) as f32 * 24.;
            art.draw(ui, "SelectDialog/Panelfoot", translated(foot), 0, canvas);
        } else {
            for name in ["FrameMessage", "FrameButton"] {
                let name = format!("CommonDialog/{name}");
                art.draw(ui, &name, translated(art.rect(&name)), 0, canvas);
            }
        }
        text_rect = translated(text_rect);
        for glyph in &pages[self.page] {
            let mut buf = [0; 4];
            draw_text(
                ui,
                glyph.ch.encode_utf8(&mut buf),
                text_rect[0] + glyph.x,
                text_rect[1] + glyph.y,
                size,
                glyph.color,
                canvas,
            );
        }
        if !last || talk {
            if confirm || release {
                if last {
                    return Some(1);
                }
                self.page += 1;
            }
            if !last && !talk {
                art.draw(
                    ui,
                    "ScenarioDialog/ImgClick",
                    [
                        text_rect[0] + text_rect[2] - 13.,
                        text_rect[1] + text_rect[3] - 12.,
                        13.,
                        12.,
                    ],
                    0,
                    canvas,
                );
            }
            return None;
        }
        let count = self.choices.len().max(1);
        if armed {
            if [Key::Up, Key::Left, Key::GamePadDPadUp, Key::GamePadDPadLeft]
                .iter()
                .any(|&key| input.get_key_state(key).pressed())
                || ui.io().mouse_wheel > 0.
            {
                self.selected = (self.selected + count - 1) % count;
            }
            if [
                Key::Down,
                Key::Right,
                Key::GamePadDPadDown,
                Key::GamePadDPadRight,
            ]
            .iter()
            .any(|&key| input.get_key_state(key).pressed())
                || ui.io().mouse_wheel < 0.
            {
                self.selected = (self.selected + 1) % count;
            }
        }
        let first = self.selected.saturating_sub(rows - 1);
        let mut answer = None;
        for index in 0..count {
            if selection && !(first..first + rows).contains(&index) {
                continue;
            }
            let name = if selection {
                "SelectDialog/StaticAnswer"
            } else if self.kind == DialogKind::Confirmation {
                if index == 0 {
                    "CommonDialog/BtYes"
                } else {
                    "CommonDialog/BtNo"
                }
            } else {
                "CommonDialog/BtOk"
            };
            let mut rect = art.rect(name);
            if selection {
                rect[1] += (index - first) as f32 * 24.;
            }
            rect = translated(rect);
            let hovered = mouse[0] >= rect[0]
                && mouse[0] < rect[0] + rect[2]
                && mouse[1] >= rect[1]
                && mouse[1] < rect[1] + rect[3];
            if press && hovered {
                self.held_choice = Some(index);
                self.selected = index;
            }
            if release && hovered && self.held_choice == Some(index) {
                answer = Some(index as i32 + 1);
            }
            let state = if down && self.held_choice == Some(index) {
                2
            } else if hovered || self.selected == index {
                1
            } else {
                0
            };
            art.draw(ui, name, rect, state, canvas);
            if selection {
                let text = dialog_markup::strip(&self.choices[index]);
                ui.get_foreground_draw_list().with_clip_rect(
                    canvas.point(rect[0], rect[1]),
                    canvas.point(rect[0] + rect[2], rect[1] + rect[3]),
                    || {
                        draw_text(
                            ui,
                            &text,
                            rect[0] + 4.,
                            rect[1] + 5.,
                            13.,
                            [0., 0., 0., 1.],
                            canvas,
                        );
                    },
                );
            }
        }
        if release {
            self.held_choice = None;
        }
        if confirm {
            answer = Some(self.selected as i32 + 1);
        }
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_keeps_all_colored_glyphs_inside_the_text_area() {
        let segments =
            dialog_markup::parse("AB\nC<colour red=255 green=0 blue=0>DEF</colour>G", [1.; 4]);
        let pages = paginate(&segments, 20., 23., 10., 3., |_| 10.);
        assert_eq!(pages.len(), 2);
        assert_eq!(
            pages.iter().flatten().map(|g| g.ch).collect::<String>(),
            "ABCDEFG"
        );
        assert!(
            pages
                .iter()
                .flatten()
                .all(|g| g.x + 10. <= 20. && g.y + 10. <= 23.)
        );
        assert_eq!(pages[1][1].color, [1., 0., 0., 1.]);
    }

    #[test]
    fn trailing_newlines_do_not_create_empty_pages() {
        let segments = dialog_markup::parse("A\n\n", [1.; 4]);
        assert_eq!(paginate(&segments, 10., 10., 10., 0., |_| 10.).len(), 1);
        let segments = dialog_markup::parse("\n\nA", [1.; 4]);
        let pages = paginate(&segments, 10., 10., 10., 0., |_| 10.);
        assert_eq!(pages[0][0].y, 0.);
    }

    #[test]
    fn native_canvas_letterboxes_widescreen_and_scales_evenly() {
        let canvas = Canvas::new([1920., 1080.]);
        assert_eq!(canvas.point(0., 0.), [240., 0.]);
        assert_eq!(canvas.point(800., 600.), [1680., 1080.]);
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_dialog_layouts_resolve_native_frames_buttons_and_atlases() {
        let root = std::env::var("PAL4_ASSET_ROOT").unwrap();
        let vfs = packfs::init_virtual_fs(&root, None);
        let mut index = crate::loaders::cegui::ui_layout_handle::ImagesetIndex::new(
            "/gamedata/ui/imagesets".into(),
        );
        for file in ["ScenarioDialog", "CommonDialog", "SelectDialog"] {
            let parsed =
                layout::load_layout(&vfs, &format!("/gamedata/ui/layouts/{file}.xml")).unwrap();
            for window in parsed.windows {
                for reference in [
                    window.image,
                    window.overlay_normal_image,
                    window.overlay_highlight_image,
                    window.overlay_pushed_image,
                ]
                .into_iter()
                .flatten()
                {
                    let (atlas, png) = index.load(&vfs, &reference.set).unwrap();
                    assert!(
                        atlas.get(&reference.image).is_some(),
                        "{file}/{}",
                        window.name
                    );
                    assert!(image::load_from_memory(&png).is_ok());
                }
            }
        }
    }
}
