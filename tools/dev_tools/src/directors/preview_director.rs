use imgui::im_str;
use opengb::{
    asset_manager::AssetManager,
    loaders::{mv3_loader::mv3_load_from_file, pol_loader::pol_load_from_file},
    scene::{CvdModelEntity, PolModelEntity, RoleAnimation, RoleAnimationRepeatMode, RoleEntity},
};
use radiance::{
    math::Vec3,
    scene::{CoreEntity, Director, Entity, SceneManager},
};
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
                    let mut e = Box::new(CoreEntity::new(
                        RoleEntity::new_from_idle_animation(
                            self.asset_mgr.clone(),
                            "preview",
                            "preview",
                            a,
                        ),
                        "preview".to_string(),
                    ));
                    e.set_active(true);
                    e as Box<dyn Entity>
                })
                .ok()
            }
            Some("pol") => Some(Box::new(CoreEntity::new(
                PolModelEntity::new(
                    &self.asset_mgr.component_factory(),
                    &self.asset_mgr.vfs(),
                    &self.path,
                ),
                "preview".to_string(),
            )) as Box<dyn Entity>),
            Some("cvd") => Some(Box::new(CvdModelEntity::create(
                self.asset_mgr.component_factory().clone(),
                &self.asset_mgr.vfs(),
                &self.path,
                "preview".to_string(),
            )) as Box<dyn Entity>),
            _ => None,
        };

        let scene = scene_manager.scene_mut().unwrap();
        if let Some(mut e) = entity {
            e.load();
            scene.add_entity(e)
        }

        scene
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(0., 200., 200.))
            .look_at(&Vec3::new(0., 0., 0.));
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
