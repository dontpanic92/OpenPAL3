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

    fn render(&mut self, scene_manager: &mut dyn SceneManager, ui: &mut Ui, delta_sec: f32) {
        let w = Window::new(im_str!("Debug"));
        w.build(ui, || {
            let fps = self.fps_counter.update_fps(delta_sec);
            ui.text(im_str!("Fps: {}", fps));
            let scene = scene_manager.core_scene();
            if let Some(s) = scene {
                ui.text(im_str!("Scene: {} {}", s.name(), s.sub_name()));
            }

            let coord = scene_manager.director().as_ref().and_then(|d| {
                d.borrow()
                    .downcast_ref::<AdventureDirector>()
                    .and_then(|adv| {
                        Some(
                            scene_manager
                                .get_resolved_role(adv.sce_vm().state(), -1)
                                .unwrap()
                                .transform()
                                .position(),
                        )
                    })
            });

            ui.text(im_str!("Coord: {:?}", &coord));
            TabBar::new(im_str!("##debug_tab_bar")).build(ui, || {
                TabItem::new(im_str!("Nav")).build(ui, || {
                    TabBar::new(im_str!("##debug_tab_bar_nav_bar")).build(ui, || {
                        if scene_manager.core_scene().is_none() {
                            return;
                        }
                        let layer_count = scene_manager.core_scene().unwrap().nav().layer_count();
                        for layer in 0..layer_count {
                            TabItem::new(&im_str!("Layer {}", layer)).build(ui, || {
                                let current_nav_coord = coord.as_ref().and_then(|c| {
                                    Some(
                                        scene_manager
                                            .core_scene_mut()?
                                            .scene_coord_to_nav_coord(layer, c),
                                    )
                                });

                                ui.text(im_str!("Nav Coord: {:?}", &current_nav_coord));
                                let text = {
                                    let s = scene_manager.core_scene().unwrap();
                                    let size = s.nav().get_map_size(layer);
                                    let mut text = "".to_string();
                                    for j in 0..size.1 {
                                        for i in 0..size.0 {
                                            let ch = (|| {
                                                if let Some(nav) = current_nav_coord {
                                                    if nav.0 as usize == i && nav.1 as usize == j {
                                                        return "x".to_string();
                                                    }
                                                }

                                                let distance = s
                                                    .nav()
                                                    .get(layer, i as i32, j as i32)
                                                    .unwrap()
                                                    .distance_to_border;
                                                
                                                return if distance > 0 { "=".to_string() } else { "_".to_string() }
                                            })(
                                            );
                                            text += ch.as_str();
                                        }

                                        text += "\n";
                                    }

                                    text
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
                        }
                    });
                });
            });
        });
    }
}

impl DebugLayer for OpenPal3DebugLayer {
    fn update(&mut self, scene_manager: &mut dyn SceneManager, ui: &mut Ui, delta_sec: f32) {
        let font = ui.push_font(ui.fonts().fonts()[1]);
        (|| {
            if self
                .input_engine
                .borrow()
                .get_key_state(Key::Tilde)
                .pressed()
            {
                self.visible = !self.visible;
            }

            if !self.visible {
                return;
            }

            self.render(scene_manager, ui, delta_sec);
        })();
        font.pop(ui);
    }
}
