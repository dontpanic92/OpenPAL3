use crosscom::ComRc;
use radiance::{
    comdef::{IEntity, IScene},
    math::Vec3,
    scene::{CoreEntity, CoreScene},
};

use super::asset_loader::{self, AssetLoader};

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
        }
    }

    pub fn load(
        asset_loader: &asset_loader::AssetLoader,
        scene_name: &str,
        block_name: &str,
    ) -> anyhow::Result<Self> {
        let scene = asset_loader.load_scene(scene_name, block_name)?;
        scene.camera().borrow_mut().set_fov43(45_f32.to_radians());

        let players = [
            load_player(asset_loader, Player::YunTianhe),
            load_player(asset_loader, Player::HanLingsha),
            load_player(asset_loader, Player::LiuMengli),
            load_player(asset_loader, Player::MurongZiying),
        ];

        scene.add_entity(players[Self::ID_YUN_TIANHE].clone());
        for p in &players {
            //scene.add_entity(p.clone());
        }

        let npc_info = asset_loader.load_npc_info(scene_name, block_name)?;
        for npc in &npc_info.data {
            let actor_name = npc.model_name.as_str();
            match actor_name {
                Ok(actor_name) => {
                    let entity = asset_loader.load_actor(
                        npc.name.as_str().unwrap_or_default(),
                        actor_name,
                        npc.get_default_act().as_deref(),
                    );

                    if let Ok(entity) = entity {
                        entity
                            .transform()
                            .borrow_mut()
                            .set_position(&Vec3::from(npc.position));
                        scene.add_entity(entity);
                    }
                }
                Err(e) => {
                    log::error!("Cannot load actor: {}", e)
                }
            }
        }

        Ok(Self { scene, players })
    }

    pub fn get_player(&self, player_id: usize) -> ComRc<IEntity> {
        self.players[player_id].clone()
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

fn load_player(asset_loader: &AssetLoader, player: Player) -> ComRc<IEntity> {
    let entity = asset_loader
        .load_actor(player.name(), player.actor_name(), Some("C01"))
        .unwrap();

    entity.set_visible(false);

    entity
}
