use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use image::{AnimationDecoder, Frame, RgbaImage};
use imgui::{ColorStackToken, Condition, StyleColor, StyleVar, TextureId, Ui};
use mini_fs::{LocalFs, MiniFs, StoreExt, ZipFs};
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    rendering::{ComponentFactory, Texture},
};

use crate::ComObject_TitleSelectionDirector;

use super::GameType;

pub struct TitleSelectionDirector {
    factory: Rc<dyn ComponentFactory>,
    audio: Rc<dyn AudioEngine>,
    input: Rc<RefCell<dyn InputEngine>>,
    selected_game: Rc<RefCell<Option<GameType>>>,
    vfs: Option<Rc<MiniFs>>,
    dpi_scale: f32,
    props: RefCell<Option<Props>>,
}

ComObject_TitleSelectionDirector!(super::TitleSelectionDirector);

pub struct Props {
    bgm_source: Box<dyn radiance::audio::AudioMemorySource>,
    ui: TitleUi,
    time: f32,
}

impl IDirectorImpl for TitleSelectionDirector {
    fn activate(&self, scene_manager: ComRc<ISceneManager>) {
        if let Some(vfs) = &self.vfs {
            let data = vfs.read_to_end("/music/Grace.ogg").unwrap();
            let mut bgm_source = self.audio.create_source();
            bgm_source.set_data(data, radiance::audio::Codec::Ogg);
            bgm_source.play(true);

            let logo = image::load_from_memory_with_format(
                &vfs.read_to_end("/images/yaobow.png").unwrap(),
                image::ImageFormat::Png,
            )
            .unwrap()
            .to_rgba8();
            let outline = vfs.open("/images/title/outlines/1.gif").unwrap();
            let img = image::gif::GifDecoder::new(outline).unwrap();
            let r = img.into_frames().map(|f| f.unwrap()).collect::<Vec<_>>();

            self.props.replace(Some(Props {
                bgm_source,
                ui: TitleUi::new(
                    vfs.clone(),
                    self.factory.clone(),
                    &logo,
                    vec![r],
                    self.selected_game.clone(),
                    self.dpi_scale,
                ),
                time: 0.,
            }));
        }
    }

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        if let Some(props) = self.props.borrow_mut().as_mut() {
            props.bgm_source.update();

            let window_size = ui.io().display_size;
            let _t2 = ui.push_style_var(StyleVar::WindowBorderSize(0.));
            let _t0 = ui.push_style_color(StyleColor::WindowBg, [1.0, 1.0, 1.0, 1.0]);
            ui.window("underlayer")
                .no_decoration()
                .size(window_size, Condition::Always)
                .position([0.0, 0.0], Condition::Always)
                .collapsible(false)
                .always_auto_resize(true)
                .build(|| {});

            let _t1 = ui.push_style_color(StyleColor::WindowBg, BG_COLOR);
            let _t3 = if props.time < 2. {
                props.time += delta_sec;
                Some(ui.push_style_var(StyleVar::Alpha(0.)))
            } else if props.time < 5. {
                let alpha = props.time;
                props.time += delta_sec;
                Some(ui.push_style_var(StyleVar::Alpha((alpha - 2.) / 3.)))
            } else {
                None
            };

            ui.window("main")
                .no_decoration()
                .size(window_size, Condition::Always)
                .position([0.0, 0.0], Condition::Always)
                .collapsible(false)
                .always_auto_resize(true)
                .build(|| {
                    props.ui.update(ui, delta_sec);
                });
        }

        None
    }
}

impl TitleSelectionDirector {
    pub fn new(
        factory: Rc<dyn ComponentFactory>,
        audio: Rc<dyn AudioEngine>,
        input: Rc<RefCell<dyn InputEngine>>,
        selected_game: Rc<RefCell<Option<GameType>>>,
        dpi_scale: f32,
    ) -> Self {
        let vfs = Self::load_vfs();
        Self {
            factory,
            audio,
            input,
            selected_game,
            vfs,
            dpi_scale,
            props: RefCell::new(None),
        }
    }

    fn load_vfs() -> Option<Rc<MiniFs>> {
        let mut vfs = MiniFs::new(false);
        let zip = PathBuf::from(ASSET_PATH);
        let local1 = PathBuf::from("./yaobow/yaobow-assets");
        let local2 = PathBuf::from("../yaobow-assets");

        if Path::exists(&zip) {
            let local = ZipFs::new(std::fs::File::open(zip).unwrap());
            vfs = vfs.mount(PathBuf::from("/"), local);
            Some(Rc::new(vfs))
        } else if Path::exists(&local1) {
            let local = LocalFs::new(&local1);
            vfs = vfs.mount(PathBuf::from("/"), local);
            Some(Rc::new(vfs))
        } else if Path::exists(&local2) {
            let local = LocalFs::new(&local2);
            vfs = vfs.mount(PathBuf::from("/"), local);
            Some(Rc::new(vfs))
        } else {
            None
        }
    }
}

struct TitleUi {
    game_pic: GamePic,
    title_list: TitleList,
    props: RefCell<UiProps>,
}

struct UiProps {
    logo: Box<dyn Texture>,
    logo_id: TextureId,
}

impl TitleUi {
    fn new(
        vfs: Rc<MiniFs>,
        factory: Rc<dyn ComponentFactory>,
        logo: &RgbaImage,
        outlines: Vec<Vec<Frame>>,
        selected_game: Rc<RefCell<Option<GameType>>>,
        dpi_scale: f32,
    ) -> Self {
        let (logo, logo_id) =
            factory.create_imgui_texture(&logo, 0, logo.width(), logo.height(), None);
        let hovered_game = Rc::new(RefCell::new(None));
        Self {
            game_pic: GamePic::new(vfs, factory.clone(), outlines, hovered_game.clone()),
            title_list: TitleList::new(factory, selected_game, hovered_game.clone(), dpi_scale),
            props: RefCell::new(UiProps { logo, logo_id }),
        }
    }

    fn update(&mut self, ui: &Ui, delta_sec: f32) {
        const LEFT_PANE_WIDTH_RATIO: f32 = 0.4;

        let [window_width, window_height] = ui.io().display_size;
        let unit = (window_width / 16.).min(window_height / 9.);
        let [central_area_width, central_area_height] = [unit * 16., unit * 9.];
        let central_area_start = [
            (window_width - central_area_width) * 0.5,
            (window_height - central_area_height) * 0.5,
        ];

        let logo_center = [
            central_area_width * LEFT_PANE_WIDTH_RATIO / 2. + central_area_start[0],
            central_area_height * 0.2 + central_area_start[1],
        ];
        let logo_size = [central_area_width * 0.4, central_area_height * 0.3];
        self.draw_logo(ui, logo_center, logo_size);

        let title_list_center = [
            central_area_width * LEFT_PANE_WIDTH_RATIO / 2. + central_area_start[0],
            central_area_height * 0.65 + central_area_start[1],
        ];
        let title_list_size = [central_area_width * 0.4, central_area_height * 0.7];
        self.title_list
            .update(ui, title_list_center, title_list_size, delta_sec);

        let game_pic_center = [
            central_area_width * (LEFT_PANE_WIDTH_RATIO + (1. - LEFT_PANE_WIDTH_RATIO) / 2.)
                + central_area_start[0],
            central_area_height * 0.5 + central_area_start[1],
        ];
        let game_pic_size = [
            central_area_width * (1. - LEFT_PANE_WIDTH_RATIO),
            central_area_height,
        ];
        self.game_pic
            .update(ui, game_pic_center, game_pic_size, delta_sec);

        self.draw_version(ui);
    }

    fn draw_logo(&mut self, ui: &Ui, center: [f32; 2], size: [f32; 2]) {
        let w = self.props.borrow().logo.width();
        let h = self.props.borrow().logo.height();
        let target_size = calc_target_size(size, [w as f32, h as f32]);
        ui.set_cursor_pos([
            center[0] - target_size[0] * 0.5,
            center[1] - target_size[1] * 0.5,
        ]);
        imgui::Image::new(self.props.borrow().logo_id, target_size).build(ui);
    }

    fn draw_version(&mut self, ui: &Ui) {
        let version = format!(
            "妖弓主程序版本：{}-{} build-dev",
            env!("CARGO_PKG_VERSION"),
            GIT_SHORT_TAG
        );
        let text_size = ui.calc_text_size(&version);
        let text_start = [5., ui.io().display_size[1] - text_size[1] - 2.];

        let fonts = ui.fonts().fonts();
        let _t = ui.push_style_color(StyleColor::Text, [0.5, 0.5, 0.5, 1.0]);
        let _f = ui.push_font(fonts[radiance::imgui::FontIndex::SMALL_FONT]);
        ui.set_cursor_pos(text_start);
        ui.text(version);
    }
}

struct GamePic {
    vfs: Rc<MiniFs>,
    outlines: TitleOutlines,
    hovered_game: Rc<RefCell<Option<usize>>>,
    factory: Rc<dyn ComponentFactory>,
    texture: Option<Box<dyn Texture>>,
    texture_id: Option<TextureId>,
    image: Option<RgbaImage>,
    current_game: Option<usize>,
    state: GamePicState,
}

#[derive(Debug, Copy, Clone)]
enum GamePicState {
    Outline,
    FadeIn,
    ShowingPic { index: usize },
}

impl GamePic {
    fn new(
        vfs: Rc<MiniFs>,
        factory: Rc<dyn ComponentFactory>,
        outlines: Vec<Vec<Frame>>,
        hovered_game: Rc<RefCell<Option<usize>>>,
    ) -> Self {
        Self {
            vfs,
            outlines: TitleOutlines::new(factory.clone(), outlines),
            hovered_game,
            factory,
            texture: None,
            texture_id: None,
            state: GamePicState::Outline,
            image: None,
            current_game: None,
        }
    }

    fn update(&mut self, ui: &Ui, center: [f32; 2], size: [f32; 2], delta_sec: f32) {
        if let Some(index) = *self.hovered_game.borrow() {
            self.state = GamePicState::ShowingPic { index };
        }

        match self.state {
            GamePicState::Outline => self.outlines.update(ui, center, size, delta_sec),
            GamePicState::ShowingPic { index } => {
                if *self.hovered_game.borrow() != self.current_game {
                    self.current_game = *self.hovered_game.borrow();
                    let (folder_name, img_count) = GAME_PICS[index];
                    let img_id = rand::random::<usize>() % img_count;
                    let img_path = format!("/images/title/{}/{}.jpg", folder_name, img_id + 1);
                    let img = image::load_from_memory_with_format(
                        &self.vfs.read_to_end(&img_path).unwrap(),
                        image::ImageFormat::Jpeg,
                    )
                    .unwrap()
                    .to_rgba8();
                    self.image = Some(img);
                }

                if let Some(img) = &self.image {
                    let (texture, texture_id) = self.factory.create_imgui_texture(
                        &img,
                        0,
                        img.width(),
                        img.height(),
                        self.texture_id,
                    );

                    self.texture.replace(texture);
                    self.texture_id = Some(texture_id);

                    let target_size =
                        calc_target_size(size, [img.width() as f32, img.height() as f32]);

                    ui.set_cursor_pos([
                        center[0] - target_size[0] * 0.5,
                        center[1] - target_size[1] * 0.5,
                    ]);
                    imgui::Image::new(texture_id, target_size).build(ui);
                }
            }
            _ => {}
        }
    }
}

struct TitleList {
    factory: Rc<dyn ComponentFactory>,
    // input: Rc<RefCell<dyn InputEngine>>,
    selected_game: Rc<RefCell<Option<GameType>>>,
    hovered_game: Rc<RefCell<Option<usize>>>,
    state: TitleListState,
    dpi_scale: f32,
}

#[derive(Debug, Clone, Copy)]
enum TitleListState {
    FirstTimeWaitingForAnyKey { time: f32 },
    WaitingForAnyKey { time: f32 },
    InAnimation { time: f32 },
    ShowingList { time: f32 },
}

impl TitleList {
    pub fn new(
        factory: Rc<dyn ComponentFactory>,
        // input: Rc<RefCell<dyn InputEngine>>,
        selected_game: Rc<RefCell<Option<GameType>>>,
        hovered_game: Rc<RefCell<Option<usize>>>,
        dpi_scale: f32,
    ) -> Self {
        Self {
            factory,
            // input,
            selected_game,
            hovered_game,
            state: TitleListState::FirstTimeWaitingForAnyKey {
                time: UNDERLINE_FIRST_TIME_WAITING,
            },
            dpi_scale,
        }
    }

    pub fn update(&mut self, ui: &Ui, center: [f32; 2], size: [f32; 2], delta_sec: f32) {
        const TEXT: &str = "按下任意键继续";
        const TEXT2: &str = "按下任意键继续_";

        let text_size = ui.calc_text_size(TEXT2);
        let text_start = [
            center[0] - text_size[0] * 0.5,
            center[1] - text_size[1] * 0.5,
        ];
        let _t = ui.push_style_color(StyleColor::Text, [0., 0., 0., 1.]);

        match &mut self.state {
            TitleListState::FirstTimeWaitingForAnyKey { time } => {
                ui.set_cursor_pos(text_start);
                ui.text(TEXT);

                *time -= delta_sec;
                if *time < 0. {
                    self.state = TitleListState::WaitingForAnyKey { time: -*time };
                }

                self.check_any_key(ui);
            }
            TitleListState::WaitingForAnyKey { time } => {
                while *time > UNDERLINE_MARGIN_TIME + UNDERLINE_SHOW_TIME {
                    *time -= UNDERLINE_MARGIN_TIME + UNDERLINE_SHOW_TIME;
                }

                ui.set_cursor_pos(text_start);
                if *time > UNDERLINE_MARGIN_TIME {
                    ui.text(TEXT2);
                } else {
                    ui.text(TEXT);
                }

                *time += delta_sec;
                self.check_any_key(ui);
            }
            TitleListState::InAnimation { time } => {
                self.hovered_game.borrow_mut().replace(0);
                self.state = TitleListState::ShowingList { time: 0. };
            }
            TitleListState::ShowingList { time } => self.draw_game_list(ui, center, size),
        }
    }

    fn check_any_key(&mut self, ui: &Ui) {
        if ui.io().keys_down.iter().any(|&k| k) || ui.io().mouse_down.iter().any(|&k| k) {
            self.state = TitleListState::InAnimation { time: 0. };
        }
    }

    fn draw_game_list(&self, ui: &imgui::Ui, center: [f32; 2], size: [f32; 2]) {
        let item_width = size[0];
        let item_start_x = center[0] - item_width * 0.5;
        let item_height = 60.0 * self.dpi_scale;
        let all_height = item_height * GAMES.len() as f32;
        let item_start_y = center[1] - all_height / 2.0;

        let mut cursor_y = item_start_y;
        for (i, game) in GAMES.iter().enumerate() {
            ui.set_cursor_pos([item_start_x, cursor_y]);
            let _tokens = self.push_button_colors(ui);
            let _text_color_token = if *self.hovered_game.borrow() != Some(i) {
                Some(ui.push_style_color(StyleColor::Text, [0.5, 0.5, 0.5, 1.]))
            } else {
                None
            };

            if ui.button_with_size(game.full_name(), [item_width, item_height]) {
                self.selected_game.replace(Some(*game));
            }

            if ui.is_item_hovered() {
                self.hovered_game.replace(Some(i));
            }

            cursor_y += item_height;
        }
    }

    fn push_button_colors<'ui>(&self, ui: &'ui imgui::Ui) -> Vec<ColorStackToken<'ui>> {
        let color = [0., 0., 0., 0.];
        vec![
            ui.push_style_color(imgui::StyleColor::Button, color),
            ui.push_style_color(imgui::StyleColor::ButtonHovered, color),
            ui.push_style_color(imgui::StyleColor::ButtonActive, color),
        ]
    }
}

struct TitleOutlines {
    factory: Rc<dyn ComponentFactory>,
    outlines: Vec<Vec<Frame>>,
    outline_state: OutlineState,
    texture: Option<Box<dyn Texture>>,
    texture_id: Option<TextureId>,
    time: f32,
}

#[derive(Debug, Clone, Copy)]
enum OutlineState {
    FirstTimeWaiting {
        time: f32,
    },
    PlayingFrame {
        outline_id: usize,
        frame_id: usize,
        frame_time: f32,
        remaining_time: f32,
    },
    Waiting {
        next_outline_id: usize,
        time: f32,
    },
}

impl TitleOutlines {
    fn new(factory: Rc<dyn ComponentFactory>, outlines: Vec<Vec<Frame>>) -> Self {
        Self {
            factory,
            outlines,
            outline_state: OutlineState::FirstTimeWaiting {
                time: UNDERLINE_FIRST_TIME_WAITING,
            },
            texture_id: None,
            texture: None,
            time: 0.,
        }
    }

    fn draw_frame(
        factory: &Rc<dyn ComponentFactory>,
        texture_id: Option<TextureId>,
        ui: &Ui,
        frame: &Frame,
        center: [f32; 2],
        size: [f32; 2],
        alpha: f32,
    ) -> (Box<dyn Texture>, Option<TextureId>) {
        let img = frame.buffer();
        let w = img.width();
        let h = img.height();
        let target_size = calc_target_size(size, [w as f32, h as f32]);
        let (texture, texture_id) = factory.create_imgui_texture(&img, w, w, h, texture_id);

        ui.set_cursor_pos([
            center[0] - target_size[0] * 0.5,
            center[1] - target_size[1] * 0.5,
        ]);
        imgui::Image::new(texture_id, target_size)
            .tint_col([1.0, 1.0, 1.0, alpha])
            .build(ui);

        (texture, Some(texture_id))
    }

    fn update(&mut self, ui: &Ui, center: [f32; 2], size: [f32; 2], delta_sec: f32) {
        let size = [size[0], size[1] * 0.7];
        match self.outline_state {
            OutlineState::PlayingFrame {
                outline_id,
                frame_id,
                frame_time,
                remaining_time,
            } => {
                let alpha = if remaining_time < OUTLINE_FADE_TIME {
                    remaining_time / OUTLINE_FADE_TIME
                } else if remaining_time > OUTLINE_SHOW_TIME - OUTLINE_FADE_TIME {
                    (OUTLINE_SHOW_TIME - remaining_time) / OUTLINE_FADE_TIME
                } else {
                    1.
                };

                let ret = Self::draw_frame(
                    &self.factory,
                    self.texture_id,
                    ui,
                    &self.outlines[outline_id][frame_id],
                    center,
                    size,
                    alpha,
                );
                self.texture_id = ret.1;
                self.texture = Some(ret.0);
            }
            _ => {}
        };

        match &mut self.outline_state {
            OutlineState::FirstTimeWaiting { time } => {
                *time -= delta_sec;
                if *time < 0. {
                    self.outline_state = OutlineState::Waiting {
                        next_outline_id: 0,
                        time: OUTLINE_MARGIN_TIME,
                    };
                }
            }
            OutlineState::PlayingFrame {
                outline_id,
                frame_id,
                frame_time,
                remaining_time,
            } => {
                let frames = &self.outlines[*outline_id];
                let frame = &frames[*frame_id];
                *frame_time += delta_sec;
                let delay: Duration = frame.delay().into();
                if *frame_time >= delay.as_secs_f32() {
                    *frame_time = *frame_time - delay.as_secs_f32();
                    *frame_id += 1;
                    if *frame_id >= frames.len() {
                        *frame_id = 0;
                    }
                }

                *remaining_time -= delta_sec;
                if *remaining_time < 0. {
                    let mut new_outline_id = *outline_id + 1;
                    if new_outline_id >= self.outlines.len() {
                        new_outline_id = 0;
                    }

                    self.outline_state = OutlineState::Waiting {
                        next_outline_id: new_outline_id,
                        time: OUTLINE_MARGIN_TIME,
                    };
                }
            }
            OutlineState::Waiting {
                next_outline_id,
                time,
            } => {
                self.time += delta_sec;
                if self.time > UNDERLINE_MARGIN_TIME + UNDERLINE_SHOW_TIME {
                    self.time = 0.;
                }

                if self.time > UNDERLINE_MARGIN_TIME {
                    let underline_size = [size[0] * 0.5, 4.];
                    ui.set_cursor_pos([
                        center[0] - underline_size[0] * 0.5,
                        center[1] + size[1] * 0.5 - underline_size[1],
                    ]);
                    let draw_list = ui.get_window_draw_list();
                    draw_list
                        .add_rect(
                            ui.cursor_pos(),
                            [
                                ui.cursor_pos()[0] + underline_size[0],
                                ui.cursor_pos()[1] + underline_size[1],
                            ],
                            [0.12, 0.12, 0.12, 1.],
                        )
                        .filled(true)
                        .build();
                }

                *time -= delta_sec;
                if *time < 0. {
                    self.outline_state = OutlineState::PlayingFrame {
                        outline_id: *next_outline_id,
                        frame_id: 0,
                        frame_time: -*time,
                        remaining_time: OUTLINE_SHOW_TIME,
                    };
                }
            }
        }
    }
}

fn calc_target_size(avail_size: [f32; 2], image_size: [f32; 2]) -> [f32; 2] {
    let (w, h) = (image_size[0], image_size[1]);
    let (avail_width, avail_height) = (avail_size[0], avail_size[1]);
    let (w_scale, h_scale) = (avail_width / w as f32, avail_height / h as f32);
    let scale = w_scale.min(h_scale);
    let target_size = [w as f32 * scale, h as f32 * scale];

    target_size
}

const GAMES: &[GameType] = &[GameType::PAL3, GameType::PAL4];
const GAME_PICS: &[(&str, usize); 2] = &[("pal3", 7), ("pal4", 4)];

#[cfg(windows)]
const ASSET_PATH: &'static str = "./yaobow-assets.zip";
#[cfg(any(linux, mac))]
const ASSET_PATH: &'static str = "../shared/yaobow/yaobow-assets.zip";
#[cfg(vita)]
const ASSET_PATH: &'static str = "ux0:data/yaobow-assets.zip";

const BG_COLOR: [f32; 4] = [241. / 255., 240. / 255., 237. / 255., 1.];
const OUTLINE_FADE_TIME: f32 = 2.;
const UNDERLINE_FIRST_TIME_WAITING: f32 = 4.6;
const UNDERLINE_SHOW_TIME: f32 = 0.4;
const UNDERLINE_MARGIN_TIME: f32 = 1.6;
const OUTLINE_MARGIN_TIME: f32 = (UNDERLINE_SHOW_TIME + UNDERLINE_MARGIN_TIME) * 8.;
const OUTLINE_SHOW_TIME: f32 = (UNDERLINE_SHOW_TIME + UNDERLINE_MARGIN_TIME) * 8.;
const GIT_SHORT_TAG: &str = env!("VERGEN_GIT_DESCRIBE");
