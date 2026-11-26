use std::{cell::RefCell, path::Path, rc::Rc};

use anyhow::{Context, Result, ensure};
use common::store_ext::StoreExt2;
use crosscom::ComRc;
use fileformats::rwbs::{clump::Clump, read_dff};
use mini_fs::MiniFs;
use radiance::{
    comdef::{IEntity, IEntityExt, IScene, ISceneExt},
    math::{Vec3, transform_aabb},
    rendering::{RenderTarget, RenderingEngine},
    scene::{Camera, CoreScene},
};
use radiance_scripting::{
    comdef::services::IRenderTarget,
    services::{render_target::ScriptedRenderTarget, texture_cache::ImguiTextureCache},
};

use super::{asset_loader::AssetLoader, gameplay::GameplayCatalog};
use crate::loaders::{
    Pal4TextureResolver, TextureResolver,
    dff::{DffLoaderConfig, create_entity_from_dff_chunks},
};

#[derive(Clone, Copy)]
struct Request {
    tab: i32,
    id: i32,
    size: (u32, u32),
    yaw: f32,
}

struct ActivePreview {
    // Drop the target (which waits for GPU completion) before releasing its scene.
    texture: ComRc<IRenderTarget>,
    target: Rc<RefCell<Box<dyn RenderTarget>>>,
    scene: ComRc<IScene>,
    key: (i32, i32),
    center: Vec3,
    radius: f32,
}

pub struct Pal4MenuPreview {
    loader: Rc<AssetLoader>,
    engine: Rc<RefCell<dyn RenderingEngine>>,
    cache: Rc<RefCell<ImguiTextureCache>>,
    catalog: Rc<GameplayCatalog>,
    request: Option<Request>,
    active: Option<ActivePreview>,
    retired: Vec<(u8, ActivePreview)>,
    failed: Option<(i32, i32)>,
}

impl Pal4MenuPreview {
    pub fn new(
        loader: Rc<AssetLoader>,
        engine: Rc<RefCell<dyn RenderingEngine>>,
        cache: Rc<RefCell<ImguiTextureCache>>,
        catalog: Rc<GameplayCatalog>,
    ) -> Self {
        Self {
            loader,
            engine,
            cache,
            catalog,
            request: None,
            active: None,
            retired: Vec::new(),
            failed: None,
        }
    }

    pub fn request(&mut self, tab: i32, id: i32, width: f32, height: f32, yaw: f32) -> i32 {
        let Some(size) = preview_extent(width, height) else {
            return 0;
        };
        if !yaw.is_finite() {
            return 0;
        }
        self.request = Some(Request { tab, id, size, yaw });
        self.active
            .as_ref()
            .filter(|active| active.key == (tab, id))
            .map_or(0, |active| active.texture.texture_id())
    }

    /// Runs during director update, never while the imgui texture cache is borrowed.
    pub fn update(&mut self, tab: Option<i32>) {
        for (frames, _) in &mut self.retired {
            *frames = frames.saturating_sub(1);
        }
        self.retired.retain(|(frames, _)| *frames > 0);
        let request = self
            .request
            .take()
            .filter(|request| Some(request.tab) == tab);
        let Some(request) = request else {
            self.retire_active();
            self.failed = None;
            return;
        };
        let key = (request.tab, request.id);
        if self.failed == Some(key) {
            return;
        }
        if self.active.as_ref().map(|active| active.key) != Some(key) {
            self.retire_active();
            self.failed = None;
            match self.load(request) {
                Ok(active) => self.active = Some(active),
                Err(error) => {
                    log::warn!("PAL4 menu model {}: {error:#}", request.id);
                    self.failed = Some(key);
                    return;
                }
            }
        }
        let active = self.active.as_ref().unwrap();
        let mut target = active.target.borrow_mut();
        if target.extent() != request.size {
            target.resize(request.size.0, request.size.1);
        }
        let aspect = request.size.0 as f32 / request.size.1 as f32;
        {
            let mut camera = active.scene.camera_mut();
            camera.set_aspect(aspect);
            let distance = framing_distance(active.radius, &camera);
            let eye = Vec3::new(
                active.center.x + distance * request.yaw.sin() * 0.98,
                active.center.y + distance * 0.2,
                active.center.z + distance * request.yaw.cos() * 0.98,
            );
            camera
                .transform_mut()
                .set_position(&eye)
                .look_at(&active.center);
        }
        self.engine
            .borrow_mut()
            .render_scene_to_target(active.scene.clone(), target.as_mut());
    }

    fn retire_active(&mut self) {
        if let Some(active) = self.active.take() {
            // UI is painted before director update. Keep this frame's referenced
            // descriptor alive for MAX_FRAMES_IN_FLIGHT + one safety frame.
            self.retired.push((3, active));
        }
    }

    fn load(&self, request: Request) -> Result<ActivePreview> {
        let (model, texture) = if request.tab == 1 {
            let item = self
                .catalog
                .properties
                .get(&request.id)
                .context("unknown property")?;
            (&item.model, &item.texture)
        } else {
            ensure!(request.tab == 2, "unsupported preview tab");
            let item = self
                .catalog
                .equipment
                .get(&request.id)
                .context("unknown equipment")?;
            (&item.model, &item.texture)
        };
        let (path, chunks) =
            read_preview_model(self.loader.vfs(), request.tab == 2, model, texture)?;
        let factory = self.loader.component_factory();
        let resolver = PreviewTextureResolver;
        let mut config = DffLoaderConfig::new(&resolver);
        config.ignore_root_frame_translation = true;
        config.fog_exempt = true;
        let entity = create_entity_from_dff_chunks(
            &factory,
            self.loader.vfs(),
            Path::new(&path),
            format!("menu-preview-{}", request.id),
            true,
            &config,
            chunks,
        );
        entity
            .transform()
            .borrow_mut()
            .rotate_axis_angle_local(&Vec3::EAST, 30.0_f32.to_radians());
        let scene = CoreScene::create();
        scene.add_entity(entity.clone());
        scene.load();
        scene.update(0.0);
        let (center, radius) =
            model_bounds(&entity).context("model has no finite renderable bounds")?;
        *scene.camera_mut() = Camera::new_with_params(
            45.0_f32.to_radians(),
            request.size.0 as f32 / request.size.1 as f32,
            0.01,
            (radius * 100.0).max(1000.0),
        );
        let mut target = factory.create_render_target(request.size.0, request.size.1);
        target.set_clear_color([0.0; 4]);
        let (texture, target) = ScriptedRenderTarget::create(target, self.cache.clone());
        Ok(ActivePreview {
            texture,
            target,
            scene,
            key: (request.tab, request.id),
            center,
            radius,
        })
    }
}

fn preview_extent(width: f32, height: f32) -> Option<(u32, u32)> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let scale = (1024.0 / width.max(height)).min(1.0);
    Some((
        (width * scale).round().max(1.0) as u32,
        (height * scale).round().max(1.0) as u32,
    ))
}

fn framing_distance(radius: f32, camera: &Camera) -> f32 {
    let projection = camera.projection_matrix();
    let tight_axis = projection[0][0].abs().max(projection[1][1].abs());
    let half_angle = (1.0 / tight_axis).atan();
    radius.max(0.1) / half_angle.sin() * 1.12
}

fn model_bounds(entity: &ComRc<IEntity>) -> Option<(Vec3, f32)> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    accumulate_bounds(entity, &mut min, &mut max);
    if !min.iter().chain(max.iter()).all(|value| value.is_finite()) {
        return None;
    }
    let center = Vec3::new(
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    );
    let radius =
        ((max[0] - min[0]).powi(2) + (max[1] - min[1]).powi(2) + (max[2] - min[2]).powi(2)).sqrt()
            * 0.5;
    if ![center.x, center.y, center.z, radius]
        .iter()
        .all(|value| value.is_finite())
    {
        return None;
    }
    Some((center, radius.max(0.1)))
}

fn accumulate_bounds(entity: &ComRc<IEntity>, min: &mut [f32; 3], max: &mut [f32; 3]) {
    if let Some(component) = entity.get_rendering_component() {
        let world = entity.world_transform();
        for object in component.render_objects() {
            if let Some((lo, hi)) = object.as_dyn().local_aabb() {
                let (lo, hi) = transform_aabb(lo, hi, world.matrix());
                for axis in 0..3 {
                    min[axis] = min[axis].min(lo[axis]);
                    max[axis] = max[axis].max(hi[axis]);
                }
            }
        }
    }
    for child in entity.children() {
        accumulate_bounds(&child, min, max);
    }
}

fn read_preview_model(
    vfs: &MiniFs,
    equipment: bool,
    model: &str,
    texture: &str,
) -> Result<(String, Vec<Clump>)> {
    let valid = |name: &str| {
        name.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    ensure!(
        !model.is_empty() && valid(model) && valid(texture),
        "invalid preview asset name"
    );
    let pack = if equipment { "PALWeapon" } else { "PALObject" };
    let mut path = format!("/gamedata/{pack}/{model}/{model}.dff");
    if equipment && !vfs.exists(&path) {
        path = format!("/gamedata/PALObject/{model}/{model}.dff");
    }
    let bytes = vfs
        .read_to_end(&path)
        .with_context(|| format!("reading {path}"))?;
    let mut chunks = read_dff(&bytes).with_context(|| format!("parsing {path}"))?;
    for chunk in &mut chunks {
        chunk.atomics.retain(|atomic| {
            let helper = chunk
                .frames
                .get(atomic.frame as usize)
                .and_then(|frame| frame.name())
                .is_some_and(|name| name.to_ascii_lowercase().starts_with("hook_"));
            let untextured =
                chunk
                    .geometries
                    .get(atomic.geometry as usize)
                    .is_some_and(|geometry| {
                        geometry
                            .materials
                            .iter()
                            .all(|material| material.texture.is_none())
                    });
            !(helper && untextured)
        });
    }
    if !texture.is_empty() {
        // The global texture store keys by name. Rename the parsed material as
        // well as its disk lookup so shared models cannot reuse another item's skin.
        let key = format!("pal4-menu:{path}#{texture}");
        for chunk in &mut chunks {
            for geometry in &mut chunk.geometries {
                for material in &mut geometry.materials {
                    if let Some(diffuse) = &mut material.texture {
                        diffuse.name = key.clone();
                    }
                }
            }
        }
    }
    Ok((path, chunks))
}

struct PreviewTextureResolver;
impl TextureResolver for PreviewTextureResolver {
    fn resolve_texture(&self, vfs: &MiniFs, path: &Path, name: &str) -> Option<Vec<u8>> {
        let name = name.rsplit_once('#').map_or(name, |(_, name)| name);
        Pal4TextureResolver.resolve_texture(vfs, path, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extent_and_camera_framing_preserve_portrait_and_landscape_models() {
        assert_eq!(preview_extent(2000.0, 4000.0), Some((512, 1024)));
        assert_eq!(preview_extent(400.0, 200.0), Some((400, 200)));
        assert_eq!(preview_extent(0.0, 200.0), None);
        assert_eq!(preview_extent(f32::NAN, 200.0), None);
        for aspect in [0.5, 1.0, 2.0] {
            let camera = Camera::new_with_params(45.0_f32.to_radians(), aspect, 0.01, 1000.0);
            let distance = framing_distance(50.0, &camera);
            for axis in [0, 1] {
                let half_angle = (1.0 / camera.projection_matrix()[axis][axis].abs()).atan();
                assert!(distance * half_angle.sin() > 50.0);
            }
        }
    }

    #[test]
    #[ignore = "requires original PAL4 assets; set PAL4_ASSET_ROOT"]
    fn original_models_resolve_catalog_skins_without_cache_aliasing() {
        let vfs = packfs::init_virtual_fs(std::env::var("PAL4_ASSET_ROOT").unwrap(), None);
        let catalog = GameplayCatalog::load(&vfs).unwrap();
        let mut names = Vec::new();
        for id in [3000, 3004, 3006, 3501, 3503, 3558] {
            let (equipment, model, texture) = if let Some(item) = catalog.properties.get(&id) {
                (false, &item.model, &item.texture)
            } else {
                let item = &catalog.equipment[&id];
                (true, &item.model, &item.texture)
            };
            let (path, chunks) = read_preview_model(&vfs, equipment, model, texture).unwrap();
            for chunk in &chunks {
                for atomic in &chunk.atomics {
                    let name = chunk.frames[atomic.frame as usize]
                        .name()
                        .unwrap_or_default();
                    assert!(!name.to_ascii_lowercase().starts_with("hook_"));
                }
            }
            let textures: Vec<_> = chunks
                .iter()
                .flat_map(|chunk| &chunk.geometries)
                .flat_map(|geometry| &geometry.materials)
                .filter_map(|material| material.texture.as_ref())
                .collect();
            assert!(!textures.is_empty());
            for diffuse in &textures {
                assert!(
                    PreviewTextureResolver
                        .resolve_texture(&vfs, Path::new(&path), &diffuse.name)
                        .is_some()
                );
            }
            names.push(textures[0].name.clone());
        }
        assert_ne!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
        assert_ne!(names[3], names[4]);
    }
}
