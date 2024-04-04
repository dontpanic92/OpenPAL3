use std::rc::Rc;

use eframe::egui::{self, Ui};
use egui_extras::{Column, TableBuilder};

use super::{window::EventEmitter, Component, ComponentRef};

#[derive(Clone, Debug)]
pub enum TableViewEvents {
    Selected(usize),
}

pub struct TableView {
    pub id: String,
    pub headers: Vec<String>,
    pub items: Vec<Vec<String>>,
    pub selected: Option<usize>,
    event_emitter: Option<Rc<EventEmitter>>,
}

impl TableView {
    pub fn new() -> TableView {
        TableView {
            id: "".to_string(),
            headers: Vec::new(),
            items: Vec::new(),
            selected: None,
            event_emitter: None,
        }
    }

    pub fn from_xml(node: roxmltree::Node) -> TableView {
        let id = node.attribute("id").unwrap_or("").to_string();

        TableView {
            id,
            headers: Vec::new(),
            items: Vec::new(),
            selected: None,
            event_emitter: None,
        }
    }

    pub fn set_event_emitter(&mut self, event_emitter: Rc<EventEmitter>) {
        self.event_emitter = Some(event_emitter);
    }

    pub fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame) {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            let text_height = egui::TextStyle::Body
                .resolve(ui.style())
                .size
                .max(ui.spacing().interact_size.y);

            let mut table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .min_scrolled_height(0.0)
                .sense(egui::Sense::click());

            for _ in &self.headers {
                table = table.column(Column::auto());
            }

            table
                .header(20.0, |mut header| {
                    for h in &self.headers {
                        header.col(|ui| {
                            ui.strong(h);
                        });
                    }
                })
                .body(|body| {
                    body.rows(text_height, self.items.len(), |mut row| {
                        let row_index = row.index();

                        row.set_selected(self.selected == Some(row_index));

                        for item in self.items[row_index].iter() {
                            row.col(|ui| {
                                ui.label(item);
                            });
                        }

                        if row.response().clicked() {
                            self.selected = Some(row_index);
                            if let Some(event_emitter) = &self.event_emitter {
                                event_emitter.emit(super::Events::TableView(
                                    TableViewEvents::Selected(row_index),
                                ));
                            }
                        }
                    });
                });
        });
    }
}

impl Component for TableView {
    fn update(&mut self, ctx: &eframe::egui::Context, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.update(ctx, ui, frame);
    }

    fn find_component(&mut self, id: &str) -> Option<&mut dyn Component> {
        if self.id == id {
            return Some(self);
        }

        None
    }

    fn as_type(&mut self) -> ComponentRef {
        ComponentRef::TableView(self)
    }
}
