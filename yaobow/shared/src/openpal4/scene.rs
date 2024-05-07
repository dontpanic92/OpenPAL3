use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crosscom::ComRc;
use fileformats::pal4::evf::EvfEvent;
use radiance::{
    comdef::{IEntity, IScene, IStaticMeshComponent},
    input::InputEngine,
    math::{Transform, Vec3},
    scene::{CoreEntity, CoreScene},
    utils::ray_casting::RayCaster,
};

use crate::scripting::angelscript::ScriptModule;

use super::{
    actor::Pal4ActorController,
    asset_loader::{self, AssetLoader},
    comdef::{IPal4ActorAnimationController, IPal4ActorController},
};

pub enum Player {
    YunTianhe,
    HanLingsha,
    LiuMengli,
    MurongZiying,
}

impl Player {
    pub fn name(&self) -> &str {
        match self {
            Player::YunTianhe => "YunTianhe",
            Player::HanLingsha => "HanLingsha",
            Player::LiuMengli => "LiuMengli",
            Player::MurongZiying => "MurongZiying",
        }
    }

    pub fn actor_name(&self) -> &str {
        match self {
            Player::YunTianhe => "101",
            Player::HanLingsha => "103",
            Player::LiuMengli => "106",
            Player::MurongZiying => "105",
        }
    }
}

pub struct Pal4Scene {
    pub(crate) scene: ComRc<IScene>,
    pub(crate) players: [ComRc<IEntity>; 4],
    pub(crate) npcs: Vec<ComRc<IEntity>>,
    pub(crate) events: Vec<EvfEvent>,
    pub(crate) module: Option<Rc<RefCell<ScriptModule>>>,
    pub(crate) triggers: Vec<Rc<SceneEventTrigger>>,
}

impl Pal4Scene {
    const ID_YUN_TIANHE: usize = 0;
    const ID_HAN_LINGSHA: usize = 1;
    const ID_LIU_MENGLI: usize = 2;
    const ID_MURONG_ZIYING: usize = 3;

    pub fn new_empty() -> Self {
        Self {
            scene: CoreScene::create(),
            players: [
                CoreEntity::create("".to_string(), false),
                CoreEntity::create("".to_string(), false),
                CoreEntity::create("".to_string(), false),
                CoreEntity::create("".to_string(), false),
            ],
            npcs: vec![],
            events: vec![],
            module: None,
            triggers: vec![],
        }
    }

    pub fn load(
        asset_loader: &Rc<asset_loader::AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_name: &str,
        block_name: &str,
    ) -> anyhow::Result<Self> {
        let scene = asset_loader.load_scene(scene_name, block_name)?;
        let clip = asset_loader.try_load_scene_clip(scene_name, block_name);
        if let Some(clip) = clip {
            scene.add_entity(clip);
        }

        let clip_na = asset_loader.try_load_scene_clip_na(scene_name, block_name);
        if let Some(clip_na) = clip_na {
            scene.add_entity(clip_na);
        }

        let skybox = asset_loader.try_load_scene_sky(scene_name, block_name);
        if let Some(skybox) = skybox {
            scene.add_entity(skybox);
        }

        scene.camera().borrow_mut().set_fov43(45_f32.to_radians());

        let floor = asset_loader.load_scene_floor(scene_name, block_name);
        let wall = asset_loader.load_scene_wall(scene_name, block_name);
        let ray_caster = create_floor_wall_ray_caster(floor.clone(), wall.clone());

        /*if let Some(floor) = floor {
            scene.add_entity(floor);
        }

        if let Some(wall) = wall {
            scene.add_entity(wall);
        }*/

        let players = [
            load_player(asset_loader, Player::YunTianhe),
            load_player(asset_loader, Player::HanLingsha),
            load_player(asset_loader, Player::LiuMengli),
            load_player(asset_loader, Player::MurongZiying),
        ];

        let events = asset_loader.load_evf(scene_name, block_name)?;

        let mut triggers = vec![];
        for (i, event) in events.events.iter().enumerate() {
            if event.vertex_count != 8 {
                continue;
            }

            let trigger = event
                .vertices
                .iter()
                .map(|trigger| {
                    Vec3::new(
                        trigger.center.x as f32,
                        trigger.center.y as f32,
                        trigger.center.z as f32,
                    )
                })
                .collect::<Vec<_>>();

            // let entity = create_box_entity2(asset_loader.component_factory(), trigger.clone());
            // scene.add_entity(entity);

            let ray_caster = create_trigger_ray_caster(trigger);
            triggers.push(Rc::new(SceneEventTrigger {
                ray_caster,
                event_index: i,
                triggered: Cell::new(false),
            }));
        }

        let controller = Pal4ActorController::create(
            input,
            players[0].clone(),
            scene.clone(),
            triggers.clone(),
            ray_caster,
        );

        players[0].add_component(IPal4ActorController::uuid(), ComRc::from_object(controller));

        for p in &players {
            scene.add_entity(p.clone());
        }

        let npc_info = asset_loader.load_npc_info(scene_name, block_name)?;
        let mut npcs = vec![];
        for npc in &npc_info.data {
            let actor_name = npc.model_name.as_str();
            match actor_name {
                Ok(actor_name) => {
                    let entity = asset_loader.load_actor(
                        npc.name.as_str().unwrap_or_default().as_str(),
                        actor_name.as_str(),
                        npc.get_default_act().as_deref(),
                    );

                    if let Ok(entity) = entity {
                        entity.set_visible(npc.default_visible == 1);
                        entity.set_enabled(npc.default_visible == 1);
                        entity
                            .transform()
                            .borrow_mut()
                            .set_position(&Vec3::new_zeros())
                            .rotate_axis_angle(&Vec3::BACK, npc.rotation[2].to_radians())
                            .rotate_axis_angle(&Vec3::UP, npc.rotation[1].to_radians())
                            .rotate_axis_angle(&Vec3::EAST, npc.rotation[0].to_radians())
                            .set_position(&Vec3::from(npc.position));

                        npcs.push(entity.clone());

                        scene.add_entity(entity);
                    }
                }
                Err(e) => {
                    log::error!("Cannot load actor: {}", e)
                }
            }
        }

        let module = asset_loader.load_script_module(scene_name)?;

        Ok(Self {
            scene,
            players,
            npcs,
            events: events.events,
            module: Some(module),
            triggers,
        })
    }

    pub fn get_player(&self, player_id: usize) -> ComRc<IEntity> {
        self.players[player_id].clone()
    }

    pub fn get_npc(&self, name: &str) -> Option<ComRc<IEntity>> {
        self.npcs.iter().find(|npc| name == npc.name()).cloned()
    }

    pub fn get_player_controller(&self, player_id: usize) -> ComRc<IPal4ActorAnimationController> {
        self.players[player_id]
            .get_component(IPal4ActorAnimationController::uuid())
            .unwrap()
            .query_interface::<IPal4ActorAnimationController>()
            .unwrap()
    }

    pub fn get_npc_controller(&self, name: &str) -> Option<ComRc<IPal4ActorAnimationController>> {
        self.get_npc(name)?
            .get_component(IPal4ActorAnimationController::uuid())?
            .query_interface::<IPal4ActorAnimationController>()
    }

    /*pub fn test_event_triggers(&self, player_id: usize) -> Option<&EvfEvent> {
        let player = self.get_player(player_id);
        let position = player.world_transform().position();
        let mut min_distance = 99999.;

        for event in &self.events {
            for trigger in &event.triggers {
                let center = Vec3::new(
                    trigger.center.x as f32,
                    trigger.center.y as f32,
                    trigger.center.z as f32,
                );
                let distance = Vec3::sub(&position, &center).norm();
                if distance < min_distance {
                    min_distance = distance;
                }

                let half_size = Vec3::scalar_mul(
                    10.,
                    &Vec3::new(
                        trigger.half_size.x,
                        trigger.half_size.y,
                        trigger.half_size.z,
                    ),
                );
                let min = Vec3::sub(&center, &half_size);
                let max = Vec3::add(&center, &half_size);

                if min.x <= position.x
                    && min.y <= position.y
                    && min.z <= position.z
                    && max.x >= position.x
                    && max.y >= position.y
                    && max.z >= position.z
                {
                    return Some(event);
                }
            }
        }

        None
    }*/

    pub fn test_event_triggers(&self) -> Option<&EvfEvent> {
        for trigger in &self.triggers {
            if trigger.triggered.get() {
                return self.events.get(trigger.event_index);
            }
        }

        None
    }

    pub fn get_player_metadata(&self, player_id: usize) -> Player {
        match player_id {
            Self::ID_YUN_TIANHE => Player::YunTianhe,
            Self::ID_HAN_LINGSHA => Player::HanLingsha,
            Self::ID_LIU_MENGLI => Player::LiuMengli,
            Self::ID_MURONG_ZIYING => Player::MurongZiying,
            _ => unreachable!(),
        }
    }
}

fn load_player(asset_loader: &Rc<AssetLoader>, player: Player) -> ComRc<IEntity> {
    let entity = asset_loader
        .load_actor(player.name(), player.actor_name(), Some("C01"))
        .unwrap();

    entity.set_visible(false);
    entity.set_enabled(false);

    entity
}

fn create_floor_wall_ray_caster(
    floor: Option<ComRc<IEntity>>,
    wall: Option<ComRc<IEntity>>,
) -> RayCaster {
    let mut ray_caster = RayCaster::new();
    if let Some(floor) = floor {
        floor.update_world_transform(&Transform::new());
        add_mesh(&mut ray_caster, floor);
    }

    if let Some(wall) = wall {
        wall.update_world_transform(&Transform::new());
        add_mesh(&mut ray_caster, wall);
    }

    ray_caster
}

fn add_mesh(ray_caster: &mut RayCaster, entity: ComRc<IEntity>) {
    for child in entity.children() {
        add_mesh(ray_caster, child);
    }

    let mesh = entity.get_component(IStaticMeshComponent::uuid());
    if let Some(mesh) = mesh {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        let mesh = mesh.get();
        let geometries = mesh.get_geometries();
        let entity_position = entity.world_transform().position();

        for geometry in geometries {
            let v = geometry
                .vertices
                .to_position_vec()
                .into_iter()
                .map(|v| Vec3::add(&entity_position, &v))
                .collect();

            let i = geometry.indices.clone();
            ray_caster.add_mesh(v, i);
        }
    }
}

pub struct SceneEventTrigger {
    ray_caster: RayCaster,
    event_index: usize,
    triggered: Cell<bool>,
}

impl SceneEventTrigger {
    pub fn check(&self, origin: &Vec3, direction: &Vec3) {
        let hit = self.ray_caster.cast_ray(origin, direction);
        let distance = Vec3::norm(direction);

        self.triggered.set(false);
        if let Some(p) = hit {
            if p < distance {
                self.triggered.set(true);
            }
        }
    }
}

fn create_trigger_ray_caster(trigger: Vec<Vec3>) -> RayCaster {
    let mut ray_caster = RayCaster::new();
    ray_caster.add_mesh(
        trigger,
        vec![
            0, 2, 1, 0, 3, 2, 0, 4, 7, 0, 7, 3, 0, 5, 4, 0, 1, 5, 6, 1, 2, 6, 5, 1, 6, 2, 3, 6, 3,
            7, 6, 7, 4, 6, 4, 5,
        ],
    );

    ray_caster
}
