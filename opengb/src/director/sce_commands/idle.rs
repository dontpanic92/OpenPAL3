use crate::director::sce_director::SceCommand;
use crate::resource_manager::ResourceManager;
use crate::scene::Mv3ModelEntity;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandIdle {
    idle_sec: f32,
    cur_sec: f32,
}

impl SceCommand for SceCommandIdle {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        state: &mut HashMap<String, Box<dyn Any>>,
        delta_sec: f32,
    ) -> bool {
        self.cur_sec += delta_sec;
        self.cur_sec > self.idle_sec
    }
}

impl SceCommandIdle {
    pub fn new(idle_sec: f32) -> Self {
        Self {
            idle_sec,
            cur_sec: 0.,
        }
    }
}
