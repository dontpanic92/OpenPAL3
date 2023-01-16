use chardet::{charset2encoding, detect};
use common::store_ext::StoreExt2;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use fileformats::dff::read_dff;
use fileformats::mv3::read_mv3;
use fileformats::pol::read_pol;
use image::ImageFormat;
use imgui::{TabBar, TabBarFlags, TabItem, TabItemFlags, Ui};
use mini_fs::{MiniFs, StoreExt};
use native_dialog::{FileDialog, MessageDialog, MessageType};
use opengb::loaders::{
    cvd_loader::cvd_load_from_file, nav_loader::nav_load_from_file, sce_loader::sce_load_from_file,
    scn_loader::scn_load_from_file,
};
use radiance::{
    audio::AudioEngine, audio::Codec as AudioCodec, rendering::ComponentFactory,
    video::Codec as VideoCodec,
};
use serde::Serialize;
use shared::fs::cpk::CpkArchive;
use std::io::Read;
use std::{io::BufReader, path::Path, rc::Rc};
use wavefront_obj::mtl::MtlSet;

use shared::exporters::{
    mv3_obj_exporter::export_mv3_to_obj,
    obj_exporter::{export_to_file, ObjSet},
    pol_obj_exporter::export_pol_to_obj,
};

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

    const NONE_EXPORT: Option<fn()> = Option::<fn()>::None;

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
            Some("mp3" | "wav" | "smp") => self.open_audio(vfs, path, &extension.unwrap()),
            Some("bik" | "mp4") => self.open_video(factory, vfs, path, &extension.unwrap()),
            Some("tga" | "png" | "dds") => self.open_image(factory, vfs, path),
            Some("scn") => self.open_scn(vfs, path),
            Some("nav") => self.open_json_from(
                path.as_ref(),
                || Some(nav_load_from_file(vfs, path.as_ref())),
                true,
                Self::NONE_EXPORT,
            ),
            Some("sce") => self.open_json_from(
                path.as_ref(),
                || Some(sce_load_from_file(vfs, path.as_ref())),
                true,
                Self::NONE_EXPORT,
            ),
            Some("mv3") => {
                let mv3_file = read_mv3(&mut BufReader::new(vfs.open(&path).unwrap())).ok();
                let name = path
                    .as_ref()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                self.open_json_from(
                    path.as_ref(),
                    || read_mv3(&mut BufReader::new(vfs.open(&path).unwrap())).ok(),
                    true,
                    Some(move || {
                        Self::export(|| {
                            export_mv3_to_obj(mv3_file.clone().as_ref(), name.clone().as_str())
                        })
                    }),
                )
            }
            Some("cvd") => self.open_json_from(
                path.as_ref(),
                || cvd_load_from_file(vfs, path.as_ref()).ok(),
                true,
                Self::NONE_EXPORT,
            ),
            Some("pol") => {
                let pol_file = read_pol(&mut BufReader::new(vfs.open(&path).unwrap())).ok();
                let name = path
                    .as_ref()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                self.open_json_from(
                    path.as_ref(),
                    || read_pol(&mut BufReader::new(vfs.open(&path).unwrap())).ok(),
                    true,
                    Some(move || {
                        Self::export(|| {
                            export_pol_to_obj(pol_file.clone().as_ref(), name.clone().as_str())
                        })
                    }),
                )
            }
            Some("dff") => self.open_json_from(
                path.as_ref(),
                || {
                    let mut data = vec![];
                    vfs.open(&path).unwrap().read_to_end(&mut data).unwrap();
                    read_dff(&data).ok()
                },
                true,
                Self::NONE_EXPORT,
            ),
            Some("h" | "asm" | "ini" | "txt" | "conf") => self.open_plain_text(vfs, path.as_ref()),
            _ => {}
        }
    }

    pub fn open_audio<P: AsRef<Path>>(&mut self, vfs: &MiniFs, path: P, extension: &str) {
        let codec = match extension {
            "mp3" => Some(AudioCodec::Mp3),
            "smp" => Some(AudioCodec::Mp3),
            "wav" => Some(AudioCodec::Wav),
            _ => None,
        };

        if let Ok(mut data) = vfs.read_to_end(&path) {
            if extension == "smp" {
                let mut cpk = CpkArchive::load(std::io::Cursor::new(&data)).unwrap();
                let name = cpk.file_names[0].clone();
                let mut content = cpk.open_str(&name).unwrap().content();
                let size = content.len() & 0xFFFFFFFC;
                content.resize(size, 0);
                println!("filename {} {} {:?}", name, content.len(), cpk.entries);

                data = xxtea::decrypt_raw(
                    &content,
                    "Vampire.C.J at Softstar Technology (ShangHai) Co., Ltd",
                );
            }

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
            Box::new(TextPane::new(
                content,
                path.as_ref().to_owned(),
                Some(DevToolsState::PreviewScene {
                    cpk_name: scn_file.cpk_name.clone(),
                    scn_name: scn_file.scn_name.clone(),
                }),
                None,
            ))
        });
    }

    pub fn open_json_from<
        P: AsRef<Path>,
        O: Serialize,
        F: Fn() -> Option<O>,
        FExport: Fn() + 'static + Clone,
    >(
        &mut self,
        path: P,
        loader: F,
        preview: bool,
        export: Option<FExport>,
    ) {
        self.open_text2(
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
            export,
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
        self.open_text2::<P, F, fn()>(path, loader, preview_state, None)
    }

    pub fn open_text2<P: AsRef<Path>, F: Fn() -> String, FExport: Fn() + 'static + Clone>(
        &mut self,
        path: P,
        loader: F,
        preview_state: Option<DevToolsState>,
        export: Option<FExport>,
    ) {
        let tab_name = path.as_ref().to_string_lossy().to_string();
        self.show_or_add_tab(tab_name, || {
            let content = loader();
            Box::new(TextPane::new(
                content,
                path.as_ref().to_owned(),
                preview_state.clone(),
                export
                    .clone()
                    .and_then(|e| Some(Box::new(e) as Box<dyn Fn()>)),
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

    fn export<F: Fn() -> Option<(ObjSet, MtlSet)>>(do_export: F) {
        let path = FileDialog::new()
            .add_filter("Wavefront OBJ", &["obj"])
            .show_save_single_file()
            .unwrap();

        let path = match path {
            Some(path) => path,
            None => return,
        };

        let obj = do_export();
        if let Some(obj) = obj {
            if let Ok(()) = export_to_file(&obj.0, &obj.1, path) {
                MessageDialog::new()
                    .set_type(MessageType::Info)
                    .set_title(crate::TITLE)
                    .set_text("导出成功")
                    .show_alert()
                    .unwrap();

                return;
            }
        }

        MessageDialog::new()
            .set_type(MessageType::Error)
            .set_title(crate::TITLE)
            .set_text("导出失败")
            .show_alert()
            .unwrap();
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
