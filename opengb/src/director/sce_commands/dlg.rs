use crate::director::sce_director::SceCommand;
use crate::resource_manager::ResourceManager;
use crate::scene::Mv3ModelEntity;
use imgui::*;
use radiance::math::Vec3;
use radiance::scene::{CoreEntity, Entity, Scene};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandDlg {
    dlg_sec: f32,
    cur_sec: f32,
}

impl SceCommand for SceCommandDlg {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut HashMap<String, Box<dyn Any>>,
        delta_sec: f32,
    ) -> bool {
        self.cur_sec += delta_sec;
        let completed = self.cur_sec > self.dlg_sec;
        if !completed {
            let w = Window::new(im_str!("Example 1: Basics"))
                .size([700.0, 300.0], Condition::Appearing)
                .position([150.0, 450.0], Condition::Appearing);
            w.build(ui, || {
                ui.text(im_str!("景天：\n 什么声音？ …… 有贼？！"));
            });
        }

        completed
    }
}

impl SceCommandDlg {
    pub fn new() -> Self {
        Self {
            dlg_sec: 5.,
            cur_sec: 0.,
        }
    }
}
