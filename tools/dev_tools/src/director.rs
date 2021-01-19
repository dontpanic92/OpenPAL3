use imgui::{im_str, Condition, ImString, TreeNode, Ui, Window};
use mini_fs::{Entries, Entry, EntryKind, StoreExt};
use opengb::asset_manager::AssetManager;
use radiance::{
    input::InputEngine,
    scene::{Director, SceneManager},
};
use std::{
    cell::RefCell,
    cmp::Ordering,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct DevToolsDirector {
    input_engine: Rc<RefCell<dyn InputEngine>>,
    asset_mgr: Rc<AssetManager>,
}

impl DevToolsDirector {
    pub fn new(input_engine: Rc<RefCell<dyn InputEngine>>, asset_mgr: Rc<AssetManager>) -> Self {
        Self {
            input_engine,
            asset_mgr,
        }
    }

    fn main_window(&mut self, ui: &mut Ui) {
        let [window_width, window_height] = ui.io().display_size;
        let font = ui.push_font(ui.fonts().fonts()[1]);

        self.file_list(ui, window_width, window_height);

        font.pop(ui);
    }

    fn file_list(&mut self, ui: &mut Ui, window_width: f32, window_height: f32) {
        let w = Window::new(im_str!("Files"))
            .collapsible(false)
            .resizable(false)
            .size([window_width * 0.3, window_height], Condition::Appearing)
            .position([0., 0.], Condition::Appearing);
        w.build(ui, || self.render_tree_nodes(ui, "/"));
    }

    fn render_tree_nodes<P: AsRef<Path>>(&mut self, ui: &Ui, path: P) {
        let entries = self.get_entries(path.as_ref());
        for e in entries {
            let e_path = PathBuf::from(&e.name);
            if e_path.file_name().is_none() {
                continue;
            }

            let e_filename = &im_str!("{}", e_path.file_name().unwrap().to_str().unwrap());
            let treenode = TreeNode::new(e_filename);

            if e.kind == EntryKind::Dir {
                treenode.build(ui, || {
                    self.render_tree_nodes(ui, path.as_ref().join(e_path.file_name().unwrap()));
                })
            } else {
                treenode.leaf(true).build(ui, || {});
            }
        }
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
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {}

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.main_window(ui);

        None
    }
}
