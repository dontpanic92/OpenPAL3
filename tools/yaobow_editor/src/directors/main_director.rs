use super::{main_content::ContentTabs, DevToolsState};
use imgui::{Condition, Ui};
use mini_fs::{Entries, Entry, EntryKind, StoreExt};
use opengb::{
    asset_manager::AssetManager,
    loaders::mv3_loader::mv3_load_from_file,
    scene::{CvdModelEntity, RoleAnimation, RoleAnimationRepeatMode, RoleEntity},
};
use radiance::{
    audio::AudioEngine,
    input::InputEngine,
    math::Vec3,
    scene::{CoreEntity, CoreScene, Director, Entity, SceneManager},
};
use radiance_editor::{scene::EditorScene, ui::window_content_rect};
use std::{
    cell::RefCell,
    cmp::Ordering,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

pub struct DevToolsDirector {
    shared_self: Weak<RefCell<Self>>,
    input_engine: Rc<RefCell<dyn InputEngine>>,
    asset_mgr: Rc<AssetManager>,
    content_tabs: ContentTabs,
}

impl DevToolsDirector {
    pub fn new(
        input_engine: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        asset_mgr: Rc<AssetManager>,
    ) -> Rc<RefCell<Self>> {
        let mut _self = Rc::new(RefCell::new(Self {
            shared_self: Weak::new(),
            input_engine: input_engine,
            content_tabs: ContentTabs::new(audio_engine),
            asset_mgr,
        }));

        _self.borrow_mut().shared_self = Rc::downgrade(&_self);
        _self
    }

    fn main_window(&mut self, ui: &Ui) -> Option<DevToolsState> {
        let content_rect = window_content_rect(ui);

        ui.window("Files")
            .collapsible(false)
            .resizable(false)
            .size(
                [content_rect.width * 0.3, content_rect.height],
                Condition::Always,
            )
            .position([content_rect.x, content_rect.y], Condition::Always)
            .movable(false)
            .build(|| self.render_tree_nodes(ui, "/"));

        let mut state = None;
        ui.window("Content")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .size(
                [content_rect.width * 0.7, content_rect.height],
                Condition::Always,
            )
            .position(
                [content_rect.x + content_rect.width * 0.3, content_rect.y],
                Condition::Always,
            )
            .movable(false)
            .build(|| state = self.render_content(ui));

        state
    }

    fn render_tree_nodes<P: AsRef<Path>>(&mut self, ui: &Ui, path: P) {
        let entries = self.get_entries(path.as_ref());
        for e in entries {
            let e_path = PathBuf::from(&e.name);
            if e_path.file_name().is_none() {
                continue;
            }

            let e_filename = &format!("{}", e_path.file_name().unwrap().to_str().unwrap());
            let e_fullname = path.as_ref().join(e_path.file_name().unwrap());
            let treenode = ui.tree_node_config(e_filename);

            if e.kind == EntryKind::Dir {
                treenode.build(|| {
                    self.render_tree_nodes(ui, &e_fullname);
                });
            } else {
                treenode.leaf(true).build(|| {
                    if ui.is_item_clicked() {
                        self.content_tabs.open(
                            self.asset_mgr.component_factory(),
                            self.asset_mgr.vfs(),
                            &e_fullname,
                        );
                    }
                });
            }
        }
    }

    fn render_content(&mut self, ui: &Ui) -> Option<DevToolsState> {
        self.content_tabs.render_tabs(ui)
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

    fn load_model(&self, scene_manager: &mut dyn SceneManager, path: PathBuf) {
        let entity = match path
            .extension()
            .map(|e| e.to_str().unwrap().to_ascii_lowercase())
            .as_ref()
            .map(|e| e.as_str())
        {
            Some("mv3") => {
                let mv3file = mv3_load_from_file(self.asset_mgr.vfs(), &path);
                let anim = mv3file.as_ref().map(|f| {
                    RoleAnimation::new(
                        &self.asset_mgr.component_factory(),
                        f,
                        self.asset_mgr.load_mv3_material(f, &path),
                        RoleAnimationRepeatMode::NoRepeat,
                    )
                });

                anim.map(|a| {
                    let mut e = Box::new(CoreEntity::new(
                        RoleEntity::new_from_idle_animation(
                            self.asset_mgr.clone(),
                            "preview",
                            "preview",
                            a,
                        ),
                        "preview".to_string(),
                        true,
                    ));
                    e.set_active(true);
                    e as Box<dyn Entity>
                })
                .ok()
            }
            Some("pol") => Some(Box::new(CoreEntity::new(
                opengb::scene::PolModelEntity::new(
                    &self.asset_mgr.component_factory(),
                    &self.asset_mgr.vfs(),
                    &path,
                ),
                "preview".to_string(),
                true,
            )) as Box<dyn Entity>),
            Some("cvd") => Some(Box::new(CvdModelEntity::create(
                self.asset_mgr.component_factory().clone(),
                &self.asset_mgr.vfs(),
                &path,
                "preview".to_string(),
                true,
            )) as Box<dyn Entity>),
            _ => None,
        };

        let scene = scene_manager.scene_mut().unwrap();
        if let Some(mut e) = entity {
            e.load();
            scene.add_entity(e)
        }

        scene
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(0., 200., 200.))
            .look_at(&Vec3::new(0., 0., 0.));
    }
}

impl Director for DevToolsDirector {
    fn activate(&mut self, _scene_manager: &mut dyn SceneManager) {}

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &imgui::Ui,
        _delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        let state = self.main_window(ui);
        if let Some(DevToolsState::Preview(path)) = state {
            scene_manager.pop_scene();
            scene_manager.push_scene(Box::new(EditorScene::new()));
            self.load_model(scene_manager, path);
        } else if let Some(DevToolsState::PreviewScene { cpk_name, scn_name }) = state {
            scene_manager.pop_scene();
            scene_manager.push_scene(Box::new(CoreScene::new(
                self.asset_mgr
                    .load_scn(cpk_name.as_str(), scn_name.as_str()),
            )));
        }

        None
    }
}
