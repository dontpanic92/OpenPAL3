use std::{cell::RefCell, rc::Rc};

use imgui::{im_str, InputTextMultiline, TabBar, TabItem, Ui, Window};
use opengb::{
    directors::{AdventureDirector, SceneManagerExtensions},
    scene,
};
use radiance::{
    application::utils::FpsCounter,
    audio::AudioEngine,
    input::{InputEngine, Key},
    radiance::DebugLayer,
    scene::{Entity, SceneManager},
};

pub struct OpenPal3DebugLayer {
    input_engine: Rc<RefCell<dyn InputEngine>>,
    audio_engine: Rc<dyn AudioEngine>,

    visible: bool,
    fps_counter: FpsCounter,
}

impl OpenPal3DebugLayer {
    pub fn new(
        input_engine: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
    ) -> OpenPal3DebugLayer {
        OpenPal3DebugLayer {
            input_engine,
            audio_engine,
            visible: false,
            fps_counter: FpsCounter::new(),
        }
    }
}

impl DebugLayer for OpenPal3DebugLayer {
    fn update(&mut self, scene_manager: &mut dyn SceneManager, ui: &mut Ui, delta_sec: f32) {
        let fps = self.fps_counter.update_fps(delta_sec);

        let input = self.input_engine.borrow();
        if input.get_key_state(Key::Tilde).pressed() {
            println!("backtick down");
            self.visible = !self.visible;
        }

        if !self.visible {
            return;
        }

        let w = Window::new(im_str!("Debug"));
        w.build(ui, || {
            ui.text(im_str!("Fps: {}", fps));
            TabBar::new(im_str!("##debug_tab_bar")).build(ui, || {
                TabItem::new(im_str!("Nav")).build(ui, || {
                    let current_nav_coord = (|| {
                        let director = scene_manager.director();
                        if let Some(d) = director {
                            if let Some(adv) = d.borrow().downcast_ref::<AdventureDirector>() {
                                let coord = scene_manager
                                    .get_resolved_role(adv.sce_vm().state(), -1)
                                    .unwrap()
                                    .transform()
                                    .position();
                                return Some(
                                    scene_manager
                                        .core_scene_mut()?
                                        .scene_coord_to_nav_coord(&coord),
                                );
                            }
                        }

                        None
                    })();

                    let scene = scene_manager.core_scene_mut();
                    let text = if let Some(scene) = scene {
                        let size = scene.nav().get_map_size();
                        let mut text = "".to_string();
                        for j in 0..size.1 {
                            for i in 0..size.0 {
                                let ch = (|| {
                                    if let Some(nav) = current_nav_coord {
                                        if nav.0 as usize == i && nav.1 as usize == j {
                                            return "x".to_string();
                                        }
                                    }

                                    return scene
                                        .nav()
                                        .get(0, i as i32, j as i32)
                                        .unwrap()
                                        .distance_to_border
                                        .to_string();
                                })();
                                text += ch.as_str();
                            }

                            text += "\n";
                        }

                        text
                    } else {
                        "No CoreScene loaded".to_string()
                    };

                    InputTextMultiline::new(
                        ui,
                        &im_str!("##debug_nav_text"),
                        &mut im_str!("{}", text),
                        [-1., -1.],
                    )
                    .read_only(true)
                    .build();
                });
            });
        });
    }
}
