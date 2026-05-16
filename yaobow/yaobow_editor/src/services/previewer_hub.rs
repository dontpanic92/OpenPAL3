//! `IPreviewerHub` Rust implementation.
//!
//! Routes a vfs path to the appropriate previewer based on file extension /
//! contents and returns the matching foreign handle. The hub owns no
//! per-file state — handles do — so opening many files in parallel is fine.

use std::cell::RefCell;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::{LittleEndian, ReadBytesExt};
use chardet::detect;
use common::store_ext::StoreExt2;
use crosscom::ComRc;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use fileformats::{binrw::BinRead, mv3::read_mv3, nod::NodFile, pol::read_pol, rwbs};
use image::ImageFormat;
use mini_fs::{MiniFs, StoreExt};
use radiance::audio::{AudioEngine, Codec as AudioCodec};
use radiance::comdef::{IEntity, ISceneManager};
use radiance::rendering::ComponentFactory;
use radiance::video::Codec as VideoCodec;
use radiance_scripting::services::ImguiTextureCache;
use shared::loaders::anm::load_anm;
use shared::loaders::smp::load_smp;
use shared::openpal3::loaders::{
    cvd_loader::cvd_load_from_file, nav_loader::nav_load_from_file,
    sce_loader::sce_load_from_file, scn_loader::scn_load_from_file,
};
use shared::GameType;

use crate::comdef::editor_services::{
    IAudioHandle, IImageHandle, IModelHandle, IPreviewerHub, IPreviewerHubImpl, IVideoHandle,
};
use crate::directors::DevToolsAssetLoader;
use crate::services::handles::{AudioHandle, ImageHandle, ModelHandle, VideoHandle};

// PreviewKind enum (mirrors the comment in yaobow_editor_services.idl).
const KIND_UNSUPPORTED: i32 = 0;
const KIND_TEXT: i32 = 1;
const KIND_IMAGE: i32 = 2;
const KIND_AUDIO: i32 = 3;
const KIND_VIDEO: i32 = 4;
const KIND_MODEL: i32 = 5;
const KIND_STRUCTURED: i32 = 6;

pub struct PreviewerHub {
    vfs: Rc<MiniFs>,
    asset_loader: DevToolsAssetLoader,
    game_type: GameType,
    factory: Rc<dyn ComponentFactory>,
    audio_engine: Rc<dyn AudioEngine>,
    scene_manager: ComRc<ISceneManager>,
    cache: Rc<RefCell<ImguiTextureCache>>,
    preview_registry: Rc<crate::services::preview_registry::PreviewRegistry>,
    last_string: RefCell<String>,
}

ComObject_PreviewerHub!(super::PreviewerHub);

impl PreviewerHub {
    pub fn create(
        vfs: Rc<MiniFs>,
        asset_loader: DevToolsAssetLoader,
        game_type: GameType,
        factory: Rc<dyn ComponentFactory>,
        audio_engine: Rc<dyn AudioEngine>,
        scene_manager: ComRc<ISceneManager>,
        cache: Rc<RefCell<ImguiTextureCache>>,
        preview_registry: Rc<crate::services::preview_registry::PreviewRegistry>,
    ) -> ComRc<IPreviewerHub> {
        ComRc::from_object(Self {
            vfs,
            asset_loader,
            game_type,
            factory,
            audio_engine,
            scene_manager,
            cache,
            preview_registry,
            last_string: RefCell::new(String::new()),
        })
    }

    fn set_last(&self, s: String) -> &str {
        *self.last_string.borrow_mut() = s;
        // SAFETY: see ConfigService::get_asset_path — single-threaded
        // script/UI path; codegen copies the &str into a CString immediately.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
}

fn extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
}

fn classify_path(path: &str) -> i32 {
    match extension(path).as_deref() {
        Some("h" | "asm" | "ini" | "txt" | "conf" | "cfg" | "log") => KIND_TEXT,
        Some("tga" | "png" | "dds") => KIND_IMAGE,
        Some("mp3" | "smp" | "wav" | "ogg") => KIND_AUDIO,
        Some("bik") => KIND_VIDEO,
        Some("mv3" | "cvd" | "dff" | "anm" | "bsp" | "pol") => KIND_MODEL,
        Some("scn" | "nav" | "sce" | "nod") => KIND_STRUCTURED,
        _ => KIND_UNSUPPORTED,
    }
}

fn jsonify<T: ?Sized + serde::Serialize>(t: &T) -> String {
    serde_json::to_string_pretty(t).unwrap_or_else(|_| "Cannot serialize into Json".to_string())
}

impl IPreviewerHubImpl for PreviewerHub {
    fn classify(&self, vfs_path: &str) -> i32 {
        classify_path(vfs_path)
    }

    fn open_text(&self, vfs_path: &str) -> &str {
        let path = PathBuf::from(vfs_path);
        let value = match self.vfs.read_to_end(&path) {
            Ok(bytes) => {
                let result = detect(&bytes);
                let coder = encoding_from_whatwg_label(chardet::charset2encoding(&result.0))
                    .unwrap_or(encoding::all::GBK);
                coder
                    .decode(&bytes, DecoderTrap::Ignore)
                    .unwrap_or_else(|_| {
                        "Cannot read the file as GBK encoded text content".to_string()
                    })
            }
            Err(_) => "Cannot open this file".to_string(),
        };
        self.set_last(value)
    }

    fn dump_structured(&self, vfs_path: &str) -> &str {
        let path = PathBuf::from(vfs_path);
        let text = match extension(vfs_path).as_deref() {
            Some("scn") => jsonify(&scn_load_from_file(&self.vfs, &path)),
            Some("nav") => jsonify(&nav_load_from_file(&self.vfs, &path)),
            Some("sce") => jsonify(&sce_load_from_file(&self.vfs, &path)),
            Some("anm") => match load_anm(&self.vfs, &path) {
                Ok(anm) => jsonify(&anm),
                Err(e) => e.to_string(),
            },
            Some("nod") => match self.vfs.open(&path) {
                Ok(file) => match NodFile::read(&mut BufReader::new(file)) {
                    Ok(nod) => jsonify(&nod),
                    Err(e) => e.to_string(),
                },
                Err(e) => e.to_string(),
            },
            _ => "Unsupported".to_string(),
        };
        self.set_last(text)
    }

    fn open_image(&self, vfs_path: &str) -> Option<ComRc<IImageHandle>> {
        let path = PathBuf::from(vfs_path);
        let extension = extension(vfs_path)?;
        match extension.as_str() {
            "tga" | "png" | "dds" => {}
            _ => return None,
        }

        let bytes = self.vfs.read_to_end(&path).ok()?;
        let dyn_image = match (extension.as_str(), self.game_type) {
            ("png", GameType::SWD5 | GameType::SWDCF | GameType::SWDHC) => {
                if bytes.len() < 8 {
                    return None;
                }
                let width = (&bytes[0..4]).read_u32::<LittleEndian>().ok()?;
                let height = (&bytes[4..8]).read_u32::<LittleEndian>().ok()?;
                let data = &bytes[8..];
                let img = image::RgbaImage::from_raw(width, height, data.to_vec())?;
                image::DynamicImage::ImageRgba8(img)
            }
            _ => image::load_from_memory(&bytes)
                .or_else(|_| image::load_from_memory_with_format(&bytes, ImageFormat::Tga))
                .ok()?,
        };
        let rgba = dyn_image.to_rgba8();
        let (width, height) = rgba.dimensions();
        ImageHandle::create(self.cache.clone(), &rgba.into_raw(), width, height)
    }

    fn open_audio(&self, vfs_path: &str) -> Option<ComRc<IAudioHandle>> {
        let path = PathBuf::from(vfs_path);
        let extension = extension(vfs_path)?;
        let codec = match extension.as_str() {
            "mp3" | "smp" => AudioCodec::Mp3,
            "wav" => AudioCodec::Wav,
            "ogg" => AudioCodec::Ogg,
            _ => return None,
        };

        let mut data = self.vfs.read_to_end(&path).ok()?;
        if extension == "smp" {
            data = load_smp(data).ok()?;
        }
        let mut source = self.audio_engine.create_source();
        source.set_data(data, codec);
        Some(AudioHandle::create(source))
    }

    fn open_video(&self, vfs_path: &str) -> Option<ComRc<IVideoHandle>> {
        let path = PathBuf::from(vfs_path);
        let extension = extension(vfs_path)?;
        let codec = match extension.as_str() {
            "bik" => VideoCodec::Bik,
            _ => return None,
        };

        let reader = Box::new(BufReader::new(self.vfs.open(&path).ok()?));
        let mut player = self.factory.create_video_player();
        let size = player.play(
            self.factory.clone(),
            self.audio_engine.clone(),
            reader,
            codec,
            true,
        )?;
        Some(VideoHandle::create(
            self.cache.clone(),
            player,
            size.0,
            size.1,
        ))
    }

    fn open_model(&self, vfs_path: &str) -> Option<ComRc<IModelHandle>> {
        let path = PathBuf::from(vfs_path);
        let ext = extension(vfs_path)?;
        let (text, entity) = match ext.as_str() {
            "mv3" => load_mv3(&self.vfs, &path, &self.asset_loader)?,
            "cvd" => load_cvd(&self.vfs, &path, &self.asset_loader)?,
            "dff" | "anm" => load_dff(&self.vfs, &path, &self.asset_loader, self.game_type)?,
            "bsp" => load_bsp(&self.vfs, &path, &self.asset_loader, self.game_type)?,
            "pol" => load_pol(&self.vfs, &path, &self.asset_loader)?,
            _ => return None,
        };
        Some(ModelHandle::create(
            text,
            entity,
            self.factory.clone(),
            self.cache.clone(),
            self.preview_registry.clone(),
        ))
    }
}

fn load_mv3(
    vfs: &MiniFs,
    path: &Path,
    asset_loader: &DevToolsAssetLoader,
) -> Option<(String, ComRc<IEntity>)> {
    use shared::openpal3::scene::{
        create_animated_mesh_from_mv3, create_mv3_entity, RoleAnimationRepeatMode, RoleController,
    };
    let text = read_mv3(&mut BufReader::new(vfs.open(path).ok()?))
        .map(|f| jsonify(&f))
        .unwrap_or_else(|_| "Unsupported".to_string());

    let entity = create_mv3_entity(
        asset_loader.pal3()?.clone(),
        "101",
        "preview",
        "preview".to_string(),
        true,
    )
    .ok()?;
    let anim = create_animated_mesh_from_mv3(
        entity.clone(),
        &asset_loader.component_factory(),
        asset_loader.vfs(),
        path,
    );
    if let Ok(anim) = anim {
        if let Some(controller) = RoleController::get_role_controller(entity.clone()) {
            controller.get().play_anim_mesh(
                "preview".to_string(),
                anim,
                RoleAnimationRepeatMode::Loop,
            );
            controller.get().set_active(true);
        }
    }
    Some((text, entity))
}

fn load_cvd(
    vfs: &MiniFs,
    path: &Path,
    asset_loader: &DevToolsAssetLoader,
) -> Option<(String, ComRc<IEntity>)> {
    use shared::openpal3::scene::create_entity_from_cvd_model;
    let text = cvd_load_from_file(vfs, path)
        .map(|f| jsonify(&f))
        .unwrap_or_else(|_| "Unsupported".to_string());
    let entity = create_entity_from_cvd_model(
        asset_loader.component_factory(),
        vfs,
        path,
        "preview".to_string(),
        true,
    );
    Some((text, entity))
}

fn load_dff(
    vfs: &MiniFs,
    path: &Path,
    asset_loader: &DevToolsAssetLoader,
    game_type: GameType,
) -> Option<(String, ComRc<IEntity>)> {
    use radiance::comdef::{IArmatureComponent, IComponent};
    use shared::loaders::dff::{create_entity_from_dff_model, DffLoaderConfig};
    use shared::loaders::Pal4TextureResolver;
    use shared::openpal4::actor::{Pal4ActorAnimationConfig, Pal4ActorAnimationController};
    use shared::openpal4::comdef::IPal4ActorAnimationController;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let text = if ext.as_deref() == Some("dff") {
        let mut buf = vec![];
        let _ = vfs.open(path).ok()?.read_to_end(&mut buf);
        rwbs::read_dff(&buf)
            .map(|f| jsonify(&f))
            .unwrap_or_else(|_| "Unsupported".to_string())
    } else {
        load_anm(vfs, path)
            .map(|f| jsonify(&f))
            .unwrap_or_else(|_| "Unsupported".to_string())
    };

    let entity = if ext.as_deref() == Some("dff") {
        create_entity_from_dff_model(
            &asset_loader.component_factory(),
            vfs,
            path,
            "preview".to_string(),
            true,
            &DffLoaderConfig {
                texture_resolver: &Pal4TextureResolver {},
                keep_right_to_render_only: false,
                force_unique_materials: false,
            },
        )
    } else if game_type == GameType::PAL4 {
        let folder_path = path.parent()?;
        let actor_name = folder_path.file_name()?.to_str()?;
        let dff_path = folder_path.join(format!("{}.dff", actor_name));
        let entity = create_entity_from_dff_model(
            &asset_loader.component_factory(),
            vfs,
            dff_path,
            "preview".to_string(),
            true,
            &DffLoaderConfig {
                texture_resolver: &Pal4TextureResolver {},
                keep_right_to_render_only: false,
                force_unique_materials: false,
            },
        );

        let armature = entity
            .get_component(IArmatureComponent::uuid())?
            .query_interface::<IArmatureComponent>()?;
        let pal4_loader = asset_loader.pal4()?;
        let controller =
            Pal4ActorAnimationController::create(actor_name.to_string(), pal4_loader, armature);
        entity.add_component(
            IPal4ActorAnimationController::uuid(),
            controller.query_interface::<IComponent>()?,
        );
        let anm = load_anm(asset_loader.vfs(), path).unwrap_or_default();
        controller.play_animation(anm, vec![], Pal4ActorAnimationConfig::Looping);
        entity
    } else {
        return None;
    };

    Some((text, entity))
}

fn load_bsp(
    vfs: &MiniFs,
    path: &Path,
    asset_loader: &DevToolsAssetLoader,
    game_type: GameType,
) -> Option<(String, ComRc<IEntity>)> {
    use shared::loaders::bsp::create_entity_from_bsp_model;
    let mut buf = vec![];
    let _ = vfs.open(path).ok()?.read_to_end(&mut buf);
    let text = rwbs::read_bsp(&buf)
        .map(|f| jsonify(&f))
        .unwrap_or_else(|_| "Unsupported".to_string());
    let entity = create_entity_from_bsp_model(
        &asset_loader.component_factory(),
        vfs,
        path,
        "preview".to_string(),
        game_type.dff_loader_config()?,
    );
    Some((text, entity))
}

fn load_pol(
    vfs: &MiniFs,
    path: &Path,
    asset_loader: &DevToolsAssetLoader,
) -> Option<(String, ComRc<IEntity>)> {
    use shared::openpal3::loaders::pol::create_entity_from_pol_model;
    let text = read_pol(&mut BufReader::new(vfs.open(path).ok()?))
        .map(|f| jsonify(&f))
        .unwrap_or_else(|_| "Unsupported".to_string());
    let entity = create_entity_from_pol_model(
        &asset_loader.component_factory(),
        vfs,
        path,
        "preview".to_string(),
        true,
    );
    Some((text, entity))
}
