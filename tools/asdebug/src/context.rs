pub enum ServerConnectionState {
    NotStarted,
    Listening,
    Connected,
    Error(String),
}

pub struct Context {
    ec: eframe::egui::Context,
    pub connection_state: ServerConnectionState,
}

impl Context {
    pub fn new(ec: eframe::egui::Context) -> Self {
        Self {
            ec,
            connection_state: ServerConnectionState::NotStarted,
        }
    }

    pub fn request_repaint(&self) {
        self.ec.request_repaint();
    }
}
