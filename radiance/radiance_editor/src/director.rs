use crosscom::ComRc;
use imgui::Condition;
use radiance::{
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    radiance::UiManager,
};
use std::{cell::RefCell, rc::Rc};

use crate::{
    ui::scene_view::{SceneView, SceneViewPlugins},
    ComObject_MainPageDirector,
};

pub struct MainPageDirector {
    scene_view: RefCell<SceneView>,
    ui: Rc<UiManager>,
    scene_manager: ComRc<ISceneManager>,
}

ComObject_MainPageDirector!(super::MainPageDirector);

impl MainPageDirector {
    pub fn create(
        scene_view_plugins: Option<SceneViewPlugins>,
        ui: Rc<UiManager>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_manager: ComRc<ISceneManager>,
    ) -> ComRc<IDirector> {
        ComRc::from_object(Self {
            scene_view: RefCell::new(SceneView::new(input, scene_view_plugins)),
            ui,
            scene_manager,
        })
    }
}

impl IDirectorImpl for MainPageDirector {
    fn activate(&self) {}

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        let ui = self.ui.ui();
        let [window_width, window_height] = ui.io().display_size;
        let style = ui.push_style_var(imgui::StyleVar::WindowPadding([0., 0.]));

        ui.window("TOP_LEVEL")
            .collapsible(false)
            .resizable(false)
            .size([window_width, window_height], Condition::Always)
            .position([0., 0.], Condition::Always)
            .movable(false)
            .draw_background(false)
            .title_bar(false)
            .bring_to_front_on_focus(false)
            .nav_focus(false)
            .build(|| {
                unsafe {
                    let s = "main_page_dock";
                    let s1 = s.as_ptr() as *const std::os::raw::c_char;
                    let id = {
                        let s2 = s1.add(s.len());
                        imgui::sys::igGetID_StrStr(s1, s2)
                    };

                    imgui::sys::igDockSpace(
                        id,
                        imgui::sys::ImVec2::new(0., 0.),
                        imgui::sys::ImGuiDockNodeFlags::from_le(
                            imgui::sys::ImGuiDockNodeFlags_PassthruCentralNode as i32,
                        ),
                        ::std::ptr::null::<imgui::sys::ImGuiWindowClass>(),
                    );
                };
                self.scene_view
                    .borrow_mut()
                    .render(self.scene_manager.clone(), ui, delta_sec)
            });

        style.pop();
        None
    }
}
