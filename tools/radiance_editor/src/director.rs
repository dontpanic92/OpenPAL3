use imgui::{Condition, Ui};
use radiance::{
    input::{InputEngine, Key},
    math::Vec3,
    scene::{Director, SceneManager},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::ui::scene_view::{SceneView, SceneViewPlugins};

pub struct UiDirector {
    shared_self: Weak<RefCell<Self>>,
    input: Rc<RefCell<dyn InputEngine>>,
    scene_view: SceneView,
}

impl UiDirector {
    pub fn new(
        scene_view_plugins: Option<SceneViewPlugins>,
        input: Rc<RefCell<dyn InputEngine>>,
    ) -> Rc<RefCell<Self>> {
        let mut _self = Rc::new(RefCell::new(Self {
            shared_self: Weak::new(),
            input,
            scene_view: SceneView::new(scene_view_plugins),
        }));

        _self.borrow_mut().shared_self = Rc::downgrade(&_self);
        _self
    }

    fn update_scene(&mut self, scene_manager: &mut dyn SceneManager, delta_sec: f32) {
        let input = self.input.borrow();
        let translate = if input.get_key_state(Key::S).is_down() {
            Some(Vec3::new(0., 0., 1000. * delta_sec))
        } else if input.get_key_state(Key::W).is_down() {
            Some(Vec3::new(0., 0., -1000. * delta_sec))
        } else {
            None
        };

        if let Some(t) = translate {
            scene_manager
                .scene_mut()
                .unwrap()
                .camera_mut()
                .transform_mut()
                .translate_local(&t);
        }

        scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform_mut()
            .rotate_axis_angle(
                &Vec3::new(0., 1., 0.),
                0.2 * delta_sec * std::f32::consts::PI,
            );
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

        self.update_scene(scene_manager, delta_sec);
        None
    }
}
