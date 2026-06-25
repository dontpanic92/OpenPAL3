use std::{collections::HashMap, io::Cursor, rc::Rc};

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use fileformats::{
    binrw::BinRead,
    nod::NodFile,
    role_bin::{AssetItem, RoleBinFile},
};
use mini_fs::MiniFs;
use radiance::{
    comdef::{IComponent, IEntity, ISkyboxComponent},
    components::skybox::SkyboxComponent,
    rendering::ComponentFactory,
};

use crate::loaders::{
    Pal5TextureResolver,
    dff::{DffLoaderConfig, create_entity_from_dff_model},
};

/// One decoded terrain block: its grid coordinate, heightfield + footer
/// texture ids (`mp`), and optional per-texel splat weights (`alp`).
pub struct MapBlock {
    pub row: u32,
    pub col: u32,
    pub mp: fileformats::pal5::mp::MpFile,
    pub alp: Option<fileformats::pal5::alp::AlpFile>,
}

pub struct AssetLoader {
    vfs: Rc<MiniFs>,
    component_factory: Rc<dyn ComponentFactory>,
    pub index: HashMap<u32, AssetItem>,
    texture_resolver: Pal5TextureResolver,
}

impl AssetLoader {
    pub fn new(component_factory: Rc<dyn ComponentFactory>, vfs: Rc<MiniFs>) -> Rc<Self> {
        let index = load_index(&vfs);
        Rc::new(Self {
            component_factory,
            vfs,
            index,
            texture_resolver: Pal5TextureResolver {},
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

    pub fn load_map_nod(&self, map_name: &str) -> anyhow::Result<NodFile> {
        // Maps ship objects as a grid of blocks (`<map>_<r>_<c>.nod`).
        // Block patch/object coordinates are absolute world space (verified
        // clean-room: block `_r_c` terrain origins are `r*5120`,`c*5120`),
        // so the per-block node lists concatenate directly.
        let blocks = self.map_blocks(map_name, "nod");
        let mut merged: Option<NodFile> = None;
        for (r, c) in &blocks {
            let path = format!("/Map/{}/{}_{}_{}.nod", map_name, map_name, r, c);
            let nod = match self.vfs.read_to_end(&path) {
                Ok(bytes) => NodFile::read(&mut Cursor::new(bytes))?,
                Err(err) => {
                    log::warn!("Pal5 nod block {} unreadable: {}", path, err);
                    continue;
                }
            };
            match &mut merged {
                Some(acc) => acc.nodes.extend(nod.nodes),
                None => merged = Some(nod),
            }
        }
        merged.ok_or_else(|| anyhow::anyhow!("no .nod blocks found for map '{}'", map_name))
    }

    /// Enumerate the `(row, col)` block coordinates present for `map_name`
    /// with the given extension (e.g. `"mp"`, `"nod"`). Blocks form a
    /// contiguous grid from the origin, so we stop probing a row at its
    /// first gap and stop probing rows once a row's first column is absent.
    /// Single-block maps return just `(0, 0)`.
    fn map_blocks(&self, map_name: &str, ext: &str) -> Vec<(u32, u32)> {
        const MAX_BLOCK: u32 = 16;
        let exists = |r: u32, c: u32| {
            let path = format!("/Map/{}/{}_{}_{}.{}", map_name, map_name, r, c, ext);
            self.vfs.exists(&path)
        };
        let mut blocks = Vec::new();
        for r in 0..MAX_BLOCK {
            if !exists(r, 0) {
                break;
            }
            blocks.push((r, 0));
            for c in 1..MAX_BLOCK {
                if !exists(r, c) {
                    break;
                }
                blocks.push((r, c));
            }
        }
        if blocks.is_empty() {
            // Fall back to the canonical first block so callers can still
            // surface a precise read error for it.
            blocks.push((0, 0));
        }
        blocks
    }

    /// Decode and merge the per-map terrain heightfield across all blocks
    /// (`<map>_<r>_<c>.mp`). Block patch origins are absolute world
    /// coordinates, so the decoded patches concatenate into one seamless
    /// heightfield.
    pub fn load_map_terrain(
        &self,
        map_name: &str,
    ) -> anyhow::Result<fileformats::pal5::mp::MpFile> {
        use fileformats::pal5::mp::MpFile;

        let blocks = self.map_blocks(map_name, "mp");
        let mut merged: Option<MpFile> = None;
        let mut last_err: Option<anyhow::Error> = None;
        for (r, c) in &blocks {
            let path = format!("/Map/{}/{}_{}_{}.mp", map_name, map_name, r, c);
            let raw = match self.vfs.read_to_end(&path) {
                Ok(raw) => raw,
                Err(err) => {
                    last_err = Some(err.into());
                    continue;
                }
            };
            match MpFile::read(&raw) {
                Ok(mp) => match &mut merged {
                    Some(acc) => acc.patches.extend(mp.patches),
                    None => merged = Some(mp),
                },
                Err(err) => {
                    log::warn!("Pal5 terrain block {} decode failed: {}", path, err);
                    last_err = Some(err.into());
                }
            }
        }
        merged.ok_or_else(|| {
            last_err.unwrap_or_else(|| anyhow::anyhow!("no .mp blocks for map '{}'", map_name))
        })
    }

    /// Load every map block paired with its alphamap, keeping per-block
    /// boundaries (each block carries its own footer texture ids + weight
    /// rasters). Blocks with an unreadable/undecodable `.mp` are skipped;
    /// a missing `.alp` yields `alp = None` (that block renders base-only).
    pub fn load_map_blocks(&self, map_name: &str) -> Vec<MapBlock> {
        use fileformats::pal5::alp::AlpFile;
        use fileformats::pal5::mp::MpFile;

        let mut out = Vec::new();
        for (r, c) in self.map_blocks(map_name, "mp") {
            let mp_path = format!("/Map/{}/{}_{}_{}.mp", map_name, map_name, r, c);
            let mp = match self
                .vfs
                .read_to_end(&mp_path)
                .map_err(anyhow::Error::from)
                .and_then(|raw| MpFile::read(&raw).map_err(anyhow::Error::from))
            {
                Ok(mp) => mp,
                Err(err) => {
                    log::warn!("Pal5 terrain block {} failed: {}", mp_path, err);
                    continue;
                }
            };
            let alp_path = format!("/Map/{}/alphamap_{}_{}.alp", map_name, r, c);
            let alp = match self
                .vfs
                .read_to_end(&alp_path)
                .map_err(anyhow::Error::from)
                .and_then(|raw| AlpFile::read(&raw).map_err(anyhow::Error::from))
            {
                Ok(alp) => Some(alp),
                Err(err) => {
                    log::warn!("Pal5 alphamap {} failed: {}", alp_path, err);
                    None
                }
            };
            out.push(MapBlock {
                row: r,
                col: c,
                mp,
                alp,
            });
        }
        out
    }

    /// Read a raw asset file (e.g. a terrain `.dds`) from the vfs.
    pub fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        Ok(self.vfs.read_to_end(path)?)
    }

    /// Decode the map's per-map atmosphere (`Map/<map>/envinfo.env`):
    /// ambient + sun light color and direction. Returns `None` if the file
    /// is absent or undecodable (the caller falls back to flat lighting).
    pub fn load_map_env(&self, map_name: &str) -> Option<fileformats::pal5::env::EnvFile> {
        use fileformats::pal5::env::EnvFile;
        let path = format!("/Map/{}/envinfo.env", map_name);
        match self
            .vfs
            .read_to_end(&path)
            .map_err(anyhow::Error::from)
            .and_then(|raw| EnvFile::read(&raw))
        {
            Ok(env) => Some(env),
            Err(err) => {
                log::warn!("Pal5 envinfo {} failed: {}", path, err);
                None
            }
        }
    }

    pub fn load_model(&self, model_path: &str) -> anyhow::Result<ComRc<IEntity>> {
        self.load_model_ex(model_path, false)
    }

    /// Like [`load_model`](Self::load_model) but lets the caller opt the
    /// model's materials out of scene fog (`fog_exempt`). Used by
    /// [`load_skybox`](Self::load_skybox): the camera-locked sky dome must
    /// never fade to the fog color.
    fn load_model_ex(&self, model_path: &str, fog_exempt: bool) -> anyhow::Result<ComRc<IEntity>> {
        // PAL5's `role_*.bin` stores Windows backslash separators in
        // `file_path`; normalise to forward slashes so downstream log
        // lines (and `Pal5TextureResolver`'s path math) see a uniform
        // POSIX path. `packfs::pkg::pkg_archive::open` re-normalises
        // `/` → `\` internally, so the pkg lookup for the `.dff`
        // itself keeps working unchanged.
        let model_path = format!("/Model/{}", model_path.replace('\\', "/"));
        create_entity_from_dff_model(
            &self.component_factory,
            &self.vfs,
            model_path.clone(),
            model_path,
            true,
            &DffLoaderConfig {
                texture_resolver: &self.texture_resolver,
                keep_right_to_render_only: false,
                force_unique_materials: false,
                ignore_root_frame_translation: false,

                bsp_lightmap_tint: None,
                dynamic_lighting: true,
                fog_exempt,
            },
        )
    }

    /// Load the scene's skybox model by its `role_*.bin` asset id (the
    /// `SkyBoxID` carried in `envinfo.env`) and tag it with a
    /// [`SkyboxComponent`] so it stays centred on the camera every frame.
    ///
    /// Returns `None` when the id is absent from the role index, points at
    /// a non-`.dff` asset, or the model fails to load — all non-fatal: a
    /// scene without a skybox still renders its terrain and objects.
    pub fn load_skybox(&self, asset_id: u32) -> Option<ComRc<IEntity>> {
        let asset = self.index.get(&asset_id)?;
        let file_path = asset.file_path.to_string();
        if !file_path.to_ascii_lowercase().ends_with(".dff") {
            log::warn!(
                "Pal5 skybox asset {} has non-model path '{}'; skipping",
                asset_id,
                file_path,
            );
            return None;
        }

        let entity = match self.load_model_ex(&file_path, true) {
            Ok(entity) => entity,
            Err(err) => {
                log::warn!(
                    "Pal5 skybox model '{}' (asset {}) failed to load: {}",
                    file_path,
                    asset_id,
                    err,
                );
                return None;
            }
        };

        let component = SkyboxComponent::create(entity.clone());
        entity.add_component(
            ISkyboxComponent::uuid(),
            component.query_interface::<IComponent>().unwrap(),
        );
        log::info!("Pal5 skybox loaded: asset {} -> '{}'", asset_id, file_path);
        Some(entity)
    }
}

fn load_index(vfs: &MiniFs) -> HashMap<u32, AssetItem> {
    let index_files = [
        "/Config/role_00.bin",
        "/Config/role_01.bin",
        "/Config/role_02.bin",
        "/Config/role_03.bin",
        "/Config/role_04.bin",
        "/Config/role_05.bin",
    ];

    let mut index = HashMap::new();
    for path in index_files.iter() {
        match load_index_single(vfs, path, &mut index) {
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to load index file {}: {}", path, e);
            }
        }
    }

    index
}

fn load_index_single(
    vfs: &MiniFs,
    path: &str,
    index: &mut HashMap<u32, AssetItem>,
) -> anyhow::Result<()> {
    let role_bin = RoleBinFile::read(&mut Cursor::new(vfs.read_to_end(path)?))?;
    for item in role_bin.items {
        index.insert(item.id, item);
    }

    Ok(())
}
