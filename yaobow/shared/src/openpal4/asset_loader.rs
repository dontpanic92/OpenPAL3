use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufReader, Cursor},
    rc::Rc,
};

use anyhow::anyhow;
use common::store_ext::StoreExt2;
use crosscom::ComRc;
use fileformats::{
    binrw::BinRead,
    npc::NpcInfoFile,
    pal4::{cam::CameraDataFile, evf::EvfFile, gob::GobFile},
};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{IArmatureComponent, IComponent, IEntity, IScene},
    components::mesh::{event::AnimationEvent, skinned_mesh::AnimKeyFrame},
    input::InputEngine,
    rendering::{ComponentFactory, Sprite},
    scene::CoreScene,
    utils::SeekRead,
};

use crate::{
    loaders::{
        anm::{load_amf, load_anm},
        bsp::create_entity_from_bsp_model,
        dff::{create_entity_from_dff_model, DffLoaderConfig},
        smp::load_smp,
        Pal4TextureResolver,
    },
    scripting::angelscript::ScriptModule,
};

use super::{actor::Pal4ActorAnimationController, comdef::IPal4ActorAnimationController};

pub struct AssetLoader {
    vfs: MiniFs,
    component_factory: Rc<dyn ComponentFactory>,
    input: Rc<RefCell<dyn InputEngine>>,
    texture_resolver: Pal4TextureResolver,
    portraits: HashMap<String, ImageSetImage>,
}

impl AssetLoader {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        input: Rc<RefCell<dyn InputEngine>>,
        vfs: MiniFs,
    ) -> Rc<Self> {
        let portraits = load_portraits(&component_factory, &vfs);
        Rc::new(Self {
            component_factory,
            input,
            vfs,
            texture_resolver: Pal4TextureResolver {},
            portraits,
        })
    }

    pub fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        self.component_factory.clone()
    }

    pub fn vfs(&self) -> &MiniFs {
        &self.vfs
    }

    pub fn load_script_module(&self, scene: &str) -> anyhow::Result<Rc<RefCell<ScriptModule>>> {
        let content = self
            .vfs
            .read_to_end(&format!("/gamedata/script/{}.csb", scene))?;
        Ok(Rc::new(RefCell::new(
            ScriptModule::read_from_buffer(&content).unwrap(),
        )))
    }

    pub fn load_object(
        &self,
        object_name: &str,
        folder: &str,
        file_name: &str,
    ) -> Option<ComRc<IEntity>> {
        let path = format!("/{}{}.dff", folder, file_name);
        self.try_load_dff(path, object_name.to_string())
    }

    pub fn load_actor(
        self: &Rc<Self>,
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
            &DffLoaderConfig {
                texture_resolver: &self.texture_resolver,
                keep_right_to_render_only: true,
            },
        );

        let armature = entity
            .get_component(IArmatureComponent::uuid())
            .unwrap()
            .query_interface::<IArmatureComponent>()
            .unwrap();

        let controller =
            Pal4ActorAnimationController::create(actor_name.to_string(), self.clone(), armature);
        entity.add_component(
            IPal4ActorAnimationController::uuid(),
            controller.query_interface::<IComponent>().unwrap(),
        );

        if let Some(default_act) = default_act {
            let anm = self.load_anm(actor_name, default_act).unwrap_or(vec![]);
            let events = self.load_amf(actor_name, default_act);
            controller.set_default(anm, events);
            controller.play_default();
        }

        Ok(entity)
    }

    pub fn load_evf(&self, scene_name: &str, block_name: &str) -> anyhow::Result<EvfFile> {
        let path = format!(
            "/gamedata/scenedata/{}/{}/{}.evf",
            scene_name, block_name, block_name,
        );

        let mut reader = BufReader::new(self.vfs.open(&path)?);
        Ok(EvfFile::read(&mut reader)?)
    }

    pub fn load_run_animation(&self, actor_name: &str) -> anyhow::Result<Animation> {
        self.load_animation(actor_name, "C03")
            .or_else(|_| self.load_animation(actor_name, "C02"))
    }

    pub fn load_animation(&self, actor_name: &str, act_name: &str) -> anyhow::Result<Animation> {
        let keyframes = self.load_anm(actor_name, act_name)?;
        let events = self.load_amf(actor_name, act_name);
        Ok(Animation { keyframes, events })
    }

    pub fn load_anm(
        &self,
        actor_name: &str,
        act_name: &str,
    ) -> anyhow::Result<Vec<Vec<AnimKeyFrame>>> {
        let act_path = format!("/gamedata/PALActor/{}/{}.anm", actor_name, act_name);
        Ok(load_anm(&self.vfs, &act_path)?)
    }

    pub fn load_amf(&self, actor_name: &str, act_name: &str) -> Vec<AnimationEvent> {
        let amf_path = format!("/gamedata/PALActor/{}/{}.amf", actor_name, act_name);
        load_amf(&self.vfs, &amf_path).unwrap_or(vec![])
    }

    pub fn load_gob(&self, scene_name: &str, block_name: &str) -> anyhow::Result<GobFile> {
        let path = format!(
            "/gamedata/scenedata/{}/{}/GameObjs.gob",
            scene_name, block_name,
        );

        let mut reader = BufReader::new(self.vfs.open(&path)?);
        Ok(GobFile::read(&mut reader)?)
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
            &DffLoaderConfig {
                texture_resolver: &self.texture_resolver,
                keep_right_to_render_only: false,
            },
        );

        scene.add_entity(entity);
        Ok(scene)
    }

    pub fn try_load_scene_sky(&self, scene_name: &str, block_name: &str) -> Option<ComRc<IEntity>> {
        let path = format!(
            "/gamedata/PALWorld/{}/{}/{}_sky.dff",
            scene_name, block_name, block_name,
        );

        self.try_load_dff(path, "sky".to_string())
    }

    pub fn try_load_scene_clip(
        &self,
        scene_name: &str,
        block_name: &str,
    ) -> Option<ComRc<IEntity>> {
        let path = format!(
            "/gamedata/PALWorld/{}/{}/{}_clip.dff",
            scene_name, block_name, block_name,
        );

        self.try_load_dff(path, "clip".to_string())
    }

    pub fn try_load_scene_clip_na(
        &self,
        scene_name: &str,
        block_name: &str,
    ) -> Option<ComRc<IEntity>> {
        let path = format!(
            "/gamedata/PALWorld/{}/{}/{}_clipNA.dff",
            scene_name, block_name, block_name,
        );

        self.try_load_dff(path, "clipNA".to_string())
    }

    fn try_load_dff(&self, path: String, object_name: String) -> Option<ComRc<IEntity>> {
        if self.vfs.exists(&path) {
            let entity = create_entity_from_dff_model(
                &self.component_factory,
                &self.vfs,
                path.clone(),
                object_name,
                true,
                &DffLoaderConfig {
                    texture_resolver: &self.texture_resolver,
                    keep_right_to_render_only: false,
                },
            );

            println!("Loaded dff: {}", path);
            Some(entity)
        } else {
            println!("Not Loaded dff: {}", path);
            None
        }
    }

    pub fn load_scene_floor(&self, scene_name: &str, block_name: &str) -> Option<ComRc<IEntity>> {
        let path = format!(
            "/gamedata/scenedata/{}/{}/{}_floor.dff",
            scene_name, block_name, block_name,
        );

        self.try_load_dff(path, "floor".to_string())
    }

    pub fn load_scene_wall(&self, scene_name: &str, block_name: &str) -> Option<ComRc<IEntity>> {
        let path = format!(
            "/gamedata/scenedata/{}/{}/{}_wall.dff",
            scene_name, block_name, block_name,
        );

        self.try_load_dff(path, "wall".to_string())
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

    pub fn load_video(&self, video_name: &str) -> anyhow::Result<Box<dyn SeekRead>> {
        let video_folder = match video_name.to_lowercase().as_str() {
            "1a.bik" | "end2.bik" | "pal4a.bik" => "VideoA",
            _ => "videob",
        };

        let path = format!("/gamedata/{}/{}", video_folder, video_name);
        Ok(Box::new(BufReader::new(self.vfs.open(&path)?)))
    }

    pub fn load_music(&self, music_name: &str) -> anyhow::Result<Vec<u8>> {
        let path = format!("/gamedata/Music/{}.smp", music_name);
        let data = load_smp(self.vfs.read_to_end(path)?)?;
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

    pub fn load_portrait(&self, name: &str) -> Option<ImageSetImage> {
        self.portraits.get(&name.to_lowercase()).cloned()
    }
}

#[derive(Clone)]
pub struct ImageSetImage {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub sprite: Rc<Sprite>,
}

fn load_portraits(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
) -> HashMap<String, ImageSetImage> {
    let portrait_files = [
        "/gamedata/ui/portrait/portrait0_0.imageset",
        "/gamedata/ui/portrait/portrait1_0.imageset",
        "/gamedata/ui/portrait/portrait2_0.imageset",
        "/gamedata/ui/portrait/portrait3_0.imageset",
        "/gamedata/ui/portrait/portrait4_0.imageset",
        "/gamedata/ui/portrait/portrait5_0.imageset",
        "/gamedata/ui/portrait/portrait6_0.imageset",
        "/gamedata/ui/portrait/portrait7_0.imageset",
        "/gamedata/ui/portrait/portrait8_0.imageset",
    ];

    let mut portraits = HashMap::new();
    let mut sprite_cache = HashMap::new();
    for portrait_file in &portrait_files {
        let ret = load_portraits_single(
            component_factory,
            vfs,
            portrait_file,
            &mut portraits,
            &mut sprite_cache,
        );
        if let Err(e) = ret {
            log::error!("load_portraits_single failed: {:?}", e);
        }
    }

    portraits
}

fn load_portraits_single(
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    imageset: &str,
    portraits: &mut HashMap<String, ImageSetImage>,
    sprite_cache: &mut HashMap<String, Rc<Sprite>>,
) -> anyhow::Result<()> {
    let data = vfs.read_to_end(imageset)?;
    let content = String::from_utf8_lossy(&data);
    let root = roxmltree::Document::parse(&content)?;
    let root = root.root_element();
    let image_file = root
        .attribute("Imagefile")
        .ok_or(anyhow!("Missing Imagefile attribute in root node"))?;
    let image_file = image_file.replace('\\', "/");
    let image_file = if !image_file.starts_with(&['/', '\\']) {
        format!("/{}", image_file)
    } else {
        image_file.to_string()
    };

    for image in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Image")
    {
        let mut read_node = || -> anyhow::Result<()> {
            let name = image
                .attribute("Name")
                .ok_or(anyhow!("Missing Name in image node"))?
                .to_lowercase();
            let x = image
                .attribute("XPos")
                .ok_or(anyhow!("Missing XPos in image node"))?
                .parse::<u32>()?;
            let y = image
                .attribute("YPos")
                .ok_or(anyhow!("Missing YPos in image node"))?
                .parse::<u32>()?;
            let width = image
                .attribute("Width")
                .ok_or(anyhow!("Missing Width in image node"))?
                .parse::<u32>()?;
            let height = image
                .attribute("Height")
                .ok_or(anyhow!("Missing Height in image node"))?
                .parse::<u32>()?;

            let sprite = sprite_cache
                .entry(image_file.clone())
                .or_insert_with(|| {
                    let image = {
                        let buf = vfs.read_to_end(&image_file).unwrap();
                        image::load_from_memory_with_format(&buf, image::ImageFormat::Png)
                            .unwrap()
                            .to_rgba8()
                    };
                    Rc::new(Sprite::load_from_image(image, component_factory.as_ref()))
                })
                .clone();
            portraits.insert(
                name.clone(),
                ImageSetImage {
                    name,
                    x,
                    y,
                    width,
                    height,
                    sprite,
                },
            );

            Ok(())
        };

        let ret = read_node();
        if let Err(e) = ret {
            log::error!("load portrait node failed: {:?}", e);
        }
    }

    Ok(())
}

pub struct Animation {
    pub keyframes: Vec<Vec<AnimKeyFrame>>,
    pub events: Vec<AnimationEvent>,
}
