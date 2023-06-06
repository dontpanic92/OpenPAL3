use shared::scripting::angelscript::ScriptModule;

pub enum ServerConnectionState {
    NotStarted,
    Listening,
    Connected,
    Error(String),
}

pub enum DebuggeeState {
    Running,
    WaitForAction,
}

pub struct Context {
    ec: eframe::egui::Context,
    pub state: DebuggeeState,
    pub connection_state: ServerConnectionState,
    pub functions: Vec<String>,
    pub module: Option<ScriptModule>,
    pub function_id: u32,
    pub stack: Vec<u8>,
    pub objects: Vec<Option<String>>,
    pub pc: usize,
    pub sp: usize,
    pub fp: usize,
    pub r1: u32,
    pub r2: u32,
    pub object_register: usize,
}

impl Context {
    pub fn new(ec: eframe::egui::Context) -> Self {
        Self {
            ec,
            state: DebuggeeState::Running,
            connection_state: ServerConnectionState::NotStarted,
            functions: vec![],
            module: None,
            function_id: 0,
            stack: vec![],
            objects: vec![],
            pc: 0,
            sp: 0,
            fp: 0,
            r1: 0,
            r2: 0,
            object_register: 0,
        }
    }

    pub fn request_repaint(&self) {
        self.ec.request_repaint();
    }
}
