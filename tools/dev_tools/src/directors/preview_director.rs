use imgui::im_str;
use opengb::{
    asset_manager::AssetManager,
    loaders::mv3_loader::mv3_load_from_file,
    scene::{RoleAnimation, RoleAnimationRepeatMode, RoleEntity},
};
use radiance::{math::Vec3, scene::{CoreEntity, Director, Entity, SceneManager}};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use super::main_director::DevToolsDirector;

pub struct PreviewDirector {
    main_director: Rc<RefCell<DevToolsDirector>>,
    asset_mgr: Rc<AssetManager>,
    path: PathBuf,
}

impl PreviewDirector {
    pub fn new(
        main_director: Rc<RefCell<DevToolsDirector>>,
        asset_mgr: Rc<AssetManager>,
        path: PathBuf,
    ) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            main_director,
            asset_mgr,
            path,
        }))
    }
}

impl Director for PreviewDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        let entity = match self
            .path
            .extension()
            .map(|e| e.to_str().unwrap().to_ascii_lowercase())
            .as_ref()
            .map(|e| e.as_str())
        {
            Some("mv3") => {
                let mv3file = mv3_load_from_file(self.asset_mgr.vfs(), &self.path);
                let anim = mv3file.as_ref().map(|f| {
                    RoleAnimation::new(
                        &self.asset_mgr.component_factory(),
                        f,
                        self.asset_mgr.load_mv3_material(f, &self.path),
                        RoleAnimationRepeatMode::NoRepeat,
                    )
                });

                anim.map(|a| {
                    Box::new(CoreEntity::new(
                        RoleEntity::new_from_idle_animation(
                            self.asset_mgr.clone(),
                            "preview",
                            "preview",
                            a,
                        ),
                        "preview",
                    ))
                })
                .ok()
            }
            _ => None,
        };

        let scene = scene_manager.scene_mut().unwrap();
        if let Some(mut e) = entity {
            e.set_active(true);
            scene.add_entity(e)
        }

        scene
            .camera_mut()
            .transform_mut()
            .translate_local(&Vec3::new(0., 100., 400.))
            .look_at(&Vec3::new(0., 50., 0.));
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        if ui.button(im_str!("Back"), [80., 32.]) {
            scene_manager
                .scene_mut()
                .unwrap()
                .root_entities_mut()
                .clear();
            return Some(self.main_director.clone());
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

        None
    }
}
