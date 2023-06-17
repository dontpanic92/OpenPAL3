use crate::{ComObject_DevToolsDirector, GameType};

use super::{main_content::ContentTabs, DevToolsState};
use crosscom::ComRc;
use imgui::Ui;
use mini_fs::{Entries, Entry, EntryKind, StoreExt};
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    math::Vec3,
    scene::CoreScene,
};
use radiance_editor::ui::window_content_rect;
use shared::{
    loaders::{Pal4TextureResolver, Pal5TextureResolver, Swd5TextureResolver, TextureResolver},
    openpal3::asset_manager::AssetManager,
};
use std::{
    cell::RefCell,
    cmp::Ordering,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct DevToolsDirector {
    asset_mgr: Rc<AssetManager>,
    content_tabs: RefCell<ContentTabs>,
}

ComObject_DevToolsDirector!(super::DevToolsDirector);

impl DevToolsDirector {
    pub fn new(
        audio_engine: Rc<dyn AudioEngine>,
        asset_mgr: Rc<AssetManager>,
        game_type: GameType,
    ) -> ComRc<IDirector> {
        let texture_resolver: Rc<dyn TextureResolver> = match game_type {
            GameType::PAL3 | GameType::PAL4 => Rc::new(Pal4TextureResolver {}),
            GameType::PAL5 | GameType::PAL5Q => Rc::new(Pal5TextureResolver {}),
            _ => Rc::new(Swd5TextureResolver {}),
        };

        ComRc::from_object(Self {
            content_tabs: RefCell::new(ContentTabs::new(
                audio_engine,
                asset_mgr.clone(),
                game_type,
                texture_resolver,
            )),
            asset_mgr,
        })
    }

    fn main_window(&self, ui: &Ui) -> Option<DevToolsState> {
        let content_rect = window_content_rect(ui);
        let h_layout = content_rect.width > 800.;

        let sizes = {
            if h_layout {
                [
                    [content_rect.width * 0.3, content_rect.height],
                    [std::f32::MIN_POSITIVE, content_rect.height],
                ]
            } else {
                [
                    [content_rect.width, content_rect.height * 0.6],
                    [content_rect.width, std::f32::MIN_POSITIVE],
                ]
            }
        };

        ui.child_window("Files")
            .size(sizes[0])
            .build(|| self.render_tree_nodes(ui, "/"));

        if h_layout {
            ui.same_line();
        }

        let mut state = None;
        ui.child_window("Content")
            .size(sizes[1])
            .build(|| state = self.render_content(ui));

        state
    }

    fn render_tree_nodes<P: AsRef<Path>>(&self, ui: &Ui, path: P) {
        let entries = self.get_entries(path.as_ref());
        for e in entries {
            let e_path = PathBuf::from(&e.name);
            if e_path.file_name().is_none() {
                continue;
            }

            let e_filename = &format!("{}", e_path.file_name().unwrap().to_str().unwrap());
            let e_fullname = path.as_ref().join(e_filename);

            let treenode = ui.tree_node_config(e_filename);

            if e.kind == EntryKind::Dir {
                treenode.build(|| {
                    self.render_tree_nodes(ui, &e_fullname);
                });
            } else {
                treenode.leaf(true).build(|| {
                    if ui.is_item_clicked() {
                        self.content_tabs
                            .borrow_mut()
                            .open(self.asset_mgr.vfs(), &e_fullname);
                    }
                });
            }
        }
    }

    fn render_content(&self, ui: &Ui) -> Option<DevToolsState> {
        self.content_tabs.borrow_mut().render_tabs(ui)
    }

    fn get_entries<P: AsRef<Path>>(&self, path: P) -> Vec<Entry> {
        let entries: Entries = self.asset_mgr.vfs().entries(path.as_ref()).unwrap();
        let mut entries: Vec<Entry> = entries.map(|e| e.unwrap()).collect();
        entries.sort_by(|a, b| match (a.kind, b.kind) {
            (EntryKind::Dir, EntryKind::Dir) => a.name.cmp(&b.name),
            (EntryKind::File, EntryKind::File) => a.name.cmp(&b.name),
            (EntryKind::Dir, EntryKind::File) => Ordering::Less,
            (EntryKind::File, EntryKind::Dir) => Ordering::Greater,
        });

        entries
    }
}

impl IDirectorImpl for DevToolsDirector {
    fn activate(&self, _scene_manager: ComRc<ISceneManager>) {}

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        ui: &imgui::Ui,
        _delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        let state = self.main_window(ui);
        match state {
            Some(DevToolsState::PreviewScene { cpk_name, scn_name }) => {
                scene_manager.pop_scene();

                let scene = self
                    .asset_mgr
                    .load_scn(cpk_name.as_str(), scn_name.as_str());
                scene
                    .camera()
                    .borrow_mut()
                    .transform_mut()
                    .set_position(&Vec3::new(0., 500., 500.))
                    .look_at(&Vec3::new(0., 0., 0.));
                scene_manager.push_scene(scene);
            }
            Some(DevToolsState::PreviewEntity(entity)) => {
                let scene = CoreScene::create();
                scene_manager.pop_scene();
                scene_manager.push_scene(scene.clone());

                entity.load();
                scene.add_entity(entity);
                scene
                    .camera()
                    .borrow_mut()
                    .transform_mut()
                    .set_position(&Vec3::new(0., 200., 200.))
                    .look_at(&Vec3::new(0., 0., 0.));
            }
            Some(DevToolsState::MainWindow) => {}
            None => {}
        }

        None
    }
}
