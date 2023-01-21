use super::{main_content::ContentTabs, DevToolsState};
use imgui::{Condition, Ui};
use mini_fs::{Entries, Entry, EntryKind, StoreExt};
use opengb::{
    asset_manager::AssetManager,
    loaders::pol::create_entity_from_pol_model,
    scene::{
        create_animated_mesh_from_mv3, create_entity_from_cvd_model, create_mv3_entity,
        RoleController,
    },
};
use radiance::{
    audio::AudioEngine,
    interfaces::{IAnimatedMeshComponent, IComponent},
    math::Vec3,
    scene::{CoreScene, Director, Scene, SceneManager},
};
use radiance_editor::{scene::EditorScene, ui::window_content_rect};
use shared::loaders::dff::create_entity_from_dff_model;
use std::{
    cell::RefCell,
    cmp::Ordering,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

pub struct DevToolsDirector {
    shared_self: Weak<RefCell<Self>>,
    asset_mgr: Rc<AssetManager>,
    content_tabs: ContentTabs,
}

impl DevToolsDirector {
    pub fn new(
        audio_engine: Rc<dyn AudioEngine>,
        asset_mgr: Rc<AssetManager>,
    ) -> Rc<RefCell<Self>> {
        let mut _self = Rc::new(RefCell::new(Self {
            shared_self: Weak::new(),
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
                let anim = create_animated_mesh_from_mv3(
                    &self.asset_mgr.component_factory(),
                    self.asset_mgr.vfs(),
                    path,
                );

                anim.map(|a| {
                    let e = create_mv3_entity(
                        self.asset_mgr.clone(),
                        "preview",
                        "preview",
                        "preview".to_string(),
                        true,
                    )
                    .unwrap();

                    e.add_component(
                        IAnimatedMeshComponent::uuid(),
                        a.query_interface::<IComponent>().unwrap(),
                    );

                    let r = RoleController::try_get_role_model(e.clone()).unwrap();
                    r.get().set_active(e.clone(), true);
                    e
                })
                .ok()
            }
            Some("pol") => Some(create_entity_from_pol_model(
                &self.asset_mgr.component_factory(),
                &self.asset_mgr.vfs(),
                &path,
                "preview".to_string(),
                true,
            )),
            Some("dff") => Some(create_entity_from_dff_model(
                &self.asset_mgr.component_factory(),
                &self.asset_mgr.vfs(),
                &path,
                "preview".to_string(),
                true,
            )),
            Some("cvd") => Some(create_entity_from_cvd_model(
                self.asset_mgr.component_factory().clone(),
                &self.asset_mgr.vfs(),
                &path,
                "preview".to_string(),
                true,
            )),
            _ => None,
        };

        let scene = scene_manager.scene_mut().unwrap();
        if let Some(e) = entity {
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

            let mut scene = CoreScene::new(
                self.asset_mgr
                    .load_scn(cpk_name.as_str(), scn_name.as_str()),
            );
            scene
                .camera_mut()
                .transform_mut()
                .set_position(&Vec3::new(0., 500., 500.))
                .look_at(&Vec3::new(0., 0., 0.));
            scene_manager.push_scene(Box::new(scene));
        }

        None
    }
}
