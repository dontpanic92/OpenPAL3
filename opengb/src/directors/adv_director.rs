use std::{cell::RefCell, rc::Rc};

use crate::asset_manager::AssetManager;

use super::{
    global_state::GlobalState,
    sce_vm::{SceExecutionOptions, SceVm},
    PersistentState, SceneManagerExtensions,
};
use log::debug;
use radiance::{
    audio::AudioEngine,
    input::{InputEngine, Key},
    math::{Mat44, Vec3},
    scene::{CoreScene, Director, Entity, SceneManager},
};

pub struct AdventureDirector {
    input_engine: Rc<RefCell<dyn InputEngine>>,
    sce_vm: SceVm,
    camera_rotation: f32,
}

impl AdventureDirector {
    pub fn new(
        app_name: &str,
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        sce_vm_options: Option<SceExecutionOptions>,
    ) -> Self {
        let p_state = Rc::new(RefCell::new(PersistentState::new(app_name.to_string())));
        let global_state = GlobalState::new(asset_mgr.clone(), &audio_engine, p_state);
        let mut sce_vm = SceVm::new(
            audio_engine.clone(),
            input_engine.clone(),
            asset_mgr.load_init_sce(),
            "init".to_string(),
            asset_mgr.clone(),
            global_state,
            sce_vm_options,
        );
        sce_vm.call_proc(51);

        Self {
            sce_vm,
            input_engine,
            camera_rotation: 0.,
        }
    }

    pub fn load(
        app_name: &str,
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        scene_manager: &mut dyn SceneManager,
        sce_vm_options: Option<SceExecutionOptions>,
        slot: i32,
    ) -> Option<Self> {
        let p_state = PersistentState::load(app_name, slot);
        let scene_name = p_state.scene_name();
        let sub_scene_name = p_state.sub_scene_name();
        if scene_name.is_none() || sub_scene_name.is_none() {
            log::error!("Cannot load save {}: scene or sub_scene is empty", slot);
            return None;
        }

        let scene = Box::new(CoreScene::new(asset_mgr.load_scn(
            scene_name.as_ref().unwrap(),
            sub_scene_name.as_ref().unwrap(),
        )));
        scene_manager.push_scene(scene);

        let mut global_state = GlobalState::new(
            asset_mgr.clone(),
            &audio_engine,
            Rc::new(RefCell::new(p_state)),
        );

        // The role id should be saved in persistant state
        let role = scene_manager
            .core_scene_mut_or_fail()
            .get_role_entity_mut(0)
            .unwrap();
        role.set_active(true);
        role.transform_mut()
            .set_position(&global_state.persistent_state_mut().position());

        global_state.play_default_bgm();

        let sce_vm = SceVm::new(
            audio_engine.clone(),
            input_engine.clone(),
            asset_mgr.load_sce(scene_name.as_ref().unwrap()),
            scene_name.unwrap(),
            asset_mgr.clone(),
            global_state,
            sce_vm_options,
        );

        Some(Self {
            sce_vm,
            input_engine,
            camera_rotation: 0.,
        })
    }

    fn test_save(&self) {
        let input = self.input_engine.borrow_mut();
        let save_slot = if input.get_key_state(Key::Num1).pressed() {
            1
        } else if input.get_key_state(Key::Num2).pressed() {
            2
        } else if input.get_key_state(Key::Num3).pressed() {
            3
        } else if input.get_key_state(Key::Num4).pressed() {
            4
        } else {
            -1
        };

        self.sce_vm
            .global_state()
            .persistent_state()
            .save(save_slot);
    }
}

impl Director for AdventureDirector {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager) {
        debug!("AdventureDirector activated");
    }

    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>> {
        self.sce_vm.update(scene_manager, ui, delta_sec);
        if !self.sce_vm.global_state().input_enabled() {
            return None;
        }

        if scene_manager.scene_mut().is_none() {
            return None;
        }

        self.test_save();
        let input = self.input_engine.borrow_mut();
        let mut direction = Vec3::new(0., 0., 0.);

        if input.get_key_state(Key::Up).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(0., 0., -1.));
        }

        if input.get_key_state(Key::Down).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(0., 0., 1.));
        }

        if input.get_key_state(Key::Left).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(-1., 0., 0.));
        }

        if input.get_key_state(Key::Right).is_down() {
            direction = Vec3::add(&direction, &Vec3::new(1., 0., 0.));
        }

        let camera_mat = &scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform()
            .matrix();
        let mut direction_mat = Mat44::new_zero();
        direction_mat[0][3] = direction.x;
        direction_mat[1][3] = direction.y;
        direction_mat[2][3] = direction.z;
        let direction_mat = Mat44::multiplied(&camera_mat, &direction_mat);
        let mut direction = Vec3::new(direction_mat[0][3], 0., direction_mat[2][3]);
        direction.normalize();

        const CAMERA_ROTATE_SPEED: f32 = 1.5;
        if input.get_key_state(Key::A).is_down() {
            self.camera_rotation -= CAMERA_ROTATE_SPEED * delta_sec;
            if self.camera_rotation < 0. {
                self.camera_rotation += std::f32::consts::PI * 2.;
            }
        }

        if input.get_key_state(Key::D).is_down() {
            self.camera_rotation += CAMERA_ROTATE_SPEED * delta_sec;

            if self.camera_rotation > std::f32::consts::PI * 2. {
                self.camera_rotation -= std::f32::consts::PI * 2.;
            }
        }

        let position = scene_manager
            .get_resolved_role(self.sce_vm.state(), -1)
            .unwrap()
            .transform()
            .position();

        scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(400., 400., 400.))
            .rotate_axis_angle(&Vec3::UP, self.camera_rotation)
            .translate(&position)
            .look_at(&position);

        let scene = scene_manager.core_scene_mut_or_fail();
        let speed = 175.;
        let mut target_position = Vec3::add(&position, &Vec3::dot(speed * delta_sec, &direction));
        let target_nav_coord = scene.scene_coord_to_nav_coord(&target_position);
        let height = scene.get_height(target_nav_coord);
        target_position.y = height;
        let distance_to_border = scene.get_distance_to_border_by_scene_coord(&target_position);

        if let Some(proc_id) = scene.test_nav_trigger(&target_position) {
            debug!("New proc triggerd by nav: {}", proc_id);
            self.sce_vm.call_proc(proc_id);
        }

        if input.get_key_state(Key::F).pressed() {
            if let Some(proc_id) = scene
                .test_aabb_trigger(&position)
                .or_else(|| scene.test_item_trigger(&position))
                .or_else(|| scene.test_role_trigger(&position))
            {
                debug!("New proc triggerd: {}", proc_id);
                self.sce_vm.call_proc(proc_id);
            }

            if let Some(new_position) = scene.test_ladder(&position) {
                debug!("Ladder detected, new position: {:?}", &new_position);
                scene_manager
                    .get_resolved_role_mut(self.sce_vm.state(), -1)
                    .unwrap()
                    .transform_mut()
                    .set_position(&new_position);
            }
        }

        let role = scene_manager
            .get_resolved_role_mut(self.sce_vm.state(), -1)
            .unwrap();
        if direction.norm() > 0.5 && distance_to_border > std::f32::EPSILON {
            role.run();
            let look_at = Vec3::new(target_position.x, position.y, target_position.z);
            role.transform_mut()
                .look_at(&look_at)
                .set_position(&target_position);

            self.sce_vm
                .global_state_mut()
                .persistent_state_mut()
                .set_position(target_position);
        } else {
            role.idle();
        }

        None
    }
}
