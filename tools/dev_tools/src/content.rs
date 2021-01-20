use imgui::{im_str, TabBar, TabBarFlags, TabItem, Ui};
use mini_fs::MiniFs;
use opengb::utilities::StoreExt2;
use radiance::audio::{AudioEngine, Codec};
use std::{path::Path, rc::Rc};

use crate::components::{AudioPane, ContentPane};

pub struct ContentTabs {
    audio_engine: Rc<dyn AudioEngine>,

    tabs: Vec<ContentTab>,
    audio_tab: Option<ContentTab>,
}

impl ContentTabs {
    pub fn new(audio_engine: Rc<dyn AudioEngine>) -> Self {
        Self {
            audio_engine,
            tabs: vec![],
            audio_tab: None,
        }
    }

    pub fn open<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P) {
        let extension = path
            .as_ref()
            .extension()
            .map(|e| e.to_str().unwrap().to_ascii_lowercase());

        match extension.as_ref().map(|e| e.as_str()) {
            Some("mp3") | Some("wav") => self.open_audio(vfs, path, &extension.unwrap()),
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
                Box::new(AudioPane::new(self.audio_engine.as_ref(), data, codec, path.as_ref().to_owned())),
            ));
        }
    }

    pub fn render_tabs(&mut self, ui: &Ui) {
        self.tabs.drain_filter(|tab| tab.opened == false);
        if Some(true) == self.audio_tab.as_ref().map(|t| t.opened == false) {
            self.audio_tab = None;
        }

        TabBar::new(im_str!("##content_tab_bar"))
            .flags(TabBarFlags::REORDERABLE | TabBarFlags::FITTING_POLICY_DEFAULT)
            .build(ui, || {
                if let Some(tab) = self.audio_tab.as_mut() {
                    tab.render(ui);
                }

                for tab in &mut self.tabs {
                    tab.render(ui);
                }
            });
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

    pub fn render(&mut self, ui: &Ui) {
        let mut opened = self.opened;
        TabItem::new(&im_str!("{}", &self.name))
            .opened(&mut opened)
            .build(ui, || {
                self.pane.render(ui);
            });

        self.opened = opened;
    }
}
