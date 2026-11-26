use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crate::{
    openpal3::{
        asset_manager::AssetManager,
        directors::SceneManagerExtensions,
        loaders::stats::StatsData,
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

fn apply_pickup_rewards(
    persistent: &mut PersistentState,
    data: &StatsData,
    rewards: Vec<(i32, i32)>,
) -> (Vec<String>, usize) {
    let mut notices = vec![];
    let mut gain_sounds = 0;
    for (item_id, money) in rewards {
        if item_id != 0 {
            let granted = persistent.grant_world_item(item_id, data);
            log::info!("PAL3 world pickup item {item_id}: granted={granted}");
            if granted {
                notices.push(format!("得到{}", data.item(item_id as u32).unwrap().name));
                gain_sounds += 1;
            }
        }
        if money != 0 {
            let before = persistent.money();
            persistent.add_money(money);
            let after = persistent.money();
            let delta = after - before;
            log::info!("PAL3 world pickup money: {money}, actual change: {delta}");
            if after == 0 {
                notices.push("失去所有钱".into());
            } else if delta < 0 {
                notices.push(format!("失去{}文钱", -delta));
            } else {
                notices.push(format!("得到{delta}文钱"));
                gain_sounds += 1;
            }
        }
    }
    (notices, gain_sounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pickup_feedback_only_sounds_for_valid_items_and_uses_actual_money_changes() {
        let data = crate::openpal3::loaders::stats::test_data();
        for (balance, amount, expected, sounds) in [
            (0, 25, "得到25文钱", 1),
            (100, -25, "失去25文钱", 0),
            (10, -25, "失去所有钱", 0),
            (0, -25, "失去所有钱", 0),
            (i32::MAX, 1, "失去所有钱", 0),
        ] {
            let mut state = PersistentState::new("OpenPAL3".into());
            state.add_money(-state.money());
            state.add_money(balance);
            let (notices, count) =
                apply_pickup_rewards(&mut state, &data, vec![(42, amount), (-1, 0), (0, 0)]);
            assert_eq!(
                notices,
                vec![
                    format!("得到{}", data.item(42).unwrap().name),
                    expected.into()
                ]
            );
            assert_eq!(count, sounds + 1);
            assert_eq!(state.inventory().quantity(42), 1);
            assert_eq!(state.money(), balance.wrapping_add(amount).max(0));
        }
        let mut state = PersistentState::new("OpenPAL3".into());
        assert_eq!(
            apply_pickup_rewards(&mut state, &data, vec![(-1, 0), (123456, 0)]),
            (vec![], 0)
        );
    }
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
        let mut p_state = PersistentState::new(app_name.to_string());
        if asset_mgr.game() == crate::GameType::PAL3 {
            p_state
                .stats_mut()
                .fill_all_resources(&asset_mgr.stats_data());
        }
        let p_state = Rc::new(RefCell::new(p_state));
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
                hazard_cooldown: 0.,
                pending_load: None,
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
        let p_state = match PersistentState::try_load(app_name, slot) {
            Ok(state) => state,
            Err(error) => {
                log::error!("Cannot load save {slot}: {error}");
                return None;
            }
        };
        let scene_name = p_state.scene_name();
        let sub_scene_name = p_state.sub_scene_name();
        if scene_name.is_none() || sub_scene_name.is_none() {
            log::error!("Cannot load save {}: scene or sub_scene is empty", slot);
            return None;
        }
        if !asset_mgr.scene_exists(
            scene_name.as_ref().unwrap(),
            sub_scene_name.as_ref().unwrap(),
        ) {
            log::error!("Cannot load save {slot}: saved scene is not installed");
            return None;
        }

        let show_loading = asset_mgr.game() == crate::GameType::PAL3;
        let global_state = GlobalState::new(
            asset_mgr.clone(),
            audio_engine.clone(),
            Rc::new(RefCell::new(p_state)),
        );

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

        let director = Self {
            props: RefCell::new(AdventureDirectorProps {
                input_engine,
                scene_manager: scene_manager.clone(),
                sce_vm,
                camera_rotation: 0.,
                layer_switch_triggered: false,
                hazard_cooldown: 0.,
                pending_load: show_loading.then_some(SaveLoadPhase::Paint),
                agent_bridge: None,
            }),
        };
        if !show_loading {
            let mut props = director.props_mut();
            props.create_scene(false);
            props.restore_scene();
        }
        Some(director)
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

    pub fn is_loading(&self) -> bool {
        let props = self.props.borrow();
        props.pending_load.is_some() || props.sce_vm.is_loading()
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
        {
            let mut props = self.props_mut();
            if let Some(phase) = props.pending_load {
                match phase {
                    SaveLoadPhase::Paint => {
                        props.sce_vm.state().render_loading(0., 0.);
                        props.pending_load = Some(SaveLoadPhase::Create);
                    }
                    SaveLoadPhase::Create => {
                        props.create_scene(true);
                        props.sce_vm.state().render_loading(0., 0.);
                        props.pending_load = Some(SaveLoadPhase::Load);
                    }
                    SaveLoadPhase::Load => {
                        let progress = props
                            .scene_manager
                            .scn_scene()
                            .unwrap()
                            .inner::<crate::openpal3::scene::ScnScene>()
                            .advance_loading();
                        if progress == 1. {
                            props.restore_scene();
                            props.pending_load = None;
                        }
                        props.sce_vm.state().render_loading(progress, delta_sec);
                    }
                }
                if let Some(scene) = props.scene_manager.scene() {
                    scene.set_active(false);
                }
                return None;
            }
        }
        // Honor `/v1/time/pause` + `/v1/time/step`: when an agent
        // bridge is installed and we're paused with no pending steps,
        // skip the entire SCE tick so movement, animations, and the
        // VM all freeze in lockstep. Stepped frames use the
        // bridge-provided fixed `dt`.
        //
        // The same borrow also reads `/v1/time/fast_forward`, which we
        // push onto the SCE state so commands can skip input-blocked
        // waits (dialog / movie) and collapse timed tweens this frame.
        let loading = self.props.borrow().sce_vm.is_loading();
        let (effective_dt, fast_forward) = match self.props.borrow().agent_bridge.as_ref() {
            Some(bridge) if loading => (delta_sec, bridge.fast_forward.get()),
            Some(bridge) => match bridge.effective_dt(delta_sec) {
                (true, dt) => (dt, bridge.fast_forward.get()),
                (false, _) => return None,
            },
            None => (delta_sec, false),
        };
        let mut props = self.props_mut();
        if props.sce_vm.global_state().asset_mgr().game() == crate::GameType::PAL3 {
            props
                .sce_vm
                .global_state_mut()
                .persistent_state_mut()
                .advance_play_time(effective_dt);
        }
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
    hazard_cooldown: f32,
    pending_load: Option<SaveLoadPhase>,
    /// Agent-server bridge. `None` when the embedded HTTP listener
    /// is not running; `Some(_)` gates pause/step + provides the
    /// synthetic-input overlay backing `/v1/input/*`.
    agent_bridge: Option<Rc<AgentBridge>>,
}

#[derive(Clone, Copy)]
enum SaveLoadPhase {
    Paint,
    Create,
    Load,
}

impl AdventureDirectorProps {
    fn create_scene(&mut self, deferred: bool) {
        let (name, sub_name) = {
            let state = self.sce_vm.global_state().persistent_state();
            (state.scene_name().unwrap(), state.sub_scene_name().unwrap())
        };
        let assets = self.sce_vm.state().asset_mgr();
        let scene = if deferred {
            assets.load_scn_deferred(&name, &sub_name)
        } else {
            assets.load_scn(&name, &sub_name)
        };
        self.scene_manager.unload_all_scenes();
        self.scene_manager.push_scene(scene);
    }

    fn restore_scene(&mut self) {
        let component = self.scene_manager.scn_scene().unwrap();
        let scn = component.inner::<crate::openpal3::scene::ScnScene>();
        let global_state = self.sce_vm.global_state_mut();
        scn.restore_mechanisms(&global_state.persistent_state());
        let role_entity = scn
            .get_role_entity(global_state.role_controlled())
            .unwrap_or_else(|| {
                global_state.set_role_controlled(0);
                scn.get_role_entity(0).unwrap()
            });
        let role = RoleController::get_role_controller(role_entity.clone()).unwrap();
        role.inner::<RoleController>().set_active(true);
        if global_state.asset_mgr().game() == crate::GameType::PAL3 {
            role.inner::<RoleController>().set_nav_layer(
                global_state
                    .persistent_state()
                    .nav_layer()
                    .min(scn.nav().layer_count().saturating_sub(1)),
            );
        }
        role_entity
            .transform()
            .borrow_mut()
            .set_position(&global_state.persistent_state_mut().position());
        global_state.play_default_bgm();
    }

    fn play_mechanism_sound(&mut self, sound: &str) {
        if let Ok(data) = self.sce_vm.state().asset_mgr().load_snd_data(sound) {
            let mut source = self.sce_vm.global_state().create_sound_source();
            source.set_data(data, radiance::audio::Codec::Wav);
            source.play(false);
            self.sce_vm
                .global_state_mut()
                .add_sound_source(Rc::new(RefCell::new(source)));
        }
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
        let was_loading = self.sce_vm.is_loading();
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
        let freeze = (adv_enabled && menu_open) || was_loading || self.sce_vm.is_loading();
        if let Some(scene) = self.scene_manager.scene() {
            scene.set_active(!freeze);
        }

        if freeze {
            return None;
        }

        if let Some(scene) = self.scene_manager.scn_scene() {
            let scene = scene.inner::<crate::openpal3::scene::ScnScene>();
            let player = self
                .scene_manager
                .get_resolved_role(self.sce_vm.state(), -1)
                .and_then(|entity| {
                    let controller = RoleController::get_role_controller(entity.clone())?;
                    Some((entity, controller.inner::<RoleController>().nav_layer()))
                });
            scene.set_mechanism_player(player.clone());
            scene.set_mechanism_script_running(self.sce_vm.state().context().is_running());
            scene.set_mechanism_party_count(
                self.sce_vm
                    .global_state()
                    .persistent_state()
                    .party()
                    .members()
                    .count(),
            );
            let procs = scene.update_mechanisms(delta_sec);
            let hazard = scene.update_hazard_damage(delta_sec, &mut self.hazard_cooldown);
            let native_pal3 = self.sce_vm.state().asset_mgr().game() == crate::GameType::PAL3;
            let rewards = scene.take_pickup_rewards();
            let chest_rewards = scene.take_chest_rewards();
            let notices = scene.take_mechanism_notices();
            let mut reward_notices = vec![];
            let mut reward_sound_count = 0;
            let stats_data = (!rewards.is_empty() || hazard.is_some())
                .then(|| self.sce_vm.state().asset_mgr().stats_data());
            let mut damage_delta = 0;
            {
                let state = self.sce_vm.global_state_mut();
                let mut persistent = state.persistent_state_mut();
                if let Some(fraction) = hazard {
                    let party = persistent.party().clone();
                    let role = persistent.role_controlled();
                    let deltas = persistent.stats_mut().hazard_damage(
                        &party,
                        fraction,
                        stats_data.as_ref().unwrap(),
                    );
                    damage_delta = deltas.get(role as usize).copied().unwrap_or(0);
                }
                if let Some(data) = &stats_data {
                    (reward_notices, reward_sound_count) =
                        apply_pickup_rewards(&mut persistent, data, rewards);
                }
                for reward in chest_rewards {
                    if reward.experience != 0 {
                        persistent
                            .stats_mut()
                            .add_experience(reward.role, reward.experience);
                        reward_notices.push(format!("经验增加{}", reward.experience));
                    }
                    persistent.add_favor(reward.role, reward.favor);
                    log::info!("PAL3 chest reward: {reward:?}");
                }
                scene.snapshot_mechanisms(&mut persistent);
                if let Some((entity, _)) = player.filter(|_| native_pal3) {
                    persistent.set_position(entity.transform().borrow().position());
                    if let Some(controller) = RoleController::get_role_controller(entity) {
                        persistent.set_nav_layer(controller.inner::<RoleController>().nav_layer());
                    }
                }
            }
            if !notices.is_empty() {
                self.sce_vm
                    .state_mut()
                    .show_mechanism_notice(notices.join("\n"));
            }
            self.sce_vm.state_mut().show_reward_notices(reward_notices);
            if damage_delta < 0 {
                self.sce_vm.state_mut().show_hazard_damage(damage_delta);
                let role = self.sce_vm.global_state().role_controlled();
                if let Some(sound) =
                    ["we015", "we016", "we017", "we018", "we019"].get(role as usize)
                {
                    self.play_mechanism_sound(sound);
                }
            }
            for sound in scene.take_mechanism_sounds() {
                self.play_mechanism_sound(&sound);
            }
            for _ in 0..reward_sound_count {
                self.play_mechanism_sound("wa006");
            }
            for proc_id in procs {
                self.sce_vm.call_proc(proc_id);
            }
            if scene.mechanism_input_locked() {
                return None;
            }
        }
        if !adv_enabled {
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
                    .map(Some)
                    .or_else(|| scene.interact_mechanism(nav_layer, &position))
                    .unwrap_or_else(|| {
                        scene.test_item_trigger(&position).or_else(|| {
                            scene.test_role_trigger(
                                &position,
                                self.sce_vm.global_state().role_controlled(),
                            )
                        })
                    })
            };
            if let Some(proc_id) = trigger_proc_id {
                debug!("New proc triggerd: {}", proc_id);
                self.sce_vm.call_proc(proc_id);
            }
            if scene_rc
                .inner::<crate::openpal3::scene::ScnScene>()
                .mechanism_input_locked()
            {
                return None;
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
