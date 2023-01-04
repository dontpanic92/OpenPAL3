pub struct ScriptGlobalContext {
    globals: Vec<u32>,
}

impl ScriptGlobalContext {
    pub fn new() -> Self {
        Self {
            globals: vec![0; 32],
        }
    }

    pub fn get_global(&self, index: usize) -> u32 {
        self.globals[index]
    }

    pub fn set_global(&mut self, index: usize, data: u32) {
        self.globals[index] = data;
    }
}
