use std::{cell::RefCell, rc::Rc};

use crate::{asset_manager::AssetManager, scene::LadderTestResult};

use super::{
    global_state::GlobalState,
    sce_vm::{SceExecutionOptions, SceVm},
    PersistentState, SceneManagerExtensions,
};
use log::debug;
use radiance::{
    audio::AudioEngine,
    input::{Axis, InputEngine, Key},
    math::{Mat44, Vec3},
    scene::{CoreScene, Director, Entity, SceneManager},
};

pub struct AdventureDirector {
    input_engine: Rc<RefCell<dyn InputEngine>>,
    sce_vm: SceVm,
    camera_rotation: f32,
    layer_switch_triggered: bool,
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
            layer_switch_triggered: false,
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
            layer_switch_triggered: false,
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

    pub fn sce_vm(&self) -> &SceVm {
        &self.sce_vm
    }

    pub fn sce_vm_mut(&mut self) -> &mut SceVm {
        &mut self.sce_vm
    }

    fn move_role(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
        moving_direction: &Vec3,
    ) {
        let camera_mat = &scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform()
            .matrix();
        let mut direction_mat = Mat44::new_zero();
        direction_mat[0][3] = moving_direction.x;
        direction_mat[1][3] = moving_direction.y;
        direction_mat[2][3] = moving_direction.z;
        let direction_mat = Mat44::multiplied(&camera_mat, &direction_mat);
        let mut direction = Vec3::new(direction_mat[0][3], 0., direction_mat[2][3]);
        direction.normalize();

        let role = scene_manager
            .get_resolved_role(self.sce_vm.state(), -1)
            .unwrap();
        let mut position = role.transform().position();

        let scene = scene_manager.core_scene_or_fail();
        let speed = 175.;
        let mut target_position = Vec3::add(&position, &Vec3::dot(speed * delta_sec, &direction));
        let target_nav_coord = scene.scene_coord_to_nav_coord(role.nav_layer(), &target_position);
        let height = scene.get_height(role.nav_layer(), target_nav_coord);
        target_position.y = height;
        let distance_to_border =
            scene.get_distance_to_border_by_scene_coord(role.nav_layer(), &target_position);

        let role = scene_manager
            .get_resolved_role_mut(self.sce_vm.state(), -1)
            .unwrap();
        if direction.norm() > 0.5
            && (self.sce_vm.global_state().pass_through_wall()
                || distance_to_border > std::f32::EPSILON)
        {
            role.run();
            let look_at = Vec3::new(target_position.x, position.y, target_position.z);
            role.transform_mut()
                .look_at(&look_at)
                .set_position(&target_position);

            self.sce_vm
                .global_state_mut()
                .persistent_state_mut()
                .set_position(target_position);

            position = target_position
        } else {
            role.idle();
        }

        scene_manager
            .scene_mut()
            .unwrap()
            .camera_mut()
            .transform_mut()
            .set_position(&Vec3::new(400., 400., 400.))
            .rotate_axis_angle(&Vec3::UP, self.camera_rotation)
            .translate(&position)
            .look_at(&position);
    }

    fn rotate_camera(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut imgui::Ui,
        delta_sec: f32,
    ) {
        let input = self.input_engine.borrow();
        const CAMERA_ROTATE_SPEED: f32 = 1.5;
        if input.get_key_state(Key::A).is_down() {
            self.camera_rotation -= CAMERA_ROTATE_SPEED * delta_sec;
        }

        if input.get_key_state(Key::D).is_down() {
            self.camera_rotation += CAMERA_ROTATE_SPEED * delta_sec;
        }

        self.camera_rotation -=
            CAMERA_ROTATE_SPEED * delta_sec * input.get_axis_state(Axis::RightStickX).value();

        if self.camera_rotation < 0. {
            self.camera_rotation += std::f32::consts::PI * 2.;
        }

        if self.camera_rotation > std::f32::consts::PI * 2. {
            self.camera_rotation -= std::f32::consts::PI * 2.;
        }
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
        if !self.sce_vm.global_state().adv_input_enabled() {
            return None;
        }

        if scene_manager.scene_mut().is_none() {
            return None;
        }

        self.test_save();

        let moving_direction = {
            let input = self.input_engine.borrow_mut();
            let mut direction = Vec3::new(0., 0., 0.);

            if input.get_key_state(Key::Up).is_down()
                || input.get_key_state(Key::GamePadDPadUp).is_down()
            {
                direction = Vec3::add(&direction, &Vec3::new(0., 0., -1.));
            }

            if input.get_key_state(Key::Down).is_down()
                || input.get_key_state(Key::GamePadDPadDown).is_down()
            {
                direction = Vec3::add(&direction, &Vec3::new(0., 0., 1.));
            }

            direction = Vec3::add(
                &direction,
                &Vec3::new(0., 0., -input.get_axis_state(Axis::LeftStickY).value()),
            );

            if input.get_key_state(Key::Left).is_down()
                || input.get_key_state(Key::GamePadDPadLeft).is_down()
            {
                direction = Vec3::add(&direction, &Vec3::new(-1., 0., 0.));
            }

            if input.get_key_state(Key::Right).is_down()
                || input.get_key_state(Key::GamePadDPadRight).is_down()
            {
                direction = Vec3::add(&direction, &Vec3::new(1., 0., 0.));
            }

            direction = Vec3::add(
                &direction,
                &Vec3::new(input.get_axis_state(Axis::LeftStickX).value(), 0., 0.),
            );
            Vec3::normalized(&direction)
        };

        self.move_role(scene_manager, ui, delta_sec, &moving_direction);
        self.rotate_camera(scene_manager, ui, delta_sec);

        let (position, nav_layer) = {
            let role = scene_manager
                .get_resolved_role(self.sce_vm.state(), -1)
                .unwrap();
            (role.transform().position(), role.nav_layer())
        };

        macro_rules! scene {
            () => {
                scene_manager.core_scene_or_fail()
            };
        }

        if let Some(proc_id) = scene!().test_nav_trigger(nav_layer, &position) {
            debug!("New proc triggerd by nav: {}", proc_id);
            self.sce_vm.call_proc(proc_id);
        }

        if scene!().test_nav_layer_trigger(nav_layer, &position) {
            if !self.layer_switch_triggered {
                let layer = scene_manager
                    .get_resolved_role_mut(self.sce_vm.state(), -1)
                    .unwrap()
                    .nav_layer();
                let new_layer = (layer + 1) % 2;

                let mut test_coord = position;
                let mut d = 0.0;
                for i in 0..50 {
                    d = scene_manager
                        .core_scene_or_fail()
                        .get_distance_to_border_by_scene_coord(new_layer, &test_coord);
                    if d > 0.0 {
                        break;
                    }

                    test_coord = Vec3::add(&test_coord, &moving_direction);
                }

                if d > 0.0 {
                    scene_manager
                        .get_resolved_role_mut(self.sce_vm.state(), -1)
                        .unwrap()
                        .switch_nav_layer();
                    scene_manager
                        .get_resolved_role_mut(self.sce_vm.state(), -1)
                        .unwrap()
                        .transform_mut()
                        .set_position(&test_coord);
                    self.layer_switch_triggered = true;
                }
            }
        } else {
            self.layer_switch_triggered = false;
        }

        let input = self.input_engine.borrow_mut();
        if input.get_key_state(Key::F).pressed() || input.get_key_state(Key::GamePadEast).pressed()
        {
            if let Some(proc_id) = scene!()
                .test_aabb_trigger(&position)
                .or_else(|| scene!().test_item_trigger(&position))
                .or_else(|| {
                    scene!()
                        .test_role_trigger(&position, self.sce_vm.global_state().role_controlled())
                })
            {
                debug!("New proc triggerd: {}", proc_id);
                self.sce_vm.call_proc(proc_id);
            }

            let result = scene!().test_ladder(nav_layer, &position);
            match result {
                Some(LadderTestResult::NewPosition((new_layer, new_position))) => {
                    debug!(
                        "Ladder detected, new_layer: {:?} new position: {:?}",
                        &new_layer, &new_position
                    );

                    if new_layer {
                        scene_manager
                            .get_resolved_role_mut(self.sce_vm.state(), -1)
                            .unwrap()
                            .switch_nav_layer();
                    }

                    scene_manager
                        .get_resolved_role_mut(self.sce_vm.state(), -1)
                        .unwrap()
                        .transform_mut()
                        .set_position(&new_position);
                }
                Some(LadderTestResult::SceProc(proc_id)) => {
                    debug!("Ladder detected, proc_id {}", &proc_id);
                    self.sce_vm.call_proc(proc_id);
                }
                None => {}
            }
        }

        None
    }
}
