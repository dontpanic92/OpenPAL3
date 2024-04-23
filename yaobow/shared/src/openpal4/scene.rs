use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use fileformats::evf::EvfEvent;
use radiance::{
    comdef::{IEntity, IScene, IStaticMeshComponent},
    debug::create_box_entity,
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

        let mut ray_caster = RayCaster::new();
        let floor = asset_loader.load_scene_floor(scene_name, block_name);
        let wall = asset_loader.load_scene_wall(scene_name, block_name);
        setup_ray_caster(&mut ray_caster, floor.clone(), wall.clone());

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

        let controller =
            Pal4ActorController::create(input, players[0].clone(), scene.clone(), ray_caster);
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

        let events = asset_loader.load_evf(scene_name, block_name)?;
        let module = asset_loader.load_script_module(scene_name)?;

        for event in &events.events {
            if event.trigger_count != 8 {
                continue;
            }
            for trigger in &event.triggers {
                let center = Vec3::new(
                    trigger.center.x as f32,
                    trigger.center.y as f32,
                    trigger.center.z as f32,
                );

                let entity = create_box_entity(asset_loader.component_factory());
                entity.transform().borrow_mut().set_position(&center);
                scene.add_entity(entity);
            }
        }

        Ok(Self {
            scene,
            players,
            npcs,
            events: events.events,
            module: Some(module),
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

    pub fn test_event_triggers(&self, player_id: usize) -> Option<&EvfEvent> {
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

fn setup_ray_caster(
    ray_caster: &mut RayCaster,
    floor: Option<ComRc<IEntity>>,
    wall: Option<ComRc<IEntity>>,
) {
    if let Some(floor) = floor {
        floor.update_world_transform(&Transform::new());
        add_mesh(ray_caster, floor);
    }

    if let Some(wall) = wall {
        wall.update_world_transform(&Transform::new());
        add_mesh(ray_caster, wall);
    }
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
