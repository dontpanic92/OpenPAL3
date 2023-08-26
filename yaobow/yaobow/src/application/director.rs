use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use imgui::{ColorStackToken, Condition, StyleColor};
use mini_fs::{LocalFs, MiniFs, ZipFs};
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
};

use crate::ComObject_TitleSelectionDirector;

use super::GameType;

pub struct TitleSelectionDirector {
    audio: Rc<dyn AudioEngine>,
    selected_game: Rc<RefCell<Option<GameType>>>,
    hovered_game: RefCell<GameType>,
    vfs: Option<MiniFs>,
    dpi_scale: f32,
    bgm_source: RefCell<Option<Box<dyn radiance::audio::AudioMemorySource>>>,
}

ComObject_TitleSelectionDirector!(super::TitleSelectionDirector);

impl IDirectorImpl for TitleSelectionDirector {
    fn activate(&self, scene_manager: ComRc<ISceneManager>) {
        if let Some(vfs) = &self.vfs {
            let data = vfs.read_to_end("/music/Grace.ogg").unwrap();
            let mut bgm_source = self.audio.create_source();
            bgm_source.set_data(data, radiance::audio::Codec::Ogg);
            bgm_source.play(true);

            self.bgm_source.replace(Some(bgm_source));
        }
    }

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        if let Some(bgm_source) = self.bgm_source.borrow_mut().as_mut() {
            bgm_source.update();
        }

        let window_size = ui.io().display_size;
        ui.window("main")
            .no_decoration()
            .size(window_size, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .collapsible(false)
            .always_auto_resize(true)
            .build(|| {
                self.game_list(ui, GAMES);
            });

        None
    }
}

impl TitleSelectionDirector {
    pub fn new(
        audio: Rc<dyn AudioEngine>,
        selected_game: Rc<RefCell<Option<GameType>>>,
        dpi_scale: f32,
    ) -> Self {
        let vfs = Self::load_vfs();
        Self {
            audio,
            selected_game,
            hovered_game: RefCell::new(GameType::PAL3),
            vfs,
            dpi_scale,
            bgm_source: RefCell::new(None),
        }
    }

    fn load_vfs() -> Option<MiniFs> {
        let mut vfs = MiniFs::new(false);
        let zip = PathBuf::from(ASSET_PATH);
        let local1 = PathBuf::from("./yaobow-assets");
        let local2 = PathBuf::from("../yaobow-assets");
        println!("good2 {:?}", std::env::current_dir().unwrap());

        if Path::exists(&zip) {
            let local = ZipFs::new(std::fs::File::open(zip).unwrap());
            vfs = vfs.mount(PathBuf::from("/"), local);
            Some(vfs)
        } else if Path::exists(&local1) {
            let local = LocalFs::new(&local1);
            vfs = vfs.mount(PathBuf::from("/"), local);
            Some(vfs)
        } else if Path::exists(&local2) {
            println!("good3 {:?}", std::env::current_dir().unwrap());
            let local = LocalFs::new(&local2);
            vfs = vfs.mount(PathBuf::from("/"), local);
            Some(vfs)
        } else {
            None
        }
    }

    fn game_list(&self, ui: &imgui::Ui, games: &[GameType]) {
        let window_size = ui.io().display_size;
        let game_list_width = window_size[0] * 0.3;
        let item_width = 200.0;
        let item_start_x = game_list_width - item_width;
        let item_height = 80.0 * self.dpi_scale;
        let all_height = item_height * games.len() as f32;
        let item_start_y = (window_size[1] - all_height) / 2.0;

        let mut cursor_y = item_start_y;
        for game in games {
            ui.set_cursor_pos([item_start_x, cursor_y]);
            let _tokens = self.push_button_colors(ui);
            let _text_color_token = if *self.hovered_game.borrow() != *game {
                Some(ui.push_style_color(StyleColor::Text, [0.5, 0.5, 0.5, 1.]))
            } else {
                None
            };

            if ui.button_with_size(game.full_name(), [item_width, item_height]) {
                self.selected_game.replace(Some(*game));
            }

            if ui.is_item_hovered() {
                self.hovered_game.replace(*game);
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

const GAMES: &[GameType] = &[GameType::PAL3, GameType::PAL4];

#[cfg(windows)]
const ASSET_PATH: &'static str = "./yaobow-assets.zip";
#[cfg(any(linux, mac))]
const ASSET_PATH: &'static str = "../shared/yaobow/yaobow-assets.zip";
#[cfg(vita)]
const ASSET_PATH: &'static str = "ux0:data/yaobow-assets.zip";
