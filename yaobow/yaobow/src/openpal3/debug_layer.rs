use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use imgui::{InputTextMultiline, TabBar, TabItem, Ui};
use radiance::{
    application::utils::FpsCounter,
    comdef::{IEntityExt, ISceneManager},
    input::{InputEngine, Key},
    math::Vec3,
    radiance::{DebugLayer, UiManager},
};
use shared::openpal3::{
    comdef::IAdventureDirector, directors::SceneManagerExtensions, scene::RoleController,
};

pub struct OpenPal3DebugLayer {
    input_engine: Rc<RefCell<dyn InputEngine>>,
    scene_manager: ComRc<ISceneManager>,
    ui: Rc<UiManager>,

    visible: RefCell<bool>,
    fps_counter: RefCell<FpsCounter>,
}

impl OpenPal3DebugLayer {
    pub fn new(
        input_engine: Rc<RefCell<dyn InputEngine>>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
    ) -> OpenPal3DebugLayer {
        OpenPal3DebugLayer {
            input_engine,
            scene_manager,
            ui,
            visible: RefCell::new(false),
            fps_counter: RefCell::new(FpsCounter::new()),
        }
    }

    fn render(&self, delta_sec: f32) {
        let ui = self.ui.ui();
        ui.window("Debug").build(|| {
            let fps = self.fps_counter.borrow_mut().update_fps(delta_sec);
            ui.text(format!("Fps: {}", fps));
            let scene = self.scene_manager.scn_scene();
            if let Some(s) = scene {
                let (name, sub_name) =
                    s.with_inner::<shared::openpal3::scene::ScnScene, _, _>(|s| {
                        (s.name().to_owned(), s.sub_name().to_owned())
                    });
                ui.text(format!("Scene: {} {}", name, sub_name));
            }

            let coord = self.scene_manager.director().as_ref().and_then(|d| {
                d.query_interface::<IAdventureDirector>().and_then(|adv| {
                    let state_role = adv
                        .with_inner::<shared::openpal3::directors::AdventureDirector, _, _>(
                            |adv| {
                                self.scene_manager
                                    .get_resolved_role(adv.sce_vm().state(), -1)
                            },
                        );
                    state_role.map(|e| e.transform().borrow().position())
                })
            });

            ui.text(format!("Coord: {:?}", &coord));
            TabBar::new("##debug_tab_bar").build(ui, || {
                Self::build_nav_tab(self.scene_manager.clone(), ui, coord.as_ref());
                Self::build_sce_tab(self.scene_manager.clone(), ui);
            });
        });
    }

    fn build_nav_tab(scene_manager: ComRc<ISceneManager>, ui: &Ui, coord: Option<&Vec3>) {
        TabItem::new("Nav").build(ui, || {
            if let Some(d) = scene_manager.director().as_ref() {
                if let Some(director) = d.query_interface::<IAdventureDirector>() {
                    director.with_inner::<shared::openpal3::directors::AdventureDirector, _, _>(
                        |d| {
                            let mut sce_vm = d.sce_vm_mut();
                            let pass_through = sce_vm.global_state_mut().pass_through_wall_mut();
                            ui.checkbox("无视地形", pass_through);

                            if let Some(s) = scene_manager.scn_scene() {
                                if ui.button("切换地图层") {
                                    let layer_count = s
                                        .with_inner::<shared::openpal3::scene::ScnScene, _, _>(
                                            |s| s.nav().layer_count(),
                                        );
                                    if layer_count > 1 {
                                        if let Some(role) =
                                            scene_manager.get_resolved_role(sce_vm.state(), -1)
                                        {
                                            let r =
                                                RoleController::get_role_controller(role).unwrap();
                                            r.with_inner::<RoleController, _, _>(|r| {
                                                r.switch_nav_layer()
                                            });
                                        }
                                    }
                                }
                            }
                        },
                    );
                }
            }

            TabBar::new("##debug_tab_bar_nav_bar").build(ui, || {
                let scn = match scene_manager.scn_scene() {
                    Some(s) => s,
                    None => return,
                };
                let layer_count = scn.with_inner::<shared::openpal3::scene::ScnScene, _, _>(|s| {
                    s.nav().layer_count()
                });
                for layer in 0..layer_count {
                    TabItem::new(&format!("Layer {}", layer)).build(ui, || {
                        let current_nav_coord = coord.as_ref().map(|c| {
                            scn.with_inner::<shared::openpal3::scene::ScnScene, _, _>(|s| {
                                s.scene_coord_to_nav_coord(layer, c)
                            })
                        });

                        ui.text(format!("Nav Coord: {:?}", &current_nav_coord));

                        if let Some(nav_coord) = current_nav_coord {
                            let height =
                                scn.with_inner::<shared::openpal3::scene::ScnScene, _, _>(|s| {
                                    s.get_height(layer, nav_coord)
                                });
                            ui.text(format!("Height: {:?}", &height));
                        }

                        let text = scn.with_inner::<shared::openpal3::scene::ScnScene, _, _>(|s| {
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

                                        return if distance > 0 {
                                            "=".to_string()
                                        } else {
                                            "_".to_string()
                                        };
                                    })();
                                    text += ch.as_str();
                                }

                                text += "\n";
                            }

                            text
                        });

                        InputTextMultiline::new(
                            ui,
                            &format!("##debug_nav_text"),
                            &mut text.to_string(),
                            [-1., -1.],
                        )
                        .read_only(true)
                        .build();
                    });
                }
            });
        });
    }

    fn build_sce_tab(scene_manager: ComRc<ISceneManager>, ui: &Ui) {
        TabItem::new("Sce").build(ui, || {
            if let Some(d) = scene_manager.director().as_ref() {
                if let Some(d) = d.query_interface::<IAdventureDirector>() {
                    d.with_inner::<shared::openpal3::directors::AdventureDirector, _, _>(|d| {
                        d.sce_vm_mut().render_debug(scene_manager.clone(), ui);
                    });
                }
            }
        });
    }
}

impl DebugLayer for OpenPal3DebugLayer {
    fn update(&self, delta_sec: f32) {
        let ui = self.ui.ui();
        let fonts = ui.fonts().fonts();
        let font = if fonts.len() > 1 {
            Some(ui.push_font(fonts[1]))
        } else {
            None
        };

        (|| {
            if self
                .input_engine
                .borrow()
                .get_key_state(Key::Tilde)
                .pressed()
            {
                let visible = *self.visible.borrow();
                self.visible.replace(!visible);
            }

            if !*self.visible.borrow() {
                return;
            }

            self.render(delta_sec);
        })();

        if let Some(font) = font {
            font.pop();
        }
    }
}
