use crate::directors::sce_vm::{SceCommand, SceState};
use imgui::{im_str, Condition, Ui, Window};
use radiance::{input::Key, scene::SceneManager};

#[derive(Clone)]
pub struct SceCommandDlg {
    text: String,
}

impl SceCommandDlg {
    const DLG_HEIGHT_FACTOR: f32 = 0.25;
    const DLG_Y_POSITION_FACTOR: f32 = 1. - SceCommandDlg::DLG_HEIGHT_FACTOR;
}

impl SceCommand for SceCommandDlg {
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        let [window_width, window_height] = ui.io().display_size;
        let (dialog_x, dialog_width) = {
            if window_width / window_height > 4. / 3. {
                let dialog_width = window_height / 3. * 4.;
                let dialog_x = (window_width - dialog_width) / 2.;
                (dialog_x, dialog_width)
            } else {
                (0., window_width)
            }
        };

        let w = Window::new(im_str!(" "))
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .size(
                [
                    dialog_width,
                    window_height * SceCommandDlg::DLG_HEIGHT_FACTOR,
                ],
                Condition::Appearing,
            )
            .position(
                [
                    dialog_x,
                    window_height * SceCommandDlg::DLG_Y_POSITION_FACTOR,
                ],
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
