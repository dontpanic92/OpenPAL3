use crosscom::ComRc;
use imgui::{Condition, WindowFlags};
use radiance::{
    comdef::{IApplication, IDirector, IDirectorImpl, ISceneManager},
    scene,
};
use radiance_editor::{director::MainPageDirector, ui::scene_view::SceneViewPlugins};
use shared::config::YaobowConfig;

use crate::{ComObject_WelcomePageDirector, GameType, SceneViewResourceView};

pub struct WelcomePageDirector {
    app: ComRc<IApplication>,
}

ComObject_WelcomePageDirector!(super::WelcomePageDirector);

impl WelcomePageDirector {
    pub fn create(app: ComRc<IApplication>) -> ComRc<IDirector> {
        ComRc::from_object(Self { app })
    }

    fn load_game(&self, game: GameType) -> Option<ComRc<IDirector>> {
        let mut config = YaobowConfig::load("openpal3.toml", "OpenPAL3").unwrap();
        match game {
            GameType::PAL3A => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\PAL3A".to_string();
            }
            GameType::PAL4 => {
                config.asset_path =
                    "F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 4\\".to_string();
            }
            GameType::PAL5 => {
                config.asset_path = "F:\\PAL5\\".to_string();
            }
            GameType::PAL5Q => {
                config.asset_path = "F:\\PAL5Q\\".to_string();
            }
            GameType::SWD5 => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\SWD5".to_string();
            }
            GameType::SWDHC => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\SWDHC".to_string();
            }
            GameType::SWDCF => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\SWDCF".to_string();
            }
            GameType::Gujian => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\Gujian".to_string();
            }
            GameType::Gujian2 => {
                config.asset_path = "F:\\SteamLibrary\\steamapps\\common\\Gujian2".to_string();
            }
            _ => {}
        };

        let resource_view_content = SceneViewResourceView::new(config, self.app.clone(), game);
        let plugins =
            SceneViewPlugins::new(Some(crosscom::ComRc::from_object(resource_view_content)));

        let input = self.app.engine().borrow().input_engine().clone();
        let ui = self.app.engine().borrow().ui_manager();
        let scene_manager = self.app.engine().borrow().scene_manager();
        let director = MainPageDirector::create(Some(plugins), ui, input, scene_manager);

        Some(director)
    }
}

impl IDirectorImpl for WelcomePageDirector {
    fn activate(&self) -> crosscom::Void {}

    fn update(&self, _: f32) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        let vpos = unsafe { (*imgui::sys::igGetMainViewport()).Pos };
        let vsize = unsafe { (*imgui::sys::igGetMainViewport()).Size };
        let ui = self.app.engine().borrow().ui_manager().ui();
        let em = ui.current_font_size();
        let columns = [
            vec![
                GameType::PAL3,
                GameType::PAL3A,
                GameType::PAL4,
                GameType::PAL5,
                GameType::PAL5Q,
            ],
            vec![GameType::Gujian, GameType::Gujian2],
            vec![GameType::SWD5, GameType::SWDHC, GameType::SWDCF],
        ];

        let mut next_director = None;
        ui.window("Welcome")
            .collapsible(false)
            .resizable(false)
            .size([vsize.x, vsize.y], Condition::Always)
            .position([vpos.x, vpos.y], Condition::Always)
            .flags(WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS)
            .movable(false)
            .title_bar(false)
            .draw_background(false)
            .build(|| {
                let w = em * 50.;
                let h = em * 30.;
                ui.window("WelcomePane")
                    .collapsible(false)
                    .resizable(false)
                    .size([w, h], Condition::Always)
                    .position(
                        [(vpos.x + vsize.x - w) / 2., (vpos.y + vsize.y - h) / 2.],
                        Condition::Always,
                    )
                    .movable(false)
                    .title_bar(false)
                    .build(|| {
                        let font = ui.push_font(ui.fonts().fonts()[0]);
                        ui.text("妖弓编辑器");
                        font.pop();

                        ui.dummy([0., 2. * em]);

                        let table = ui.begin_table("t1", 3).unwrap();

                        let mut row = 0;
                        loop {
                            let mut completed = true;
                            for i in 0..3 {
                                ui.table_next_column();
                                if let Some(game) = columns[i].get(row) {
                                    if ui.button_with_size(
                                        format!("运行《{}》编辑器", game.full_name()),
                                        [-std::f32::MIN_POSITIVE, em * 3.],
                                    ) {
                                        next_director = self.load_game(*game);
                                    }
                                    completed = false;
                                } else {
                                    ui.text("");
                                }
                            }

                            row += 1;

                            if completed {
                                break;
                            }
                        }

                        table.end();
                    });
            });

        next_director
    }
}
