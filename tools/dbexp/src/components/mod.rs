use eframe::egui::Ui;

pub mod label;
pub mod scroll_area;
pub mod table_view;
pub mod tree_view;
pub mod window;

pub trait Component {
    fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame);
    fn find_component(&mut self, id: &str) -> Option<&mut dyn Component>;
    fn as_type(&mut self) -> ComponentRef;
}

pub enum ComponentRef<'a> {
    TableView(&'a mut table_view::TableView),
    ScrollArea(&'a mut scroll_area::ScrollArea),
    TreeView(&'a mut tree_view::TreeView),
    Label(&'a mut label::Label),
}

impl<'a> ComponentRef<'a> {
    pub fn as_table_view(&mut self) -> Option<&mut table_view::TableView> {
        match self {
            ComponentRef::TableView(tv) => Some(tv),
            _ => None,
        }
    }

    pub fn as_scroll_area(&mut self) -> Option<&mut scroll_area::ScrollArea> {
        match self {
            ComponentRef::ScrollArea(sa) => Some(sa),
            _ => None,
        }
    }

    pub fn as_tree_view(&mut self) -> Option<&mut tree_view::TreeView> {
        match self {
            ComponentRef::TreeView(tv) => Some(tv),
            _ => None,
        }
    }

    pub fn as_label(&mut self) -> Option<&mut label::Label> {
        match self {
            ComponentRef::Label(l) => Some(l),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Events {
    TreeView(tree_view::TreeViewEvents),
    TableView(table_view::TableViewEvents),
}

pub fn create_component_from_xml(node: roxmltree::Node) -> Option<Box<dyn Component>> {
    match node.tag_name().name() {
        "" => None,
        "TableView" => Some(Box::new(table_view::TableView::from_xml(node))),
        "ScrollArea" => Some(Box::new(scroll_area::ScrollArea::from_xml(node))),
        "TreeView" => Some(Box::new(tree_view::TreeView::from_xml(node))),
        "Label" => Some(Box::new(label::Label::from_xml(node))),
        _ => panic!("Unknown component type: {}", node.tag_name().name()),
    }
}
