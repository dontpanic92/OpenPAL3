use crate::scene::ScnScene;

pub trait Director {
    fn initialize(&mut self);
    fn update(&mut self, scn: &ScnScene);
}
