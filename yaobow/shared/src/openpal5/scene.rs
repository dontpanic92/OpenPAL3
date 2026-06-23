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
        // directional sun. PAL5 ships no per-scene `.lgt`; its terrain is
        // dynamically lit (the splat shader applies Lambert + ambient), so
        // without this the lighting term would collapse to a flat, dead
        // ambient. We model the sun as a far point light (no attenuation)
        // in the parsed azimuth/elevation direction. If the `.env` is
        // missing, fall back to full ambient so terrain never renders black.
        match asset_loader.load_map_env(scene_name) {
            Some(env) => {
                let dir = env.sun_direction();
                const SUN_DISTANCE: f32 = 1.0e7;
                let sun = radiance::scene::SceneLight::new(
                    Vec3::new(
                        dir[0] * SUN_DISTANCE,
                        dir[1] * SUN_DISTANCE,
                        dir[2] * SUN_DISTANCE,
                    ),
                    env.sun_color,
                );
                scene.set_lighting(radiance::scene::SceneLighting::new(env.ambient, vec![sun]));
                // Fog color/params are decoded and carried on `env` but not
                // yet pushed to the renderer (radiance has no fog path); log
                // them so per-map atmosphere is observable in traces.
                log::info!(
                    "Pal5Scene '{}': atmosphere ambient {:?} sun {:?} az={} el={} dir {:?} | fog {:?} a={} b={} tag={}",
                    scene_name,
                    env.ambient,
                    env.sun_color,
                    env.sun_azimuth_deg,
                    env.sun_elevation_deg,
                    dir,
                    env.fog_color,
                    env.fog_param_a,
                    env.fog_param_b,
                    env.build_tag,
                );
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

        Ok(Self { scene })
    }
}
