use std::collections::HashMap;

pub struct PersistentState {
    global_vars: HashMap<i16, i32>,
}

impl PersistentState {
    pub fn new() -> Self {
        Self {
            global_vars: HashMap::new(),
        }
    }

    pub fn set_global(&mut self, var: i16, value: i32) {
        self.global_vars.insert(var, value);
    }

    pub fn get_global(&mut self, var: i16) -> Option<i32> {
        self.global_vars.get(&var).and_then(|v| Some(*v))
    }
}
