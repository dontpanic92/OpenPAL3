
pub trait Scene {
    fn load(&mut self);
    fn update(&mut self);
    fn unload(&mut self);
}

pub fn create() -> Box<dyn Scene> {
    Box::new(RuntimeScene {})
}

struct RuntimeScene {
}

impl Scene for RuntimeScene {
    fn load(&mut self) {

    }

    fn update(&mut self) {

    }

    fn unload(&mut self) {
        
    }
}
