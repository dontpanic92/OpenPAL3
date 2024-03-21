use std::{
    cell::{Cell, RefCell, RefMut},
    rc::Rc,
};

use eframe::egui;

use super::{Component, Events};

pub struct Window {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub left_panel: Option<Panel>,
    pub right_panel: Option<Panel>,
    pub central_panel: Option<Panel>,
    pub event_emitter: Rc<EventEmitter>,
    pub event_handler: Cell<Option<Box<dyn Fn(&mut Self, Events)>>>,
}

impl Window {
    pub fn new(title: &str, width: u32, height: u32) -> Window {
        Window {
            title: title.to_string(),
            width,
            height,
            left_panel: None,
            right_panel: None,
            central_panel: None,
            event_emitter: Rc::new(EventEmitter::new()),
            event_handler: Cell::new(None),
        }
    }

    pub fn from_xml(xml: &str) -> Window {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let root = doc.root_element();
        let title = root.attribute("title").unwrap().to_string();
        let width = root.attribute("width").unwrap().parse().unwrap();
        let height = root.attribute("height").unwrap().parse().unwrap();

        let left_panel = Window::parse_panel("LeftPanel", PanelPosition::Left, root);

        let right_panel = Window::parse_panel("RightPanel", PanelPosition::Right, root);

        let central_panel = Window::parse_panel("CentralPanel", PanelPosition::Central, root);

        Window {
            title,
            width,
            height,
            left_panel,
            right_panel,
            central_panel,
            event_emitter: Rc::new(EventEmitter::new()),
            event_handler: Cell::new(None),
        }
    }

    pub fn set_event_handler(&mut self, handler: impl Fn(&mut Self, Events) + 'static) {
        self.event_handler.set(Some(Box::new(handler)));
    }

    pub fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.left_panel
            .as_mut()
            .map(|panel| panel.update(ctx, frame));

        self.right_panel
            .as_mut()
            .map(|panel| panel.update(ctx, frame));

        self.central_panel
            .as_mut()
            .map(|panel| panel.update(ctx, frame));

        if let Some(handler) = self.event_handler.take() {
            for event in self.event_emitter.clone().events().drain(..) {
                handler(self, event);
            }

            self.event_handler.set(Some(handler));
        }
    }

    pub fn find_component(&mut self, id: &str) -> Option<&mut dyn Component> {
        self.left_panel
            .as_mut()
            .and_then(|panel| panel.find_component(id))
            .or_else(|| {
                self.right_panel
                    .as_mut()
                    .and_then(|panel| panel.find_component(id))
            })
            .or_else(|| {
                self.central_panel
                    .as_mut()
                    .and_then(|panel| panel.find_component(id))
            })
    }

    fn parse_panel(tag: &str, position: PanelPosition, node: roxmltree::Node) -> Option<Panel> {
        for child in node.children() {
            if child.tag_name().name() == tag {
                return Some(Panel::from_xml(position, child));
            }
        }

        None
    }
}

pub enum PanelPosition {
    Left,
    Right,
    Central,
}

pub struct Panel {
    pub position: PanelPosition,
    pub resizable: bool,
    pub width: u32,
    pub children: Vec<Box<dyn Component>>,
}

impl Panel {
    pub fn new(resizable: bool, width: u32) -> Panel {
        Panel {
            position: PanelPosition::Central,
            resizable,
            width,
            children: Vec::new(),
        }
    }

    pub fn from_xml(position: PanelPosition, node: roxmltree::Node) -> Panel {
        let resizable = node
            .attribute("resizable")
            .unwrap_or("false")
            .parse()
            .unwrap_or(false);
        let width = node
            .attribute("width")
            .unwrap_or("200")
            .parse()
            .unwrap_or(200);

        let mut children = Vec::new();
        for child in node.children() {
            let component = super::create_component_from_xml(child);
            if let Some(component) = component {
                children.push(component);
            }
        }

        Panel {
            position,
            resizable,
            width,
            children,
        }
    }

    pub fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        match self.position {
            PanelPosition::Left => egui::SidePanel::left("left_panel")
                .resizable(self.resizable)
                .default_width(self.width as f32)
                .show(ctx, |ui| {
                    self.show_children(ctx, ui, frame);
                }),
            PanelPosition::Right => egui::SidePanel::right("right_panel")
                .resizable(self.resizable)
                .default_width(self.width as f32)
                .show(ctx, |ui| {
                    self.show_children(ctx, ui, frame);
                }),
            PanelPosition::Central => egui::CentralPanel::default().show(ctx, |ui| {
                self.show_children(ctx, ui, frame);
            }),
        };
    }

    pub fn find_component(&mut self, id: &str) -> Option<&mut dyn Component> {
        for child in &mut self.children {
            if let Some(component) = child.find_component(id) {
                return Some(component);
            }
        }

        None
    }

    fn show_children(
        &mut self,
        ctx: &eframe::egui::Context,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
    ) {
        for child in &mut self.children {
            child.update(ctx, ui, frame);
        }
    }
}

pub struct EventEmitter {
    pub events: RefCell<Vec<Events>>,
}

impl EventEmitter {
    pub fn new() -> EventEmitter {
        EventEmitter {
            events: RefCell::new(Vec::new()),
        }
    }

    pub fn emit(&self, event: Events) {
        self.events.borrow_mut().push(event);
    }

    fn events(&self) -> RefMut<Vec<Events>> {
        self.events.borrow_mut()
    }
}
