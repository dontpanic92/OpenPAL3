use crate::directors::sce_vm::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::{MouseButton, Ui};
use radiance::{comdef::ISceneManager, input::Key};

#[derive(Debug, Clone)]
pub struct SceCommandDlg {
    text: String,
    dlg_end: bool,
    adv_input_enabled: bool,
}

impl SceCommand for SceCommandDlg {
    fn initialize(&mut self, scene_manager: ComRc<ISceneManager>, state: &mut SceState) {
        self.adv_input_enabled = state.global_state_mut().adv_input_enabled();
        state.global_state_mut().set_adv_input_enabled(false);
    }

    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        delta_sec: f32,
    ) -> bool {
        if self.dlg_end {
            // state.global_state_mut().set_adv_input_enabled(self.adv_input_enabled);
            return true;
        }

        state.dialog_box().draw(self.text.as_ref(), ui, delta_sec);

        // delay set_adv_input to the next frame so that the last kay pressed
        // won't trigger the sce proc again.
        self.dlg_end = state.input().get_key_state(Key::Space).pressed()
            || state.input().get_key_state(Key::GamePadEast).pressed()
            || state.input().get_key_state(Key::GamePadSouth).pressed()
            || ui.is_mouse_released(MouseButton::Left);

        false
    }
}

impl SceCommandDlg {
    pub fn new(text: String) -> Self {
        Self {
            text: text.replace("\\n", "\n"),
            dlg_end: false,
            adv_input_enabled: false,
        }
    }
}
