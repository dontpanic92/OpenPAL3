use eframe::egui::{CollapsingHeader, Ui};

use super::Component;

#[derive(Clone, Copy)]
pub enum TreeNodeType {
    Node,
    LeafNode,
}

#[derive(Clone, Debug)]
pub enum TreeViewEvents {}

#[derive(Clone)]
pub struct Tree {
    name: String,
    children: Vec<Tree>,
    ty: TreeNodeType,
}

impl Tree {
    pub fn new(name: String, ty: TreeNodeType) -> Self {
        Self {
            name,
            children: vec![],
            ty,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.ui_impl(ui, 0, self);
    }

    fn ui_impl(&self, ui: &mut Ui, depth: usize, node: &Tree) {
        CollapsingHeader::new(&node.name)
            .default_open(true)
            .show(ui, |ui| self.children_ui(ui, depth, node));
    }

    fn children_ui(&self, ui: &mut Ui, depth: usize, node: &Tree) {
        for child in &node.children {
            match child.ty {
                TreeNodeType::Node => self.ui_impl(ui, depth + 1, child),
                TreeNodeType::LeafNode => self.leaf_ui(ui, child),
            };
        }
    }

    fn leaf_ui(&self, ui: &mut Ui, node: &Tree) {
        ui.label(&self.name);
    }
}

pub struct TreeView {
    pub id: String,
    pub tree: Tree,
}

impl TreeView {
    pub fn new() -> Self {
        Self {
            id: "".to_string(),
            tree: Tree::new("root".to_string(), TreeNodeType::Node),
        }
    }

    pub fn from_xml(node: roxmltree::Node) -> Self {
        let id = node.attribute("id").unwrap_or("").to_string();

        Self {
            id,
            tree: Tree::new("root".to_string(), TreeNodeType::Node),
        }
    }

    pub fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.tree.ui(ui);
    }
}

impl Component for TreeView {
    fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.update(ctx, ui, frame);
    }

    fn find_component(&mut self, id: &str) -> Option<&mut dyn Component> {
        if self.id == id {
            Some(self)
        } else {
            None
        }
    }

    fn as_type(&mut self) -> super::ComponentRef {
        super::ComponentRef::TreeView(self)
    }
}
