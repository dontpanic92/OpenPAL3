use crate::{ComObject_DevToolsDirector, GameType};

use super::{main_content::ContentTabs, DevToolsAssetLoader, DevToolsState};
use crosscom::ComRc;
use imgui::Ui;
use mini_fs::{Entries, Entry, EntryKind, StoreExt};
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    math::Vec3,
    radiance::UiManager,
    scene::CoreScene,
};
use radiance_editor::ui::window_content_rect;
use std::{
    cell::RefCell,
    cmp::Ordering,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct DevToolsDirector {
    scene_manager: ComRc<ISceneManager>,
    asset_mgr: DevToolsAssetLoader,
    ui: Rc<UiManager>,
    content_tabs: RefCell<ContentTabs>,
    cache: RefCell<lru::LruCache<String, Vec<Entry>>>,
}

ComObject_DevToolsDirector!(super::DevToolsDirector);

impl DevToolsDirector {
    pub fn new(
        audio_engine: Rc<dyn AudioEngine>,
        scene_manager: ComRc<ISceneManager>,
        asset_mgr: DevToolsAssetLoader,
        ui: Rc<UiManager>,
        game_type: GameType,
    ) -> ComRc<IDirector> {
        ComRc::from_object(Self {
            scene_manager,
            content_tabs: RefCell::new(ContentTabs::new(
                audio_engine,
                asset_mgr.clone(),
                game_type,
            )),
            ui,
            asset_mgr,
            cache: RefCell::new(lru::LruCache::new(20)),
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
        let key = path.as_ref().to_string_lossy().to_string();
        self.cache
            .borrow_mut()
            .get_or_insert(key, || {
                let entries: Entries = self.asset_mgr.vfs().entries(path.as_ref()).unwrap();
                let mut entries: Vec<Entry> = entries.map(|e| e.unwrap()).collect();
                entries.sort_by(|a, b| match (a.kind, b.kind) {
                    (EntryKind::Dir, EntryKind::Dir) => a.name.cmp(&b.name),
                    (EntryKind::File, EntryKind::File) => a.name.cmp(&b.name),
                    (EntryKind::Dir, EntryKind::File) => Ordering::Less,
                    (EntryKind::File, EntryKind::Dir) => Ordering::Greater,
                });

                entries
            })
            .unwrap()
            .clone()
    }
}

impl IDirectorImpl for DevToolsDirector {
    fn activate(&self) {}

    fn update(&self, _delta_sec: f32) -> Option<ComRc<IDirector>> {
        let ui = self.ui.ui();
        let state = self.main_window(ui);
        match state {
            Some(DevToolsState::PreviewScene { cpk_name, scn_name }) => {
                self.scene_manager.pop_scene();

                let scene = self
                    .asset_mgr
                    .pal3()
                    .unwrap()
                    .load_scn(cpk_name.as_str(), scn_name.as_str());
                scene
                    .camera()
                    .borrow_mut()
                    .transform_mut()
                    .set_position(&Vec3::new(0., 500., 500.))
                    .look_at(&Vec3::new(0., 0., 0.));
                self.scene_manager.push_scene(scene);
            }
            Some(DevToolsState::PreviewEntity(entity)) => {
                let scene = CoreScene::create();
                self.scene_manager.pop_scene();
                self.scene_manager.push_scene(scene.clone());

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
