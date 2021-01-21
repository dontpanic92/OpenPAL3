use imgui::{TabBar, TabBarFlags, TabItem, TabItemFlags, Ui, im_str};
use mini_fs::MiniFs;
use opengb::{loaders::scn_loader::scn_load_from_file, utilities::StoreExt2};
use radiance::audio::{AudioEngine, Codec};
use std::{path::Path, rc::Rc};

use crate::components::{AudioPane, ContentPane, TextPane};

pub struct ContentTabs {
    audio_engine: Rc<dyn AudioEngine>,
    tabs: Vec<ContentTab>,
    audio_tab: Option<ContentTab>,
    selected_tab: Option<String>,
}

impl ContentTabs {
    pub fn new(audio_engine: Rc<dyn AudioEngine>) -> Self {
        Self {
            audio_engine,
            tabs: vec![],
            audio_tab: None,
            selected_tab: None,
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P) {
        let extension = path
            .as_ref()
            .extension()
            .map(|e| e.to_str().unwrap().to_ascii_lowercase());

        match extension.as_ref().map(|e| e.as_str()) {
            Some("mp3") | Some("wav") => self.open_audio(vfs, path, &extension.unwrap()),
            Some("scn") => self.open_scn(vfs, path),
            _ => {}
        }
    }

    pub fn open_audio<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P, extension: &str) {
        let codec = match extension {
            "mp3" => Some(Codec::Mp3),
            "wav" => Some(Codec::Wav),
            _ => None,
        };

        if let Ok(data) = vfs.read_to_end(&path) {
            self.audio_tab = Some(ContentTab::new(
                "audio".to_string(),
                Box::new(AudioPane::new(
                    self.audio_engine.as_ref(),
                    data,
                    codec,
                    path.as_ref().to_owned(),
                )),
            ));
        }
    }

    pub fn open_scn<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P) {
        let scn_file = scn_load_from_file(vfs, path.as_ref());

        let tab_name = path.as_ref().to_string_lossy().to_string();
        self.show_or_add_tab(tab_name, || {
            Box::new(TextPane::new(format!("{:?}", scn_file), path.as_ref().to_owned()))
        });
    }

    pub fn render_tabs(&mut self, ui: &Ui) {
        self.tabs.drain_filter(|tab| tab.opened == false);
        if Some(true) == self.audio_tab.as_ref().map(|t| t.opened == false) {
            self.audio_tab = None;
        }

        TabBar::new(im_str!("##content_tab_bar"))
            .flags(TabBarFlags::REORDERABLE | TabBarFlags::FITTING_POLICY_DEFAULT | TabBarFlags::AUTO_SELECT_NEW_TABS)
            .build(ui, || {
                if let Some(tab) = self.audio_tab.as_mut() {
                    tab.render(ui, self.selected_tab.as_ref());
                }

                for tab in &mut self.tabs {
                    tab.render(ui, self.selected_tab.as_ref());
                }

                self.selected_tab = None;
            });
    }

    fn show_or_add_tab<F: Fn() -> Box<dyn ContentPane>>(&mut self, tab_name: String, new_pane: F) {
        let tab = self.tabs.iter().find(|t| t.name == tab_name);
        match tab {
            None => self.tabs.push(ContentTab::new(
                tab_name.to_string(),
                new_pane()),
            ),
            Some(_) => self.selected_tab = Some(tab_name),
        }
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

    pub fn render(&mut self, ui: &Ui, selected_tab: Option<&String>) {
        let selected = selected_tab.map(|name| self.name == *name).unwrap_or(false);
        let flags = if selected { TabItemFlags::SET_SELECTED } else { TabItemFlags::empty() };
        let mut opened = self.opened;
        TabItem::new(&im_str!("{}", &self.name))
            .opened(&mut opened)
            .flags(flags)
            .build(ui, || {
                self.pane.render(ui);
            });

        self.opened = opened;
    }
}
