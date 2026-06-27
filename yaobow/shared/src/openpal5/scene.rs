use crosscom::ComRc;
use radiance::{comdef::IScene, math::Vec3, scene::CoreScene};

use super::asset_loader::AssetLoader;
use radiance::comdef::{IEntityExt, ISceneExt};

pub struct Pal5Scene {
    pub scene: ComRc<IScene>,
}

impl Pal5Scene {
    pub fn new_empty() -> Self {
        Self {
            scene: CoreScene::create(),
        }
    }

    pub fn load(asset_loader: &AssetLoader, scene_name: &str) -> anyhow::Result<Self> {
        let scene = CoreScene::create();
        scene.camera_mut().set_fov43(45_f32.to_radians());

        // Per-map atmosphere (`envinfo.env`): a dim ambient fill plus a
        // directional sun. PAL5 ships no per-scene `.lgt`; its terrain and
        // (when dynamic lighting is enabled) its buildings are dynamically lit
        // (Lambert + ambient), so without this the lighting term would
        // collapse to a flat, dead ambient. The sun is a first-class
        // directional light in the parsed azimuth/elevation direction. If the
        // `.env` is missing, fall back to full ambient so geometry never
        // renders black.
        match asset_loader.load_map_env(scene_name) {
            Some(env) => {
                let dir = env.sun_direction();
                let sun = radiance::scene::DirectionalLight::new(
                    Vec3::new(dir[0], dir[1], dir[2]),
                    env.sun_color,
                );

                // Per-map linear distance fog. PAL5's original engine uses
                // fixed-function linear eye-space fog: each vertex shader emits
                // `oFog = clip-w` (eye depth) and the device blends toward the
                // fog color between `FOGSTART` and `FOGEND`. We reproduce that
                // as a first-class radiance `Fog` (linear eye-space). Only
                // enable it when the range is well-formed (`end > start`); a few
                // demon-realm battlemaps ship inverted/degenerate values (e.g.
                // start=1.0,end=0.0) which we treat as "no fog" rather than
                // washing the whole scene to the fog color.
                //
                // The `[0,1]` `envinfo.env` fractions (`fog_param_a/b`) scale by
                // the main scene camera's far distance to give the eye-space
                // FOGSTART/FOGEND. PAL5 renders its world at true scale (terrain
                // blocks span 5120 units; the `far = 1000` projection in
                // `Pal5.exe` is an aspect=1.0 sub-camera, NOT the world view).
                // `6000` was calibrated against the original game's density
                // (kuangfengzhai: gentle haze on the far cliffs, crisp village);
                // tunable via the `PAL5_FOG_FAR` env var.
                let fog_far: f32 = std::env::var("PAL5_FOG_FAR")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(6000.0);
                let fog_start = env.fog_param_a * fog_far;
                let fog_end = env.fog_param_b * fog_far;
                let fog = if fog_end > fog_start + 1.0 {
                    Some(radiance::scene::Fog::new(
                        [env.fog_color[0], env.fog_color[1], env.fog_color[2]],
                        fog_start,
                        fog_end,
                    ))
                } else {
                    None
                };

                let mut lighting =
                    radiance::scene::SceneLighting::with_sun(env.ambient, vec![], sun);
                lighting.fog = fog;
                scene.set_lighting(lighting);

                log::info!(
                    "Pal5Scene '{}': atmosphere ambient {:?} sun {:?} az={} el={} dir {:?} | fog {:?} a={} b={} -> eye[{:.0}..{:.0}] enabled={} skybox={:?}",
                    scene_name,
                    env.ambient,
                    env.sun_color,
                    env.sun_azimuth_deg,
                    env.sun_elevation_deg,
                    dir,
                    env.fog_color,
                    env.fog_param_a,
                    env.fog_param_b,
                    fog_start,
                    fog_end,
                    fog.is_some(),
                    env.skybox_asset_id(),
                );

                // Skybox: the scene's `SkyBoxID` (carried in `envinfo.env`)
                // selects a `\BuildingP5\yingdi\_skybox*.dff` model. It is
                // re-centred on the camera every frame by the attached
                // `SkyboxComponent`, so a scene with no skybox id simply
                // renders without one.
                if let Some(skybox_id) = env.skybox_asset_id() {
                    if let Some(skybox) = asset_loader.load_skybox(skybox_id) {
                        scene.add_entity(skybox);
                    }
                }
            }
            None => {
                scene.set_lighting(radiance::scene::SceneLighting::new([1.0, 1.0, 1.0], vec![]));
            }
        }

        // Terrain heightfield + splat textures, loaded per block. A
        // decode/build failure is non-fatal — the scene's objects still
        // load.
        let blocks = asset_loader.load_map_blocks(scene_name);
        let patch_count: usize = blocks.iter().map(|b| b.mp.patches.len()).sum();
        if !blocks.is_empty() {
            if let Some(terrain) =
                super::terrain::build_terrain_entity(asset_loader, scene_name, &blocks)
            {
                scene.add_entity(terrain);
                log::info!(
                    "Pal5Scene '{}': terrain loaded ({} blocks, {} patches)",
                    scene_name,
                    blocks.len(),
                    patch_count,
                );
            }
        } else {
            log::warn!("Pal5Scene '{}': no terrain blocks found", scene_name);
        }

        let nod = asset_loader.load_map_nod(scene_name)?;

        let mut loaded = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;

        for node in &nod.nodes {
            // Resolve the node's asset entry from the role index. Many
            // `.nod` nodes reference ids that are absent from the
            // `role_*.bin` index (gameplay markers, server-only props);
            // those are skipped, not fatal.
            let Some(asset) = asset_loader.index.get(&node.asset_id) else {
                log::debug!(
                    "Pal5Scene: node '{:?}' asset_id {} not in role index; skipping",
                    node.name,
                    node.asset_id,
                );
                continue;
            };

            let file_path = asset.file_path.to_string();

            // Only `.dff` clumps are renderable scene objects. PAL5 also
            // stores degenerate `file_path` values (e.g. `"1"`) for
            // non-model assets — guard against those so a stray entry
            // never reaches the loader.
            if !file_path.to_ascii_lowercase().ends_with(".dff") {
                skipped += 1;
                continue;
            }

            // Isolate per-node failures: a single unreadable/corrupt
            // model must not abort the whole scene (foliage is
            // interleaved with buildings, so an early `?` would hide
            // everything after the first bad node).
            let model = match asset_loader.load_model(&file_path) {
                Ok(model) => model,
                Err(err) => {
                    failed += 1;
                    log::warn!(
                        "Pal5Scene: failed to load model '{}' (asset_id {}): {}",
                        file_path,
                        node.asset_id,
                        err,
                    );
                    continue;
                }
            };

            model
                .transform()
                .borrow_mut()
                .scale_local(&Vec3::new(node.scale[0], node.scale[1], node.scale[2]))
                .rotate_axis_angle_local(&Vec3::BACK, -node.rotation[0].to_radians())
                .rotate_axis_angle_local(&Vec3::UP, node.rotation[1].to_radians())
                .rotate_axis_angle_local(&Vec3::EAST, -node.rotation[2].to_radians())
                .set_position(&Vec3::new(
                    node.position[0],
                    node.position[1],
                    node.position[2],
                ));
            scene.add_entity(model);
            loaded += 1;
        }

        log::info!(
            "Pal5Scene '{}': {} models loaded, {} skipped (non-model/unindexed), {} failed of {} nodes",
            scene_name,
            loaded,
            skipped,
            failed,
            nod.nodes.len(),
        );

        // Grass (`<map>_<r>_<c>.ctr`): a terrain-conformal grass overlay built
        // from each block's `.ctr` density grid (`cao###`-textured, density-
        // driven coverage). See `grass::build_block_grass`. Non-fatal — a map
        // with no grass still renders terrain + objects.
        let mut grass_layers = 0usize;
        for block in &blocks {
            let leaves = asset_loader.load_block_ctr(scene_name, block.row, block.col);
            if leaves.is_empty() {
                continue;
            }
            let chunks = super::grass::build_block_grass(asset_loader, scene_name, block, &leaves);
            grass_layers += chunks.len();
            for chunk in chunks {
                scene.add_entity(chunk);
            }
        }
        if grass_layers > 0 {
            log::info!(
                "Pal5Scene '{}': grass overlay built ({} layer chunks)",
                scene_name,
                grass_layers,
            );
        }

        Ok(Self { scene })
    }
}
