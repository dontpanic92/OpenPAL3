use crosscom::ComRc;
use imgui::{Condition, Ui};
use radiance::comdef::{IEntity, ISceneManager};

pub struct NodeView {}

impl NodeView {
    pub fn render(&mut self, scene_manager: ComRc<ISceneManager>, ui: &Ui, _delta_sec: f32) {
        let [window_width, window_height] = ui.window_size();
        ui.window("对象树")
            .collapsible(false)
            .size(
                [window_width * 0.2, window_height * 0.7],
                Condition::FirstUseEver,
            )
            .position([0., 0.], Condition::FirstUseEver)
            .movable(true)
            .build(|| {
                let root_entities = scene_manager.scene().unwrap().root_entities();
                for e in root_entities {
                    self.build_node(ui, e.clone());
                }
            });
    }

    fn build_node(&mut self, ui: &Ui, entity: ComRc<IEntity>) {
        let treenode = ui.tree_node_config(entity.name());
        if entity.children().len() != 0 {
            treenode.build(|| {
                for e in entity.children() {
                    self.build_node(ui, e);
                }
            });
        } else {
            treenode.leaf(true).build(|| {});
        }
    }
}
