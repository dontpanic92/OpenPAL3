use eframe::egui::{self, Ui};

use super::{create_component_from_xml, Component, ComponentRef};

pub enum Orientation {
    Horizontal,
    Vertical,
}

pub struct ScrollArea {
    pub id: String,
    pub orientation: Orientation,
    pub children: Vec<Box<dyn Component>>,
}

impl ScrollArea {
    pub fn new(orientation: Orientation) -> ScrollArea {
        ScrollArea {
            id: "".to_string(),
            orientation,
            children: Vec::new(),
        }
    }

    pub fn from_xml(node: roxmltree::Node) -> ScrollArea {
        let id = node.attribute("id").unwrap_or("").to_string();

        let orientation = match node.attribute("orientation") {
            Some("horizontal") => Orientation::Horizontal,
            Some("vertical") => Orientation::Vertical,
            _ => Orientation::Vertical,
        };

        let mut children = Vec::new();
        for child in node.children() {
            let component = create_component_from_xml(child);
            if let Some(component) = component {
                children.push(component);
            }
        }

        ScrollArea {
            id,
            orientation,
            children,
        }
    }

    pub fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame) {
        let scroll_area = match self.orientation {
            Orientation::Horizontal => egui::ScrollArea::horizontal(),
            Orientation::Vertical => egui::ScrollArea::vertical(),
        };

        scroll_area.show(ui, |ui| {
            for child in &mut self.children {
                child.update(ctx, ui, frame);
            }
        });
    }
}

impl Component for ScrollArea {
    fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.update(ctx, ui, frame);
    }

    fn find_component(&mut self, id: &str) -> Option<&mut dyn Component> {
        if self.id == id {
            return Some(self);
        }

        for child in &mut self.children {
            if let Some(component) = child.find_component(id) {
                return Some(component);
            }
        }

        None
    }

    fn as_type(&mut self) -> ComponentRef {
        ComponentRef::ScrollArea(self)
    }
}
