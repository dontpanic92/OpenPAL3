use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use imgui::*;
use radiance::scene::Scene;
use std::rc::Rc;

#[derive(Clone)]
pub struct SceCommandDlg {
    dlg_sec: f32,
    cur_sec: f32,
    text: String,
}

impl SceCommand for SceCommandDlg {
    fn update(
        &mut self,
        scene: &mut Box<dyn Scene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        self.cur_sec += delta_sec;
        let completed = self.cur_sec > self.dlg_sec;
        if !completed {
            let w = Window::new(im_str!(" "))
                .collapsible(false)
                .title_bar(false)
                .resizable(false)
                .size([700.0, 250.0], Condition::Appearing)
                .position([280.0, 600.0], Condition::Appearing);
            w.build(ui, || {
                ui.text(im_str!("{}", self.text));
            });
        }

        completed
    }
}

impl SceCommandDlg {
    pub fn new(text: &str) -> Self {
        Self {
            dlg_sec: 5.,
            cur_sec: 0.,
            text: text.to_owned(),
        }
    }
}
