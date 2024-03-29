use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crosscom::ComRc;
use fileformats::cam::CameraDataFile;
use radiance::{
    audio::AudioEngine,
    comdef::ISceneManager,
    input::InputEngine,
    math::Vec3,
    radiance::{TaskHandle, TaskManager, UiManager},
    rendering::{ComponentFactory, VideoPlayer},
};

use crate::ui::dialog_box::{AvatarPosition, DialogBox};

use super::{actor::Pal4ActorAnimationConfig, asset_loader::AssetLoader, scene::Pal4Scene};

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
    voice_task: Option<Rc<TaskHandle>>,
    camera_data: Option<CameraDataFile>,
    scene_name: String,
    block_name: String,
    leader: usize,
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
            voice_task: None,
            camera_data: None,
            scene_name: String::new(),
            block_name: String::new(),
            leader: 0,
            scene: Pal4Scene::new_empty(),
            dialog_box: DialogBox::new(ui),
        }
    }

    pub fn set_leader(&mut self, leader: i32) {
        self.leader = leader as usize;
        self.scene.get_player(self.leader).set_visible(true);
    }

    pub fn set_player_pos(&mut self, player: i32, pos: &Vec3) {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .set_position(&pos);
    }

    pub fn set_player_ang(&mut self, player: i32, ang: f32) {
        let player = self.map_player(player);

        self.scene
            .get_player(player)
            .transform()
            .borrow_mut()
            .clear_rotation()
            .rotate_axis_angle_local(&Vec3::UP, ang * std::f32::consts::PI / 180.0);
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
        let scene = Pal4Scene::load(&self.loader, scene_name, block_name).unwrap();
        self.scene_manager.push_scene(scene.scene.clone());

        self.scene = scene;
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
