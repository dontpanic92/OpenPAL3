use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crate::{
    openpal3::{
        asset_manager::AssetManager,
        directors::SceneManagerExtensions,
        scene::{LadderTestResult, RoleController},
        states::{global_state::GlobalState, persistent_state::PersistentState},
    },
    scripting::sce::vm::{SceExecutionOptions, SceVm},
    utils::{get_camera_rotation, get_moving_direction},
};

use crosscom::ComRc;
use log::debug;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, IEntityExt, ISceneExt, ISceneManager},
    input::{InputEngine, Key},
    math::Vec3,
    radiance::UiManager,
};

use crate::agent_common::AgentBridge;

pub struct AdventureDirector {
    props: RefCell<AdventureDirectorProps>,
}

ComObject_AdventureDirector!(super::AdventureDirector);

impl AdventureDirector {
    pub fn new(
        app_name: &str,
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        ui: Rc<UiManager>,
        scene_manager: ComRc<ISceneManager>,
        sce_vm_options: Option<SceExecutionOptions>,
        dialog_renderer: ComRc<crate::openpal3::comdef::IPal3DialogRenderer>,
        status_renderer: ComRc<crate::openpal3::comdef::IPal3StatusRenderer>,
    ) -> Self {
        let p_state = Rc::new(RefCell::new(PersistentState::new(app_name.to_string())));
        let global_state = GlobalState::new(asset_mgr.clone(), audio_engine.clone(), p_state);
        let mut sce_vm = SceVm::new(
            audio_engine.clone(),
            input_engine.clone(),
            ui,
            scene_manager.clone(),
            asset_mgr.load_init_sce(),
            "init".to_string(),
            asset_mgr.clone(),
            global_state,
            sce_vm_options,
            dialog_renderer,
            status_renderer,
        );
        sce_vm.call_proc(51);

        Self {
            props: RefCell::new(AdventureDirectorProps {
                input_engine,
                scene_manager,
                sce_vm,
                camera_rotation: 0.,
                layer_switch_triggered: false,
                agent_bridge: None,
            }),
        }
    }

    fn props_mut(&self) -> RefMut<'_, AdventureDirectorProps> {
        self.props.borrow_mut()
    }

    pub fn load(
        app_name: &str,
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        input_engine: Rc<RefCell<dyn InputEngine>>,
        ui: Rc<UiManager>,
        scene_manager: ComRc<ISceneManager>,
        sce_vm_options: Option<SceExecutionOptions>,
        slot: i32,
        dialog_renderer: ComRc<crate::openpal3::comdef::IPal3DialogRenderer>,
        status_renderer: ComRc<crate::openpal3::comdef::IPal3StatusRenderer>,
    ) -> Option<Self> {
        let p_state = PersistentState::load(app_name, slot);
        let scene_name = p_state.scene_name();
        let sub_scene_name = p_state.sub_scene_name();
        if scene_name.is_none() || sub_scene_name.is_none() {
            log::error!("Cannot load save {}: scene or sub_scene is empty", slot);
            return None;
        }

        let scene = asset_mgr.load_scn(
            scene_name.as_ref().unwrap(),
            sub_scene_name.as_ref().unwrap(),
        );

        scene_manager.push_scene(scene);

        let mut global_state = GlobalState::new(
            asset_mgr.clone(),
            audio_engine.clone(),
            Rc::new(RefCell::new(p_state)),
        );

        // The role id should be saved in persistant state
        let role_entity = scene_manager
            .scn_scene()
            .unwrap()
            .inner::<crate::openpal3::scene::ScnScene>()
            .get_role_entity(0)
            .unwrap();

        let role = RoleController::get_role_controller(role_entity.clone()).unwrap();
        role.inner::<RoleController>().set_active(true);
        role_entity
            .transform()
            .borrow_mut()
            .set_position(&global_state.persistent_state_mut().position());

        global_state.play_default_bgm();

        let mut sce_vm = SceVm::new(
            audio_engine.clone(),
            input_engine.clone(),
            ui,
            scene_manager.clone(),
            asset_mgr.load_sce(scene_name.as_ref().unwrap()),
            scene_name.as_ref().unwrap().clone(),
            asset_mgr.clone(),
            global_state,
            sce_vm_options,
            dialog_renderer,
            status_renderer,
        );

        // don't draw curtain when loading a save
        sce_vm.state_mut().set_curtain(0.);
        sce_vm.state_mut().try_call_proc_by_name(&format!(
            "_{}_{}",
            scene_name.as_ref().unwrap(),
            sub_scene_name.as_ref().unwrap()
        ));

        Some(Self {
            props: RefCell::new(AdventureDirectorProps {
                input_engine,
                scene_manager: scene_manager.clone(),
                sce_vm,
                camera_rotation: 0.,
                layer_switch_triggered: false,
                agent_bridge: None,
            }),
        })
    }

    pub fn sce_vm(&self) -> Ref<'_, SceVm> {
        Ref::map(self.props.borrow(), |p| &p.sce_vm)
    }

    pub fn sce_vm_mut(&self) -> RefMut<'_, SceVm> {
        RefMut::map(self.props.borrow_mut(), |p| &mut p.sce_vm)
    }

    /// Install the agent bridge so this director honors pause/step
    /// and exposes its tick rate to `/v1/state`. Called by
    /// `Pal3Service::set_agent_bridge` after the agent server is
    /// booted (so menus exit and a fresh `AdventureDirector` gets the
    /// bridge before its first tick).
    pub fn set_agent_bridge(&self, bridge: Rc<AgentBridge>) {
        self.props.borrow_mut().agent_bridge = Some(bridge);
    }

    /// Currently-installed bridge, if any. Used by `Pal3Service`'s
    /// agent dispatcher to inspect/replace it when a new director is
    /// pushed onto the scene manager.
    pub fn agent_bridge(&self) -> Option<Rc<AgentBridge>> {
        self.props.borrow().agent_bridge.clone()
    }

    /// Resolve the currently-controlled role entity (player slot
    /// returned by `GlobalState::role_controlled`). `None` when no
    /// scene is mounted or the role cannot be resolved.
    pub fn controlled_role_position(&self) -> Option<Vec3> {
        let p = self.props.borrow();
        if p.scene_manager.scene().is_none() {
            return None;
        }
        let role = p.scene_manager.get_resolved_role(p.sce_vm.state(), -1)?;
        Some(role.transform().borrow().position())
    }

    /// Teleport the leader (the role returned by `role_controlled`)
    /// to `pos`. Mirrors the agent server's `/v1/player/teleport`
    /// semantics. Returns `false` when no scene/role is available.
    pub fn teleport_controlled_role(&self, pos: Vec3) -> bool {
        let p = self.props.borrow();
        if p.scene_manager.scene().is_none() {
            return false;
        }
        let role = match p.scene_manager.get_resolved_role(p.sce_vm.state(), -1) {
            Some(r) => r,
            None => return false,
        };
        role.transform().borrow_mut().set_position(&pos);
        drop(p);
        // Mirror to persistent state so a subsequent save records the
        // teleported position.
        let mut pm = self.props.borrow_mut();
        pm.sce_vm
            .global_state_mut()
            .persistent_state_mut()
            .set_position(pos);
        true
    }
}

impl IDirectorImpl for AdventureDirector {
    fn activate(&self) {
        debug!("AdventureDirector activated");
    }

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        // Honor `/v1/time/pause` + `/v1/time/step`: when an agent
        // bridge is installed and we're paused with no pending steps,
        // skip the entire SCE tick so movement, animations, and the
        // VM all freeze in lockstep. Stepped frames use the
        // bridge-provided fixed `dt`.
        //
        // The same borrow also reads `/v1/time/fast_forward`, which we
        // push onto the SCE state so commands can skip input-blocked
        // waits (dialog / movie) and collapse timed tweens this frame.
        let (effective_dt, fast_forward) = match self.props.borrow().agent_bridge.as_ref() {
            Some(bridge) => match bridge.effective_dt(delta_sec) {
                (true, dt) => (dt, bridge.fast_forward.get()),
                (false, _) => return None,
            },
            None => (delta_sec, false),
        };
        let mut props = self.props_mut();
        props.sce_vm.state_mut().set_fast_forward(fast_forward);
        props.do_update(effective_dt)
    }

    fn deactivate(&self) {}
}

struct AdventureDirectorProps {
    input_engine: Rc<RefCell<dyn InputEngine>>,
    scene_manager: ComRc<ISceneManager>,
    sce_vm: SceVm,
    camera_rotation: f32,
    layer_switch_triggered: bool,
    /// Agent-server bridge. `None` when the embedded HTTP listener
    /// is not running; `Some(_)` gates pause/step + provides the
    /// synthetic-input overlay backing `/v1/input/*`.
    agent_bridge: Option<Rc<AgentBridge>>,
}

impl AdventureDirectorProps {
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

    fn move_role(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        delta_sec: f32,
        moving_direction: &Vec3,
    ) {
        let role = scene_manager
            .get_resolved_role(self.sce_vm.state(), -1)
            .unwrap();
        let role_controller = RoleController::get_role_controller(role.clone()).unwrap();
        let mut position = role.transform().borrow().position();

        let nav_layer = role_controller.inner::<RoleController>().nav_layer();
        let speed = 175.;
        let mut target_position = Vec3::add(
            &position,
            &Vec3::scalar_mul(speed * delta_sec, &moving_direction),
        );
        let (target_nav_coord, height, distance_to_border) = {
            let scn = scene_manager.scn_scene().unwrap();
            let scene = scn.inner::<crate::openpal3::scene::ScnScene>();
            let tnc = scene.scene_coord_to_nav_coord(nav_layer, &target_position);
            let h = scene.get_height(nav_layer, tnc);
            let d = scene.get_distance_to_border_by_scene_coord(nav_layer, &target_position);
            (tnc, h, d)
        };
        let _ = target_nav_coord;
        target_position.y = height;

        let role = scene_manager
            .get_resolved_role(self.sce_vm.state(), -1)
            .unwrap();
        if moving_direction.norm() > 0.5
            && (self.sce_vm.global_state().pass_through_wall()
                || distance_to_border > std::f32::EPSILON)
        {
            role_controller.inner::<RoleController>().run();
            let look_at = Vec3::new(target_position.x, position.y, target_position.z);
            role.transform()
                .borrow_mut()
                .look_at(&look_at)
                .set_position(&target_position);

            self.sce_vm
                .global_state_mut()
                .persistent_state_mut()
                .set_position(target_position);

            position = target_position
        } else {
            role_controller.inner::<RoleController>().idle();
        }

        {
            let scene = scene_manager.scene().unwrap();
            scene
                .camera_mut()
                .transform_mut()
                .set_position(&Vec3::new(400., 400., 400.))
                .rotate_axis_angle(&Vec3::UP, self.camera_rotation)
                .translate(&position)
                .look_at(&position);
        }
    }

    fn do_update(&mut self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        self.sce_vm.update(delta_sec);

        // Freeze the whole game world while the full-screen status (状态)
        // menu is open, but only when the player actually has control —
        // never mid-cutscene, where the SCE VM is driving animations.
        // `IScene::set_active(false)` makes the scene's per-frame entity
        // update a no-op, so NPC patrols, role animations and world
        // transforms all hold their pose; we also skip the world sim
        // below so movement/triggers stop too.
        let adv_enabled = self.sce_vm.global_state().adv_input_enabled();
        let menu_open = self.sce_vm.state().status_renderer().is_menu_open();
        let freeze = adv_enabled && menu_open;
        if let Some(scene) = self.scene_manager.scene() {
            scene.set_active(!freeze);
        }

        if !adv_enabled {
            return None;
        }
        if freeze {
            return None;
        }

        if self.scene_manager.scene().is_none() {
            return None;
        }

        self.test_save();

        let moving_direction = get_moving_direction(
            self.input_engine.clone(),
            self.scene_manager.scene().unwrap(),
        );
        self.move_role(self.scene_manager.clone(), delta_sec, &moving_direction);

        // Advance ambient NPC patrols (non-scripted townsfolk walking their
        // authored loop). Scripted roles are untouched.
        {
            let scene_rc = self.scene_manager.scn_scene().unwrap();
            scene_rc
                .inner::<crate::openpal3::scene::ScnScene>()
                .step_patrols(delta_sec);
        }

        self.camera_rotation =
            get_camera_rotation(self.input_engine.clone(), self.camera_rotation, delta_sec);

        let (position, nav_layer) = {
            let role = self
                .scene_manager
                .get_resolved_role(self.sce_vm.state(), -1)
                .unwrap();
            let r = RoleController::get_role_controller(role.clone()).unwrap();
            (
                role.transform().borrow().position(),
                r.inner::<RoleController>().nav_layer(),
            )
        };

        let scene_rc = self.scene_manager.scn_scene().unwrap();
        let proc_id_opt = {
            let s = scene_rc.inner::<crate::openpal3::scene::ScnScene>();
            s.test_nav_trigger(nav_layer, &position)
        };
        if let Some(proc_id) = proc_id_opt {
            debug!("New proc triggerd by nav: {}", proc_id);
            self.sce_vm.call_proc(proc_id);
        }

        let nav_layer_triggered = {
            let s = scene_rc.inner::<crate::openpal3::scene::ScnScene>();
            s.test_nav_layer_trigger(nav_layer, &position)
        };
        if nav_layer_triggered {
            if !self.layer_switch_triggered {
                let layer = {
                    let e = self
                        .scene_manager
                        .get_resolved_role(self.sce_vm.state(), -1)
                        .unwrap();
                    let r = RoleController::get_role_controller(e).unwrap();
                    r.inner::<RoleController>().nav_layer()
                };
                let new_layer = (layer + 1) % 2;

                let mut test_coord = position;
                let mut d = 0.0;
                for _i in 0..50 {
                    d = {
                        let s = scene_rc.inner::<crate::openpal3::scene::ScnScene>();
                        s.get_distance_to_border_by_scene_coord(new_layer, &test_coord)
                    };
                    if d > 0.0 {
                        break;
                    }

                    test_coord = Vec3::add(&test_coord, &moving_direction);
                }

                if d > 0.0 {
                    let e = self
                        .scene_manager
                        .get_resolved_role(self.sce_vm.state(), -1)
                        .unwrap();
                    let r = RoleController::get_role_controller(e).unwrap();
                    r.inner::<RoleController>().switch_nav_layer();
                    self.scene_manager
                        .get_resolved_role(self.sce_vm.state(), -1)
                        .unwrap()
                        .transform()
                        .borrow_mut()
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
            let trigger_proc_id = {
                let scene = scene_rc.inner::<crate::openpal3::scene::ScnScene>();
                scene
                    .test_aabb_trigger(&position)
                    .or_else(|| scene.test_item_trigger(&position))
                    .or_else(|| {
                        scene.test_role_trigger(
                            &position,
                            self.sce_vm.global_state().role_controlled(),
                        )
                    })
            };
            if let Some(proc_id) = trigger_proc_id {
                debug!("New proc triggerd: {}", proc_id);
                self.sce_vm.call_proc(proc_id);
            }

            let result = {
                let scene = scene_rc.inner::<crate::openpal3::scene::ScnScene>();
                scene.test_ladder(nav_layer, &position)
            };
            match result {
                Some(LadderTestResult::NewPosition((new_layer, new_position))) => {
                    debug!(
                        "Ladder detected, new_layer: {:?} new position: {:?}",
                        &new_layer, &new_position
                    );

                    if new_layer {
                        let e = self
                            .scene_manager
                            .get_resolved_role(self.sce_vm.state(), -1)
                            .unwrap();
                        let r = RoleController::get_role_controller(e).unwrap();
                        r.inner::<RoleController>().switch_nav_layer();
                    }

                    self.scene_manager
                        .get_resolved_role(self.sce_vm.state(), -1)
                        .unwrap()
                        .transform()
                        .borrow_mut()
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
