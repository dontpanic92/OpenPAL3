use super::{main_content::ContentTabs, DevToolsState, PreviewDirector};
use imgui::{Condition, TreeNode, Ui, Window};
use mini_fs::{Entries, Entry, EntryKind, StoreExt};
use opengb::asset_manager::AssetManager;
use radiance::{
    audio::AudioEngine,
    input::InputEngine,
    scene::{Director, SceneManager},
};
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

    fn main_window(&mut self, ui: &mut Ui) -> Option<DevToolsState> {
        let [window_width, window_height] = ui.io().display_size;
        let font = ui.push_font(ui.fonts().fonts()[1]);

        let w = Window::new("Files")
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.3, window_height], Condition::Always)
            .position([0., 0.], Condition::Always)
            .movable(false);
        w.build(ui, || self.render_tree_nodes(ui, "/"));

        let mut state = None;
        let w2 = Window::new("Content")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.7, window_height], Condition::Always)
            .position([window_width * 0.3, 0.], Condition::Always)
            .movable(false);
        w2.build(ui, || state = self.render_content(ui));

        font.pop();

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
            let treenode = TreeNode::new(e_filename);

            if e.kind == EntryKind::Dir {
                treenode.build(ui, || {
                    self.render_tree_nodes(ui, &e_fullname);
                });
            } else {
                treenode.leaf(true).build(ui, || {
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
}

impl Director for DevToolsDirector {
    fn activate(&mut self, _scene_manager: &mut dyn SceneManager) {}

    fn update(
        &mut self,
        _scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        _delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        if let Some(DevToolsState::Preview(path)) = self.main_window(ui) {
            Some(PreviewDirector::new(
                self.shared_self.upgrade().unwrap(),
                self.asset_mgr.clone(),
                self.input_engine.clone(),
                path,
            ))
        } else {
            None
        }
    }
}
