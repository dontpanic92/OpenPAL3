use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use imgui::Condition;
use radiance::comdef::{IApplication, IDirector, IDirectorImpl, ISceneManager};

use crate::ComObject_TitleSelectionDirector;

use super::GameType;

pub struct TitleSelectionDirector {
    app: ComRc<IApplication>,
    selected_game: Rc<RefCell<Option<GameType>>>,
}

ComObject_TitleSelectionDirector!(super::TitleSelectionDirector);

impl IDirectorImpl for TitleSelectionDirector {
    fn activate(&self, scene_manager: ComRc<ISceneManager>) {}

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        let window_size = ui.io().display_size;
        ui.window("main")
            .no_decoration()
            .size(window_size, Condition::Always)
            .position([0.0, 0.0], Condition::Always)
            .collapsible(false)
            .always_auto_resize(true)
            .build(|| {
                for game in GAMES {
                    if ui.button(game.full_name()) {
                        self.selected_game.replace(Some(*game));
                    }
                }
            });

        None
    }
}

impl TitleSelectionDirector {
    pub fn new(app: ComRc<IApplication>, selected_game: Rc<RefCell<Option<GameType>>>) -> Self {
        Self { app, selected_game }
    }
}

const GAMES: &[GameType] = &[GameType::PAL3, GameType::PAL4];
