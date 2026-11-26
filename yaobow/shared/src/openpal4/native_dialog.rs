use std::{collections::HashMap, rc::Rc};

use anyhow::Context;
use common::store_ext::StoreExt2;
use imgui::{MouseButton, Ui};
use radiance::input::{InputEngine, Key};

use crate::{
    loaders::cegui::{
        layout::{self, ImageRef},
        sequence::SequenceAnimation,
        ui_layout_handle::uv_for,
    },
    ui::{
        dialog_box::{AvatarPosition, DialogBox, DialogBoxPresenter},
        dialog_markup::{self, Segment},
    },
};

use super::asset_loader::{AssetLoader, ImageSetImage};

type Rect = [f32; 4];

#[derive(Default)]
struct TextReveal {
    elapsed: f32,
    visible: usize,
}

impl TextReveal {
    fn update(&mut self, delta_sec: f32, glyphs: usize) {
        if delta_sec.is_finite() && delta_sec > 0. {
            self.elapsed += delta_sec;
            // OLSDialogEditbox advances once, then clears its timer (not a catch-up loop).
            if self.elapsed > 0.1 {
                self.visible = (self.visible + 1).min(glyphs);
                self.elapsed = 0.;
            }
        }
    }

    fn advance(&mut self, glyphs: usize) -> bool {
        if self.visible < glyphs {
            self.visible = glyphs;
            false
        } else {
            *self = Self::default();
            true
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DialogKind {
    Talk,
    Message,
    Selection,
    Confirmation,
}

impl DialogKind {
    fn text_widget(self) -> &'static str {
        match self {
            Self::Talk => "ScenarioDialog/EditDialog",
            Self::Selection => "SelectDialog/EditQuestion",
            _ => "CommonDialog/Message",
        }
    }
}

struct Widget {
    rect: Rect,
    images: [Option<ImageRef>; 3],
    frame: [Option<ImageRef>; 8],
    text_color: [f32; 4],
}

pub(crate) struct DialogArtwork {
    widgets: HashMap<String, Widget>,
    images: HashMap<String, ImageSetImage>,
    click_sequence: Option<SequenceAnimation>,
}

impl DialogArtwork {
    pub(crate) fn load(loader: &AssetLoader) -> anyhow::Result<Self> {
        let mut widgets = HashMap::new();
        let mut click_sequence = None;
        for file in ["ScenarioDialog", "CommonDialog", "SelectDialog"] {
            let parsed =
                layout::load_layout(loader.vfs(), &format!("/gamedata/ui/layouts/{file}.xml"))
                    .with_context(|| format!("loading {file}.xml"))?;
            let mut rects: Vec<Rect> = Vec::new();
            for window in &parsed.windows {
                if file == "ScenarioDialog" && window.name == "ImgClick" {
                    if let Some(name) = &window.seg_image {
                        let path = format!("/gamedata/ui/sequence_images/{name}.seqimage");
                        click_sequence = Some(
                            SequenceAnimation::parse(&loader.vfs().read_to_end(&path)?)
                                .with_context(|| format!("loading {path}"))?,
                        );
                    }
                }
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
                let text_color = window
                    .properties
                    .iter()
                    .find(|p| p.name == "NormalTextColour")
                    .and_then(|p| u32::from_str_radix(&p.value, 16).ok())
                    .map(|argb| {
                        [
                            ((argb >> 16) & 255) as f32 / 255.,
                            ((argb >> 8) & 255) as f32 / 255.,
                            (argb & 255) as f32 / 255.,
                            (argb >> 24) as f32 / 255.,
                        ]
                    })
                    .unwrap_or([1.; 4]);
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
                        text_color,
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
        Ok(Self {
            widgets,
            images,
            click_sequence,
        })
    }

    fn rect(&self, name: &str) -> Rect {
        self.widgets.get(name).map_or([0.; 4], |w| w.rect)
    }

    fn image(&self, reference: &ImageRef) -> Option<&ImageSetImage> {
        self.images
            .get(&format!("{}:{}", reference.set, reference.image).to_lowercase())
    }

    fn draw_click(&self, ui: &Ui, rect: Rect, canvas: Canvas, elapsed: f64) {
        if let Some(image) = self
            .click_sequence
            .as_ref()
            .and_then(|sequence| self.image(sequence.frame(elapsed)))
        {
            blit(ui, image, rect, canvas, false);
        } else {
            self.draw(ui, "ScenarioDialog/ImgClick", rect, 0, canvas);
        }
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

fn image_uvs(rect: [u32; 4], [width, height]: [u32; 2], mirror: bool) -> [[f32; 2]; 2] {
    let (u0, v0, u1, v1) = uv_for(rect, width, height);
    let (u0, u1) = if mirror { (u1, u0) } else { (u0, u1) };
    [[u0, v0], [u1, v1]]
}

fn blit(ui: &Ui, image: &ImageSetImage, [x, y, w, h]: Rect, canvas: Canvas, mirror: bool) {
    let [uv0, uv1] = image_uvs(
        [image.x, image.y, image.width, image.height],
        [image.sprite.width(), image.sprite.height()],
        mirror,
    );
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
    reveal: TextReveal,
    animation_elapsed: f64,
    pointer_position: Option<[f32; 2]>,
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
            reveal: TextReveal::default(),
            animation_elapsed: 0.,
            pointer_position: None,
        }
    }

    pub(crate) fn choice_count(&self) -> usize {
        self.choices.len()
    }

    pub(crate) fn text_color(&self) -> [f32; 4] {
        self.art
            .as_ref()
            .and_then(|art| art.widgets.get(self.kind.text_widget()))
            .map_or([1.; 4], |widget| widget.text_color)
    }

    pub(crate) fn prime_input(&mut self, ui: &Ui, input: &dyn InputEngine) {
        self.pointer_position = Some(ui.io().mouse_pos);
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
        delta_sec: f32,
    ) -> Option<i32> {
        if delta_sec.is_finite() && delta_sec > 0. {
            self.animation_elapsed += delta_sec as f64;
        }
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
        let pointer_moved = self.pointer_position != Some(ui.io().mouse_pos);
        self.pointer_position = Some(ui.io().mouse_pos);
        let mouse = [
            (ui.io().mouse_pos[0] - canvas.x) / canvas.scale,
            (ui.io().mouse_pos[1] - canvas.y) / canvas.scale,
        ];
        let _font = radiance::imgui::game_font(radiance::imgui::GameFontSize::LARGE)
            .map(|font| ui.push_font(font));
        let talk = self.kind == DialogKind::Talk;
        let selection = self.kind == DialogKind::Selection;
        let mut text_rect = art.rect(self.kind.text_widget());
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
        if talk {
            self.reveal.update(delta_sec, pages[self.page].len());
        }
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
            art.draw_click(
                ui,
                art.rect("ScenarioDialog/ImgClick"),
                canvas,
                self.animation_elapsed,
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
        let visible = if talk {
            self.reveal.visible
        } else {
            pages[self.page].len()
        };
        for glyph in pages[self.page].iter().take(visible) {
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
                if !talk || self.reveal.advance(pages[self.page].len()) {
                    if last {
                        return Some(1);
                    }
                    self.page += 1;
                }
            }
            if !last && !talk {
                art.draw_click(
                    ui,
                    [
                        text_rect[0] + text_rect[2] - 13.,
                        text_rect[1] + text_rect[3] - 12.,
                        13.,
                        12.,
                    ],
                    canvas,
                    self.animation_elapsed,
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
        let buttons: Vec<_> = (0..count)
            .filter(|&index| !selection || (first..first + rows).contains(&index))
            .map(|index| {
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
                (index, name, translated(rect))
            })
            .collect();
        let hovered_choice = buttons
            .iter()
            .find(|(_, _, rect)| {
                mouse[0] >= rect[0]
                    && mouse[0] < rect[0] + rect[2]
                    && mouse[1] >= rect[1]
                    && mouse[1] < rect[1] + rect[3]
            })
            .map(|(index, _, _)| *index);
        if armed && (pointer_moved || press) {
            if let Some(index) = hovered_choice {
                self.selected = index;
            }
        }
        let mut answer = None;
        for (index, name, rect) in buttons {
            let hovered = hovered_choice == Some(index);
            if press && hovered {
                self.held_choice = Some(index);
                self.selected = index;
            }
            if release && hovered && self.held_choice == Some(index) {
                answer = Some(index as i32 + 1);
            }
            let state = if down && self.held_choice == Some(index) {
                2
            } else if self.selected == index {
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
    fn stretched_dialog_images_never_filter_neighboring_atlas_cells() {
        for region in [[2, 2, 4, 2], [2, 2, 1, 2], [2, 2, 2, 1], [2, 2, 1, 1]] {
            let outside = |x: i32, y: i32| {
                if x < region[0] as i32
                    || x >= (region[0] + region[2]) as i32
                    || y < region[1] as i32
                    || y >= (region[1] + region[3]) as i32
                {
                    1.
                } else {
                    0.
                }
            };
            for mirror in [false, true] {
                let [uv0, uv1] = image_uvs(region, [8, 8], mirror);
                for scale in [0.75f32, 1., 1.515, 2.] {
                    let width = (29. * scale).round() as usize;
                    let height = (126. * scale).round() as usize;
                    for y in 0..height {
                        for x in 0..width {
                            let u = uv0[0] + (uv1[0] - uv0[0]) * (x as f32 + 0.5) / width as f32;
                            let v = uv0[1] + (uv1[1] - uv0[1]) * (y as f32 + 0.5) / height as f32;
                            let (tx, ty) = (u * 8. - 0.5, v * 8. - 0.5);
                            let (ix, iy) = (tx.floor() as i32, ty.floor() as i32);
                            let (fx, fy) = (tx - ix as f32, ty - iy as f32);
                            let contamination = outside(ix, iy) * (1. - fx) * (1. - fy)
                                + outside(ix + 1, iy) * fx * (1. - fy)
                                + outside(ix, iy + 1) * (1. - fx) * fy
                                + outside(ix + 1, iy + 1) * fx * fy;
                            assert!(
                                contamination < 1e-6,
                                "atlas bleed: {region:?}, scale={scale}, mirror={mirror}, sample=({x},{y})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn text_reveal_uses_native_cadence_and_consumes_the_first_advance() {
        let mut reveal = TextReveal::default();
        reveal.update(0.1, 4);
        assert_eq!(reveal.visible, 0);
        reveal.update(0.01, 4);
        assert_eq!(reveal.visible, 1);
        reveal.update(3.0, 4);
        assert_eq!(
            reveal.visible, 2,
            "native update does not batch glyphs after a stalled frame"
        );
        assert!(
            !reveal.advance(4),
            "first input reveals, rather than dismisses, the page"
        );
        assert_eq!(reveal.visible, 4);
        assert!(reveal.advance(4));
        assert_eq!(reveal.visible, 0, "the next page starts unrevealed");
        for delta in [0., -1., f32::NAN, f32::INFINITY] {
            reveal.update(delta, 4);
        }
        assert_eq!(reveal.visible, 0);
    }

    #[test]
    fn native_sequences_respect_declared_order_durations_and_loop_count() {
        let source = br#"<SequenceImage FrameCount="2" LoopCount="0">
            <ImageFrame Index="1" Duration="0.3" Imageset="ui" Image="second"/>
            <ImageFrame Index="0" Duration="0.2" Imageset="ui" Image="first"/>
            </SequenceImage>"#;
        let mut sequence = SequenceAnimation::parse(source).unwrap();
        assert_eq!(sequence.frame(0.1).image, "first");
        assert_eq!(sequence.frame(0.25).image, "second");
        assert_eq!(sequence.frame(0.6).image, "first");
        sequence.loops = 1;
        assert_eq!(sequence.frame(0.6).image, "second");
        for invalid in [
            r#"<SequenceImage FrameCount="1"><ImageFrame Index="0" Duration="0" Imageset="s" Image="i"/></SequenceImage>"#,
            r#"<SequenceImage FrameCount="1"><ImageFrame Index="2" Duration=".2" Imageset="s" Image="i"/></SequenceImage>"#,
        ] {
            assert!(SequenceAnimation::parse(invalid.as_bytes()).is_err());
        }
    }

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
        let sequence = SequenceAnimation::parse(
            &vfs.read_to_end("/gamedata/ui/sequence_images/sanjiao.seqimage")
                .unwrap(),
        )
        .unwrap();
        for (reference, _) in sequence.frames {
            let (atlas, _) = index.load(&vfs, &reference.set).unwrap();
            assert!(atlas.get(&reference.image).is_some());
        }
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
