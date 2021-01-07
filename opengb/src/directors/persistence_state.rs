use std::collections::HashMap;

pub struct PersistenceState {
    global_vars: HashMap<i16, i32>,
}

impl PersistenceState {
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
