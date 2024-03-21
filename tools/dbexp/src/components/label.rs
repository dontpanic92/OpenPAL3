pub struct Label {
    pub id: String,
    pub text: String,
}

impl Label {
    pub fn new() -> Label {
        Label {
            id: "".to_string(),
            text: "".to_string(),
        }
    }

    pub fn from_xml(node: roxmltree::Node) -> Label {
        let id = node.attribute("id").unwrap_or("").to_string();
        let text = node.text().unwrap_or("").to_string();

        Label { id, text }
    }

    pub fn update(
        &mut self,
        ctx: &eframe::egui::Context,
        ui: &mut eframe::egui::Ui,
        frame: &mut eframe::Frame,
    ) {
        ui.label(&self.text);
    }
}

impl super::Component for Label {
    fn update(
        &mut self,
        ctx: &eframe::egui::Context,
        ui: &mut eframe::egui::Ui,
        frame: &mut eframe::Frame,
    ) {
        self.update(ctx, ui, frame);
    }

    fn find_component(&mut self, id: &str) -> Option<&mut dyn super::Component> {
        if self.id == id {
            return Some(self);
        }
        None
    }

    fn as_type(&mut self) -> super::ComponentRef {
        super::ComponentRef::Label(self)
    }
}
