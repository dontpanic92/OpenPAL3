use crate::preview::previewers::audio::AudioPreviewer;
use crate::preview::previewers::image::ImagePreviewer;
use crate::preview::previewers::models::ModelPreviewer;
use crate::preview::previewers::others::OthersPreviewer;
use crate::preview::previewers::text::TextPreviewer;
use crate::preview::previewers::video::VideoPreviewer;
use crate::preview::previewers::Previewer;
use crate::{preview::panes::ContentPane, GameType};
use imgui::{TabBar, TabBarFlags, TabItem, TabItemFlags, Ui};
use mini_fs::MiniFs;
use radiance::audio::AudioEngine;
use shared::loaders::TextureResolver;
use shared::openpal3::asset_manager::AssetManager;
use std::{path::Path, rc::Rc};

use super::DevToolsState;

pub struct ContentTabs {
    audio_tab: Option<ContentTab>,
    selected_tab: Option<String>,
    previewers: Vec<Box<dyn Previewer>>,
}

impl ContentTabs {
    pub fn new(
        audio_engine: Rc<dyn AudioEngine>,
        asset_mgr: Rc<AssetManager>,
        game_type: GameType,
    ) -> Self {
        Self {
            audio_tab: None,
            selected_tab: None,
            previewers: vec![
                Box::new(TextPreviewer::new()),
                Box::new(ImagePreviewer::new(
                    asset_mgr.component_factory(),
                    game_type,
                )),
                Box::new(AudioPreviewer::new(audio_engine.clone())),
                Box::new(VideoPreviewer::new(
                    asset_mgr.component_factory(),
                    audio_engine.clone(),
                )),
                Box::new(OthersPreviewer::create()),
                Box::new(ModelPreviewer::new(asset_mgr, game_type)),
            ],
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P) {
        for p in &self.previewers {
            if let Some(content_tab) = p.open(vfs, path.as_ref()) {
                self.audio_tab = Some(content_tab);
                return;
            }
        }
    }

    pub fn render_tabs(&mut self, ui: &Ui) -> Option<DevToolsState> {
        let mut state = None;
        TabBar::new("##content_tab_bar")
            .flags(
                TabBarFlags::REORDERABLE
                    | TabBarFlags::FITTING_POLICY_DEFAULT
                    | TabBarFlags::AUTO_SELECT_NEW_TABS,
            )
            .build(ui, || {
                let mut tmp_state = None;
                if let Some(tab) = self.audio_tab.as_mut() {
                    tmp_state = tmp_state.or(tab.render(ui, self.selected_tab.as_ref()));
                }

                self.selected_tab = None;
                state = tmp_state;
            });

        state
    }
}

pub struct ContentTab {
    name: String,
    opened: bool,
    pane: Box<dyn ContentPane>,
}

impl ContentTab {
    pub fn new(name: String, pane: Box<dyn ContentPane>) -> Self {
        Self {
            name,
            opened: true,
            pane,
        }
    }

    pub fn render(&mut self, ui: &Ui, selected_tab: Option<&String>) -> Option<DevToolsState> {
        let selected = selected_tab.map(|name| self.name == *name).unwrap_or(false);
        let flags = if selected {
            TabItemFlags::SET_SELECTED
        } else {
            TabItemFlags::empty()
        };
        let mut opened = self.opened;
        let mut state = None;
        TabItem::new(&format!("{}", &self.name))
            .opened(&mut opened)
            .flags(flags)
            .build(ui, || {
                state = self.pane.render(ui);
            });

        self.opened = opened;

        state
    }
}
