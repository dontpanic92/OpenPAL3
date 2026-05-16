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
    rwbs::uva::UvAnimDict,
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
    vfs: Rc<MiniFs>,
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
        let vfs = Rc::new(vfs);
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

    pub fn vfs_rc(&self) -> Rc<MiniFs> {
        self.vfs.clone()
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
        let path = format!("/{}{}.dff", folder, file_name).replace("\\", "/");
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
                force_unique_materials: false,
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
                force_unique_materials: false,
            },
        );

        scene.add_entity(entity);

        println!("Loaded scene: {} {}", scene_name, block_name);
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

    /// Try to load the optional `<block>_water.dff` (animated water
    /// surface). Mirrors `try_load_scene_clip` / `try_load_scene_sky` —
    /// returns `None` for scenes that don't ship a water mesh.
    ///
    /// The on-disk layout for PAL4 water assets isn't uniform across
    /// scene categories (M-series → `/gamedata/scenedata/...`, Q-series
    /// → `/gamedata/{scene}/q01/{block}/...`, combat → `/gamedata/
    /// PALWorld/CombatWorld/...`), so this method probes several
    /// plausible candidates and loads the first that exists.
    pub fn try_load_scene_water(
        &self,
        scene_name: &str,
        block_name: &str,
    ) -> Option<ComRc<IEntity>> {
        let candidates = self.water_candidate_paths(scene_name, block_name, "dff");
        let path = self.find_first_existing(&candidates)?;
        log::debug!("[uv-anim] loading water DFF from {}", path);
        let entity = create_entity_from_dff_model(
            &self.component_factory,
            &self.vfs,
            path.clone(),
            "water".to_string(),
            true,
            &DffLoaderConfig {
                texture_resolver: &self.texture_resolver,
                keep_right_to_render_only: false,
                // Water materials are mutated per-frame by UvAnimDriver.
                // Opt them out of the shared material cache so the UV
                // transform doesn't leak onto unrelated geometry that
                // happens to share the same texture+params.
                force_unique_materials: true,
            },
        );
        Some(entity)
    }

    /// Try to load the UV-animation dictionary sibling of the water mesh
    /// (`<block>_water.uva`). Returns `None` if the file is missing or
    /// fails to parse — water surfaces without a `.uva` render statically
    /// with identity UVs.
    pub fn try_load_scene_water_uva(
        &self,
        scene_name: &str,
        block_name: &str,
    ) -> Option<UvAnimDict> {
        let candidates = self.water_candidate_paths(scene_name, block_name, "uva");
        let path = self.find_first_existing(&candidates)?;
        let data = self.vfs.read_to_end(&path).ok()?;
        match UvAnimDict::read_from_bytes(&data) {
            Ok(dict) => {
                log::debug!(
                    "[uv-anim] parsed {} animation(s) from {}",
                    dict.animations.len(),
                    path
                );
                Some(dict)
            }
            Err(e) => {
                log::error!("[uv-anim] failed to parse {}: {}", path, e);
                None
            }
        }
    }

    fn find_first_existing(&self, candidates: &[String]) -> Option<String> {
        for c in candidates {
            let exists = self.vfs.exists(c);
            log::debug!("[uv-anim] probe {} -> exists={}", c, exists);
            if exists {
                return Some(c.clone());
            }
        }
        // Most PAL4 scenes don't ship water; keep this at debug so it
        // doesn't drown the log on every scene load. Bump to info when
        // diagnosing a scene that SHOULD have water but isn't being
        // detected.
        log::debug!(
            "[uv-anim] no water asset found; tried {} path(s): {:?}",
            candidates.len(),
            candidates
        );
        None
    }

    /// Build the list of candidate VFS paths for a water asset, ordered
    /// most→least likely based on observed disk layouts:
    ///
    /// 1. `/gamedata/PALWorld/{scene}/{block}/{block}_water.<ext>`
    ///    (mirrors `_clip.dff` / `_sky.dff` — combat scenes)
    /// 2. `/gamedata/{scene}/q01/{block}/{block}_water.<ext>`
    ///    (Q-series quest layout: Q01_water lives at this shape)
    /// 3. `/gamedata/{scene}/{block_lower}/{block}/{block}_water.<ext>`
    ///    (general form of #2 for any block-lowercase mid-folder)
    /// 4. `/gamedata/scenedata/{scene}/{block}/{block}_water.<ext>`
    ///    (M-series scenedata layout, parallel to `_floor.dff`)
    /// 5. `/gamedata/ui2/ui/uiWorld/{block_lower}/{block}_water.<ext>`
    ///    (UI worlds — BJ_water, ZJM_water live here)
    fn water_candidate_paths(
        &self,
        scene_name: &str,
        block_name: &str,
        ext: &str,
    ) -> Vec<String> {
        let bl = block_name.to_lowercase();
        vec![
            format!("/gamedata/PALWorld/{}/{}/{}_water.{}", scene_name, block_name, block_name, ext),
            format!("/gamedata/{}/q01/{}/{}_water.{}", scene_name, block_name, block_name, ext),
            format!("/gamedata/{}/{}/{}/{}_water.{}", scene_name, bl, block_name, block_name, ext),
            format!("/gamedata/scenedata/{}/{}/{}_water.{}", scene_name, block_name, block_name, ext),
            format!("/gamedata/ui2/ui/uiWorld/{}/{}_water.{}", bl, block_name, ext),
        ]
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
                    force_unique_materials: false,
                },
            );

            Some(entity)
        } else {
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
        println!("Loading video: {}", path);
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
