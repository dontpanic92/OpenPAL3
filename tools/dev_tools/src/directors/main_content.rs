use chardet::{charset2encoding, detect};
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use image::ImageFormat;
use imgui::{TabBar, TabBarFlags, TabItem, TabItemFlags, Ui};
use mini_fs::MiniFs;
use opengb::{
    loaders::{
        cvd_loader::cvd_load_from_file, mv3_loader::mv3_load_from_file,
        nav_loader::nav_load_from_file, pol_loader::pol_load_from_file,
        sce_loader::sce_load_from_file, scn_loader::scn_load_from_file,
    },
    utilities::StoreExt2,
};
use radiance::{
    audio::AudioEngine, audio::Codec as AudioCodec, rendering::ComponentFactory,
    video::Codec as VideoCodec,
};
use serde::Serialize;
use std::{path::Path, rc::Rc};

use super::{
    components::{AudioPane, ContentPane, ImagePane, TextPane, VideoPane},
    DevToolsState,
};

pub struct ContentTabs {
    audio_engine: Rc<dyn AudioEngine>,
    tabs: Vec<ContentTab>,
    audio_tab: Option<ContentTab>,
    video_tab: Option<ContentTab>,
    selected_tab: Option<String>,
}

impl ContentTabs {
    pub fn new(audio_engine: Rc<dyn AudioEngine>) -> Self {
        Self {
            audio_engine,
            tabs: vec![],
            audio_tab: None,
            video_tab: None,
            selected_tab: None,
        }
    }

    pub fn open<P: AsRef<Path>>(
        &mut self,
        factory: Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
    ) {
        let extension = path
            .as_ref()
            .extension()
            .map(|e| e.to_str().unwrap().to_ascii_lowercase());

        match extension.as_ref().map(|e| e.as_str()) {
            Some("mp3" | "wav") => self.open_audio(vfs, path, &extension.unwrap()),
            Some("bik" | "mp4") => self.open_video(factory, vfs, path, &extension.unwrap()),
            Some("tga" | "png" | "dds") => self.open_image(factory, vfs, path),
            Some("scn") => self.open_scn(vfs, path),
            Some("nav") => self.open_json_from(
                path.as_ref(),
                || Some(nav_load_from_file(vfs, path.as_ref())),
                true,
            ),
            Some("sce") => self.open_json_from(
                path.as_ref(),
                || Some(sce_load_from_file(vfs, path.as_ref())),
                true,
            ),
            Some("mv3") => self.open_json_from(
                path.as_ref(),
                || mv3_load_from_file(vfs, path.as_ref()).ok(),
                true,
            ),
            Some("cvd") => self.open_json_from(
                path.as_ref(),
                || cvd_load_from_file(vfs, path.as_ref()).ok(),
                true,
            ),
            Some("pol") => self.open_json_from(
                path.as_ref(),
                || pol_load_from_file(vfs, path.as_ref()).ok(),
                true,
            ),
            Some("h" | "asm" | "ini" | "txt" | "conf") => self.open_plain_text(vfs, path.as_ref()),
            _ => {}
        }
    }

    pub fn open_audio<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P, extension: &str) {
        let codec = match extension {
            "mp3" => Some(AudioCodec::Mp3),
            "wav" => Some(AudioCodec::Wav),
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

    pub fn open_video<P: AsRef<Path>>(
        &mut self,
        factory: Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
        extension: &str,
    ) {
        let codec = match extension {
            "bik" => Some(VideoCodec::Bik),
            _ => None,
        };
        if let Ok(data) = vfs.read_to_end(&path) {
            self.video_tab = Some(ContentTab::new(
                "video".to_string(),
                Box::new(VideoPane::new(
                    factory,
                    data,
                    codec,
                    path.as_ref().to_owned(),
                )),
            ));
        }
    }

    pub fn open_image<P: AsRef<Path>>(
        &mut self,
        factory: Rc<dyn ComponentFactory>,
        vfs: &MiniFs,
        path: P,
    ) {
        let tab_name = path.as_ref().to_string_lossy().to_string();
        self.show_or_add_tab(tab_name, || {
            let image = vfs
                .read_to_end(&path)
                .ok()
                .and_then(|b| {
                    image::load_from_memory(&b)
                        .or_else(|_| image::load_from_memory_with_format(&b, ImageFormat::Tga))
                        .or_else(|err| Err(err))
                        .ok()
                })
                .and_then(|img| Some(img.to_rgba8()));
            Box::new(ImagePane::new(factory.clone(), image))
        });
    }

    pub fn open_scn<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P) {
        let scn_file = scn_load_from_file(vfs, path.as_ref());

        let tab_name = path.as_ref().to_string_lossy().to_string();
        self.show_or_add_tab(tab_name, || {
            let content = serde_json::to_string_pretty(&scn_file)
                .unwrap_or("Cannot serialize as Json".to_string());
            Box::new(TextPane::new(content, path.as_ref().to_owned(), None))
        });
    }

    pub fn open_json_from<P: AsRef<Path>, O: Serialize, F: Fn() -> Option<O>>(
        &mut self,
        path: P,
        loader: F,
        preview: bool,
    ) {
        self.open_text(
            path.as_ref(),
            || {
                loader()
                    .map(|obj| {
                        serde_json::to_string_pretty(&obj)
                            .unwrap_or("Cannot serialize as Json".to_string())
                    })
                    .unwrap_or("Cannot load this file".to_string())
            },
            if preview {
                Some(DevToolsState::Preview(path.as_ref().to_owned()))
            } else {
                None
            },
        );
    }

    pub fn open_plain_text<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P) {
        self.open_text(
            path.as_ref(),
            || {
                vfs.read_to_end(path.as_ref())
                    .and_then(|v| {
                        let result = detect(&v);
                        let coder = encoding_from_whatwg_label(charset2encoding(&result.0))
                            .unwrap_or(encoding::all::GBK);
                        Ok(coder.decode(&v, DecoderTrap::Ignore).unwrap_or(
                            "Cannot read the file as GBK encoded text content".to_string(),
                        ))
                    })
                    .unwrap_or("Cannot open this file".to_string())
            },
            None,
        );
    }

    pub fn open_text<P: AsRef<Path>, F: Fn() -> String>(
        &mut self,
        path: P,
        loader: F,
        preview_state: Option<DevToolsState>,
    ) {
        let tab_name = path.as_ref().to_string_lossy().to_string();
        self.show_or_add_tab(tab_name, || {
            let content = loader();
            Box::new(TextPane::new(
                content,
                path.as_ref().to_owned(),
                preview_state.clone(),
            ))
        });
    }

    pub fn render_tabs(&mut self, ui: &Ui) -> Option<DevToolsState> {
        self.tabs.drain_filter(|tab| tab.opened == false);
        if Some(true) == self.audio_tab.as_ref().map(|t| t.opened == false) {
            self.audio_tab = None;
        }

        if Some(true) == self.video_tab.as_ref().map(|t| t.opened == false) {
            self.video_tab = None;
        }

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

                if let Some(tab) = self.video_tab.as_mut() {
                    tmp_state = tmp_state.or(tab.render(ui, self.selected_tab.as_ref()));
                }

                for tab in &mut self.tabs {
                    tmp_state = tmp_state.or(tab.render(ui, self.selected_tab.as_ref()));
                }

                self.selected_tab = None;
                state = tmp_state;
            });

        state
    }

    fn show_or_add_tab<F: Fn() -> Box<dyn ContentPane>>(&mut self, tab_name: String, new_pane: F) {
        let tab = self.tabs.iter().find(|t| t.name == tab_name);
        match tab {
            None => self
                .tabs
                .push(ContentTab::new(tab_name.to_string(), new_pane())),
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
