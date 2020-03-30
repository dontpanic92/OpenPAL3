use crate::director::sce_director::{SceCommand, WellKnownVariables};
use crate::resource_manager::ResourceManager;
use crate::scene::Mv3ModelEntity;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandRunScriptMode {
    mode: i32,
}

impl SceCommand for SceCommandRunScriptMode {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        state: &mut HashMap<String, Box<dyn Any>>,
        delta_sec: f32,
    ) -> bool {
        let mode = state
            .get_mut(WellKnownVariables::RUN_MODE)
            .unwrap()
            .as_mut()
            .downcast_mut::<i32>()
            .unwrap();
        *mode = self.mode;
        return true;
    }
}

impl SceCommandRunScriptMode {
    pub fn new(mode: i32) -> Self {
        Self { mode }
    }
}
