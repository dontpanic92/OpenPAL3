use std::{cell::RefCell, io::Cursor, rc::Rc};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use fileformats::{binrw::BinRead, cam::CameraDataFile, npc::NpcInfoFile};
use mini_fs::MiniFs;
use radiance::{
    comdef::{IArmatureComponent, IEntity, IScene},
    components::mesh::skinned_mesh::AnimKeyFrame,
    rendering::ComponentFactory,
    scene::CoreScene,
};

use crate::{
    loaders::{
        anm::load_anm, bsp::create_entity_from_bsp_model, dff::create_entity_from_dff_model,
        smp::load_smp, Pal4TextureResolver,
    },
    scripting::angelscript::ScriptModule,
};

use super::actor::Pal4Actor;

pub struct AssetLoader {
    vfs: MiniFs,
    component_factory: Rc<dyn ComponentFactory>,
    texture_resolver: Pal4TextureResolver,
}

impl AssetLoader {
    pub fn new(component_factory: Rc<dyn ComponentFactory>, vfs: MiniFs) -> Rc<Self> {
        Rc::new(Self {
            component_factory,
            vfs,
            texture_resolver: Pal4TextureResolver {},
        })
    }

    pub fn load_script_module(&self, scene: &str) -> anyhow::Result<Rc<RefCell<ScriptModule>>> {
        let content = self
            .vfs
            .read_to_end(&format!("/gamedata/script/{}.csb", scene))?;
        Ok(Rc::new(RefCell::new(
            ScriptModule::read_from_buffer(&content).unwrap(),
        )))
    }

    pub fn load_actor(
        &self,
        entity_name: &str,
        actor_name: &str,
        default_act: Option<&str>,
    ) -> anyhow::Result<ComRc<IEntity>> {
        let model_path = format!("/gamedata/PALActor/{}/{}.dff", actor_name, actor_name);

        let entity = create_entity_from_dff_model(
            &self.component_factory,
            &self.vfs,
            model_path,
            entity_name.to_string(),
            true,
            &self.texture_resolver,
        );

        if let Some(default_act) = default_act {
            let act_path = format!("/gamedata/PALActor/{}/{}.anm", actor_name, default_act);
            let anm = load_anm(&self.vfs, &act_path)?;
            Pal4Actor::set_anim(entity.clone(), &anm);
        }

        Ok(entity)
    }

    pub fn load_anm(
        &self,
        actor_name: &str,
        act_name: &str,
    ) -> anyhow::Result<Vec<Vec<AnimKeyFrame>>> {
        let act_path = format!("/gamedata/PALActor/{}/{}.anm", actor_name, act_name);
        Ok(load_anm(&self.vfs, &act_path)?)
    }

    pub fn load_scene(&self, scene_name: &str, block_name: &str) -> anyhow::Result<ComRc<IScene>> {
        let path = format!(
            "/gamedata/PALWorld/{}/{}/{}.bsp",
            scene_name, block_name, block_name,
        );

        let scene = CoreScene::create();
        let entity = create_entity_from_bsp_model(
            &self.component_factory,
            &self.vfs,
            path,
            "world".to_string(),
            &self.texture_resolver,
        );

        scene.add_entity(entity);
        Ok(scene)
    }

    pub fn load_npc_info(&self, scene_name: &str, block_name: &str) -> anyhow::Result<NpcInfoFile> {
        let path = format!(
            "/gamedata/scenedata/{}/{}/npcInfo.npc",
            scene_name, block_name,
        );

        let data = self.vfs.read_to_end(&path)?;
        let mut cursor = Cursor::new(data);
        Ok(NpcInfoFile::read(&mut cursor)?)
    }

    pub fn load_video(&self, video_name: &str) -> anyhow::Result<Vec<u8>> {
        let video_folder = match video_name.to_lowercase().as_str() {
            "1a.bik" | "end2.bik" | "pal4a.bik" => "VideoA",
            _ => "videob",
        };

        let path = format!("/gamedata/{}/{}", video_folder, video_name);
        Ok(self.vfs.read_to_end(&path)?)
    }

    pub fn load_music(&self, music_name: &str) -> anyhow::Result<Vec<u8>> {
        let path = format!("/gamedata/Music/{}.smp", music_name);
        let data = load_smp(&self.vfs.read_to_end(path)?)?;
        Ok(data)
    }

    pub fn load_sound(&self, sound_name: &str, ext: &str) -> anyhow::Result<Vec<u8>> {
        let path = format!("/gamedata/PALSound/{}.{}", sound_name, ext);
        let data = self.vfs.read_to_end(path)?;
        Ok(data)
    }
    pub fn load_camera_data(
        &self,
        camera_data_name: &str,
        scene_name: &str,
        block_name: &str,
    ) -> anyhow::Result<CameraDataFile> {
        let path = format!(
            "/gamedata/scenedata/{}/{}/{}.cam",
            scene_name, block_name, camera_data_name,
        );
        let data = CameraDataFile::read(&mut Cursor::new(self.vfs.read_to_end(path)?))?;
        Ok(data)
    }
}
