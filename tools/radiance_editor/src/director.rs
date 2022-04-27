use imgui::{Condition, Ui};
use radiance::{
    input::InputEngine,
    scene::{Director, SceneManager},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::ui::scene_view::{SceneView, SceneViewPlugins};

pub struct UiDirector {
    shared_self: Weak<RefCell<Self>>,
    scene_view: SceneView,
}

impl UiDirector {
    pub fn new(
        scene_view_plugins: Option<SceneViewPlugins>,
        input: Rc<RefCell<dyn InputEngine>>,
    ) -> Rc<RefCell<Self>> {
        let mut _self = Rc::new(RefCell::new(Self {
            shared_self: Weak::new(),
            scene_view: SceneView::new(input, scene_view_plugins),
        }));

        _self.borrow_mut().shared_self = Rc::downgrade(&_self);
        _self
    }
}

impl Director for UiDirector {
    fn activate(&mut self, _scene_manager: &mut dyn SceneManager) {}

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        let [window_width, window_height] = ui.io().display_size;
        let font = ui.push_font(ui.fonts().fonts()[1]);

        ui.window("TOP_LEVEL")
            .collapsible(false)
            .resizable(false)
            .size([window_width, window_height], Condition::Always)
            .position([0., 0.], Condition::Always)
            .movable(false)
            .draw_background(false)
            .title_bar(false)
            .bring_to_front_on_focus(false)
            .build(|| self.scene_view.render(scene_manager, ui, delta_sec));

        font.pop();

        None
    }
}
