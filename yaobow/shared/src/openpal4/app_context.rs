use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crosscom::ComRc;
use fileformats::cam::CameraDataFile;
use radiance::{
    audio::AudioEngine,
    comdef::{IEntity, ISceneManager},
    input::InputEngine,
    math::Vec3,
    radiance::{TaskHandle, TaskManager, UiManager},
    rendering::{ComponentFactory, VideoPlayer},
    utils::{act_drop::ActDrop, interp_value::InterpValue},
};

use crate::ui::dialog_box::{AvatarPosition, DialogBox};

use super::{
    actor::{Pal4ActorAnimation, Pal4ActorAnimationConfig},
    asset_loader::AssetLoader,
    comdef::IPal4ActorController,
    scene::Pal4Scene,
};

pub struct Pal4AppContext {
    pub(crate) loader: Rc<AssetLoader>,
    pub(crate) scene_manager: ComRc<ISceneManager>,
    pub(crate) ui: Rc<UiManager>,
    pub(crate) input: Rc<RefCell<dyn InputEngine>>,
    pub(crate) task_manager: Rc<TaskManager>,
    pub(crate) scene: Pal4Scene,
    pub(crate) dialog_box: DialogBox,

    component_factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
    video_player: Box<VideoPlayer>,
    bgm_task: Option<Rc<TaskHandle>>,
    sound_tasks: HashMap<i32, Rc<TaskHandle>>,
    sound_id: i32,
    actdrop: ActDrop,
    voice_task: Option<Rc<TaskHandle>>,
    camera_data: Option<CameraDataFile>,
    scene_name: String,
    block_name: String,
    leader: usize,
    player_locked: bool,

    moving_entities: HashMap<ActorId, MovingEntity>,
}

impl Pal4AppContext {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
        input: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        task_manager: Rc<TaskManager>,
    ) -> Self {
        Self {
            loader,
            scene_manager,
            ui: ui.clone(),
            task_manager,
            input,
            component_factory: component_factory.clone(),
            audio_engine,
            video_player: component_factory.create_video_player(),
            bgm_task: None,
            sound_tasks: HashMap::new(),
            sound_id: 0,
            actdrop: ActDrop::new(),
            voice_task: None,
            camera_data: None,
            scene_name: String::new(),
            block_name: String::new(),
            leader: 0,
            scene: Pal4Scene::new_empty(),
            dialog_box: DialogBox::new(ui),
            player_locked: true,
            moving_entities: HashMap::new(),
        }
    }

    pub fn update(&mut self, delta_sec: f32) {
        self.actdrop.update(self.ui.ui(), delta_sec);
        self.update_moving_entities(delta_sec);
    }

    fn update_moving_entities(&mut self, delta_sec: f32) {
        let moving_entities = std::mem::take(&mut self.moving_entities);
        self.moving_entities = self.update_moving_entities_(moving_entities, delta_sec);
    }

    fn update_moving_entities_(
        &mut self,
        mut entities: HashMap<ActorId, MovingEntity>,
        delta_sec: f32,
    ) -> HashMap<ActorId, MovingEntity> {
        let mut to_remove = Vec::new();

        const RUN_SPEED: f32 = 150.;
        const WALK_SPEED: f32 = 75.;

        for (id, entity) in entities.iter() {
            let pos = entity.entity.transform().borrow().position();
            let target = entity.target;
            let speed = if entity.run { RUN_SPEED } else { WALK_SPEED };

            let moving_distance = speed * delta_sec;
            let diff = Vec3::sub(&target, &pos);
            let distance = diff.norm();
            if distance < moving_distance {
                entity.entity.transform().borrow_mut().set_position(&target);
                to_remove.push(id.clone());
            } else {
                let direction = Vec3::normalized(&diff);
                let new_pos = Vec3::add(&pos, &Vec3::scalar_mul(moving_distance, &direction));
                let look_at = Vec3::new(pos.x, new_pos.y, pos.z);
                entity
                    .entity
                    .transform()
                    .borrow_mut()
                    .set_position(&new_pos)
                    .look_at(&look_at);
            }
        }

        for id in to_remove {
            match &id {
                ActorId::Player(player) => {
                    self.player_play_animation(*player as i32, Pal4ActorAnimation::Idle);
                }
                ActorId::Npc(name) => {
                    self.npc_play_animation(name, Pal4ActorAnimation::Idle);
                }
            }

            entities.remove(&id);
        }

        entities
    }

    pub fn try_trigger_scene_events(&mut self, _delta_sec: f32) -> Option<String> {
        self.scene
            .test_event_triggers(self.leader)
            .and_then(|event| event.function.function.as_str().ok())
    }

    pub fn set_actdrop(&mut self, darkness: InterpValue<f32>) {
        self.actdrop.set_darkness(darkness);
    }

    pub fn get_actdrop(&self) -> &ActDrop {
        &self.actdrop
    }

    pub fn set_leader(&mut self, leader: i32) {
        self.leader = leader as usize;
        self.scene.get_player(self.leader).set_visible(true);
    }

    pub fn set_player_pos(&mut self, player: i32, pos: &Vec3) {
        let player = self.map_player(player);
        self.scene.get_player(player).set_visible(true);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .set_position(&pos);
    }

    pub fn get_player_pos(&mut self, player: i32) -> Vec3 {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow()
            .position()
    }

    pub fn player_to(&mut self, player: i32, target: &Vec3, run: bool) {
        let mapped_player = self.map_player(player);
        let entity = self.scene.get_player(mapped_player);

        let moving_entity = MovingEntity {
            entity,
            target: target.clone(),
            run,
        };

        let animation = if run {
            Pal4ActorAnimation::Run
        } else {
            Pal4ActorAnimation::Walk
        };

        self.player_play_animation(player, animation);
        self.moving_entities
            .insert(ActorId::Player(mapped_player), moving_entity);
    }

    pub fn player_moving(&mut self, player: i32) -> bool {
        let player = self.map_player(player);
        self.moving_entities.contains_key(&ActorId::Player(player))
    }

    pub fn npc_to(&mut self, name: &str, target: &Vec3, run: bool) {
        let entity = self.scene.get_npc(name);
        if entity.is_none() {
            return;
        }

        let moving_entity = MovingEntity {
            entity: entity.unwrap(),
            target: target.clone(),
            run,
        };

        let animation = if run {
            Pal4ActorAnimation::Run
        } else {
            Pal4ActorAnimation::Walk
        };

        self.npc_play_animation(name, animation);
        self.moving_entities
            .insert(ActorId::Npc(name.to_string()), moving_entity);
    }

    pub fn npc_moving(&mut self, name: &str) -> bool {
        self.moving_entities
            .contains_key(&ActorId::Npc(name.to_string()))
    }

    pub fn player_lookat(&mut self, player: i32, target: &Vec3) {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .look_at(target);
    }

    pub fn lock_player(&mut self, lock: bool) {
        self.player_locked = lock;
        self.scene
            .get_player(self.leader)
            .get_component(IPal4ActorController::uuid())
            .unwrap()
            .query_interface::<IPal4ActorController>()
            .unwrap()
            .lock_control(lock);
    }

    pub fn set_player_ang(&mut self, player: i32, ang: f32) {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .clear_rotation()
            .rotate_axis_angle_local(&Vec3::UP, ang.to_radians());
    }

    pub fn player_do_action(&mut self, player: i32, action: &str, flag: i32) {
        let player = self.map_player(player);
        let metadata = self.scene.get_player_metadata(player);
        let anm = self.loader.load_anm(metadata.actor_name(), action).unwrap();
        let events = self.loader.load_amf(metadata.actor_name(), action);

        let config = match flag {
            -1 => Pal4ActorAnimationConfig::PauseOnHold,
            0 => Pal4ActorAnimationConfig::Looping,

            // TODO: >0 means playing n times
            _ => Pal4ActorAnimationConfig::OneTime,
        };

        self.scene
            .get_player_controller(player)
            .play_animation(anm, events, config);
    }

    pub fn player_play_animation(&mut self, player: i32, animation: Pal4ActorAnimation) {
        let player = self.map_player(player);
        self.scene
            .get_player_controller(player)
            .play(animation, Pal4ActorAnimationConfig::Looping);
    }

    pub fn npc_play_animation(&mut self, name: &str, animation: Pal4ActorAnimation) {
        self.scene
            .get_npc_controller(name)
            .map(|controller| controller.play(animation, Pal4ActorAnimationConfig::Looping));
    }

    pub fn player_unhold_act(&mut self, player: i32) {
        let player = self.map_player(player);
        self.scene.get_player_controller(player).unhold();
    }

    pub fn player_act_completed(&mut self, player: i32) -> bool {
        let player = self.map_player(player);
        self.scene
            .get_player_controller(player)
            .animation_completed()
    }

    pub fn player_set_direction(&mut self, player: i32, direction: f32) {
        let player = self.map_player(player);
        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .clear_rotation()
            .rotate_axis_angle_local(&Vec3::UP, direction * std::f32::consts::PI / 180.0);
    }

    pub fn load_scene(&mut self, scene_name: &str, block_name: &str) {
        let _ = self.scene_manager.pop_scene();
        self.scene =
            Pal4Scene::load(&self.loader, self.input.clone(), scene_name, block_name).unwrap();
        self.set_leader(self.leader as i32);
        self.scene_manager.push_scene(self.scene.scene.clone());

        self.scene_name = scene_name.to_string();
        self.block_name = block_name.to_string();
    }

    pub fn start_play_movie(&mut self, name: &str) -> Option<(u32, u32)> {
        let reader = self.loader.load_video(name).unwrap();
        self.video_player.play(
            self.component_factory.clone(),
            self.audio_engine.clone(),
            reader,
            radiance::video::Codec::Bik,
            false,
        )
    }

    pub fn video_player(&mut self) -> &mut VideoPlayer {
        &mut self.video_player
    }

    pub fn play_bgm(&mut self, name: &str) -> anyhow::Result<()> {
        self.stop_bgm();

        let data = self.loader.load_music(name)?;
        let mut source = self.audio_engine.create_source();
        source.set_data(data, radiance::audio::Codec::Mp3);
        source.play(true);

        self.bgm_task = Some(self.task_manager.run_generic(move |_| {
            source.update();
            false
        }));

        Ok(())
    }

    pub fn stop_bgm(&mut self) {
        if let Some(task) = &self.bgm_task {
            task.stop();
        }
    }

    pub fn play_sound(&mut self, name: &str) -> anyhow::Result<i32> {
        self.sound_tasks.retain(|_, v| !v.is_finished());

        let id = self.find_next_sound_id();
        let task = self.play_sound_internal(name, radiance::audio::Codec::Wav)?;
        self.sound_tasks.insert(id, task);
        Ok(id)
    }

    pub fn stop_sound(&mut self, id: i32) {
        if let Some(task) = self.sound_tasks.remove(&id) {
            task.stop();
        }
    }

    pub fn play_voice(&mut self, name: &str) -> anyhow::Result<()> {
        self.stop_voice();

        let task = self.play_sound_internal(name, radiance::audio::Codec::Mp3)?;
        self.voice_task = Some(task);
        Ok(())
    }

    pub fn stop_voice(&mut self) {
        if let Some(task) = &self.voice_task {
            task.stop();
        }
    }

    pub fn prepare_camera(&mut self, name: &str) -> anyhow::Result<()> {
        let data = self
            .loader
            .load_camera_data(name, &self.scene_name, &self.block_name)?;
        self.camera_data = Some(data);
        Ok(())
    }

    pub fn run_camera(&mut self, name: &str) {
        log::debug!("run_camera: {}", name);
        if let Some(data) = &self.camera_data {
            let camera_data = data.get_camera_data(name);
            if let Some(camera_data) = camera_data {
                let position = camera_data.get_position();
                let look_at = camera_data.get_look_at();
                log::debug!("camera_data: {:?} {:?}", position, look_at);
                // if camera_data.is_instant() {
                let camera = self.scene_manager.scene().unwrap().camera();
                let mut camera = camera.borrow_mut();
                camera
                    .transform_mut()
                    .set_position(&Vec3::new(position[0], position[1], position[2]))
                    .look_at(&Vec3::new(look_at[0], look_at[1], look_at[2]));
                // } else {

                // }
            }
        }
    }

    pub fn set_portrait(&mut self, name: &str, left: bool) {
        let image = self.loader.load_portrait(name);
        self.dialog_box.set_avatar(
            image,
            if left {
                AvatarPosition::Left
            } else {
                AvatarPosition::Right
            },
        );
    }

    fn find_next_sound_id(&mut self) -> i32 {
        while self.sound_tasks.contains_key(&self.sound_id) {
            self.sound_id += 1;
            if self.sound_id == 10000 {
                self.sound_id = 0;
            }
        }

        self.sound_id
    }

    fn play_sound_internal(
        &mut self,
        name: &str,
        codec: radiance::audio::Codec,
    ) -> anyhow::Result<Rc<TaskHandle>> {
        let ext = if codec == radiance::audio::Codec::Mp3 {
            "mp3"
        } else {
            "wav"
        };

        let data = self.loader.load_sound(name, ext)?;
        let mut source = self.audio_engine.create_source();
        source.set_data(data, codec);
        source.play(false);

        let task = self.task_manager.run_generic(move |_| {
            source.update();
            source.state() == radiance::audio::AudioSourceState::Stopped
        });

        Ok(task)
    }

    #[inline]
    fn map_player(&self, player: i32) -> usize {
        if player == -1 {
            self.leader
        } else {
            player as usize
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum ActorId {
    Player(usize),
    Npc(String),
}

struct MovingEntity {
    entity: ComRc<IEntity>,
    target: Vec3,
    run: bool,
}
