use crate::director::sce_director::SceCommand;
use crate::director::sce_state::SceState;
use crate::scene::ScnScene;
use imgui::{im_str, Condition, Ui, Window};
use radiance::{input::Key, scene::CoreScene};

#[derive(Clone)]
pub struct SceCommandDlg {
    text: String,
}

impl SceCommandDlg {
    const DLG_HEIGHT_FACTOR: f32 = 0.2;
    const DLG_Y_POSITION_FACTOR: f32 = 1. - SceCommandDlg::DLG_HEIGHT_FACTOR;
}

impl SceCommand for SceCommandDlg {
    fn update(
        &mut self,
        scene: &mut CoreScene<ScnScene>,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let [window_width, window_height] = ui.io().display_size;
        let w = Window::new(im_str!(" "))
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .size(
                [
                    window_width,
                    window_height * SceCommandDlg::DLG_HEIGHT_FACTOR,
                ],
                Condition::Appearing,
            )
            .position(
                [0., window_height * SceCommandDlg::DLG_Y_POSITION_FACTOR],
                Condition::Appearing,
            );
        w.build(ui, || {
            ui.text_wrapped(&im_str!("{}", self.text));
        });

        state.input().get_key_state(Key::Space).pressed()
    }
}

impl SceCommandDlg {
    pub fn new(text: String) -> Self {
        Self {
            text: text.replace("\\n", "\n"),
        }
    }
}
