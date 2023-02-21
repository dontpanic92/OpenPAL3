use crosscom::ComRc;
use imgui::{Condition, Ui};
use radiance::{
    comdef::{IDirector, IDirectorImpl},
    input::InputEngine,
    scene::SceneManager,
};
use std::{cell::RefCell, rc::Rc};

use crate::{
    ui::scene_view::{SceneView, SceneViewPlugins},
    ComObject_MainPageDirector,
};

pub struct MainPageDirector {
    scene_view: RefCell<SceneView>,
}

ComObject_MainPageDirector!(super::MainPageDirector);

impl MainPageDirector {
    pub fn new(
        scene_view_plugins: Option<SceneViewPlugins>,
        input: Rc<RefCell<dyn InputEngine>>,
    ) -> ComRc<IDirector> {
        ComRc::from_object(Self {
            scene_view: RefCell::new(SceneView::new(input, scene_view_plugins)),
        })
    }
}

impl IDirectorImpl for MainPageDirector {
    fn activate(&self, _scene_manager: &mut dyn SceneManager) {}

    fn update(
        &self,
        scene_manager: &mut dyn SceneManager,
        ui: &Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
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
            .build(|| {
                self.scene_view
                    .borrow_mut()
                    .render(scene_manager, ui, delta_sec)
            });

        font.pop();

        None
    }
}
