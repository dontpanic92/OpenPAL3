use std::collections::HashMap;

use crate::scripting::sce::{SceCommand, SceState};
use crosscom::ComRc;
use imgui::{Condition, Ui};
use radiance::{comdef::ISceneManager, input::Key};

lazy_static::lazy_static! {
    pub static ref KEY_NUM_MAP: HashMap<Key, i32> = create_key_num_hashmap();
}

#[derive(Debug, Clone)]
pub struct SceCommandDlgSel {
    list: Vec<String>,
}

impl SceCommand for SceCommandDlgSel {
    fn update(
        &mut self,
        _scene_manager: ComRc<ISceneManager>,
        ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let [window_width, window_height] = ui.io().display_size;
        let (_dialog_x, _dialog_width) = {
            if window_width / window_height > 4. / 3. {
                let dialog_width = window_height / 3. * 4.;
                let dialog_x = (window_width - dialog_width) / 2.;
                (dialog_x, dialog_width)
            } else {
                (0., window_width)
            }
        };

        ui.window("DlgSel")
            .collapsible(false)
            .title_bar(false)
            .resizable(false)
            .always_auto_resize(true)
            .position_pivot([0.5, 0.5])
            .position([window_width / 2., window_height / 2.], Condition::Always)
            .build(|| {
                self.list
                    .iter()
                    .for_each(|text| ui.text(&format!("{}", text)));
            });

        let dlg_sel = KEY_NUM_MAP
            .iter()
            .map(|(&key, &value)| {
                if state.input().get_key_state(key).pressed() {
                    value
                } else {
                    -1
                }
            })
            .find(|&value| value != -1);

        if let Some(sel) = dlg_sel {
            if (sel as usize) < self.list.len() {
                state
                    .context_mut()
                    .current_proc_context_mut()
                    .set_dlgsel(sel);
                return true;
            }
        }

        return false;
    }

    fn initialize(&mut self, _scene_manager: ComRc<ISceneManager>, _state: &mut SceState) {}
}

impl SceCommandDlgSel {
    pub fn new(mut list: Vec<String>) -> Self {
        list.reverse();
        Self { list }
    }
}

fn create_key_num_hashmap() -> HashMap<Key, i32> {
    let mut map = HashMap::new();
    map.insert(Key::Num1, 0);
    map.insert(Key::Num2, 1);
    map.insert(Key::Num3, 2);
    map.insert(Key::Num4, 3);
    map.insert(Key::Num5, 4);
    map.insert(Key::Num6, 5);
    map.insert(Key::Num7, 6);
    map.insert(Key::Num8, 7);
    map.insert(Key::Num9, 8);
    map.insert(Key::Num0, 9);
    map
}
