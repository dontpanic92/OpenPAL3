//! PAL4 plot catalog generator.
//!
//! Walks the AngelScript module (`script.csb`) plus the per-block
//! EVF/GOB/NPC data and emits a JSON map of:
//!
//! ```text
//! scenes.<scene>.blocks.<block> = {
//!     entry_fn, triggers, npcs, objects, transitions, fns
//! }
//! ```
//!
//! Intended consumer is the agent driver (a Python script or an MCP
//! adapter): pair `current scene+block` from `/v1/state` with the
//! catalog to know which trigger to fire next via
//! `/v1/scene/fire_trigger`, which globals gate it (`fns[..].reads`),
//! and which transitions cross block boundaries.
//!
//! The walker is a *very* small abstract interpreter — it only tracks
//! integer / string-literal pushes onto the operand stack so it can
//! recover the literal args to `giArenaLoad` and similar sysfns.
//! Anything more complex flushes the stack; we'd rather emit an empty
//! `transitions` list than a false one.

use std::{
    collections::BTreeMap,
    fs::File,
    io::BufWriter,
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::Parser;
use fileformats::{
    binrw::BinRead,
    npc::NpcInfoFile,
    pal4::{
        evf::{EvfEvent, EvfFile},
        gob::{GobFile, GobObjectType},
    },
};
use common::store_ext::StoreExt2;
use packfs::init_virtual_fs;
use serde::Serialize;
use shared::{
    openpal4::{app_context::Pal4AppContext, scripting::create_context},
    scripting::angelscript::{
        disasm, AsInst, AsInstInstance, ScriptGlobalContext, ScriptGlobalFunction, ScriptModule,
    },
};

#[derive(Parser)]
#[command(about = "Generate a PAL4 plot catalog (JSON)")]
struct Cli {
    /// PAL4 install root (the directory containing `gamedata/`).
    #[arg(long)]
    root: PathBuf,

    /// Output JSON path.
    #[arg(long, default_value = "pal4_plot.json")]
    out: PathBuf,

    /// If set, dump the catalog as pretty-printed JSON instead of the
    /// compact one-line form. Pretty is much larger but easier to
    /// diff by hand.
    #[arg(long)]
    pretty: bool,

    /// Debug: print the disassembly of every function whose name
    /// matches this string in the per-scene module, then exit.
    /// Used to refine the abstract walker.
    #[arg(long)]
    debug_fn: Option<String>,
}

// ---- output schema ------------------------------------------------------

#[derive(Serialize, Default)]
struct Catalog {
    version: u32,
    sysfn_count: usize,
    global_count: usize,
    scenes: BTreeMap<String, Scene>,
}

#[derive(Serialize, Default)]
struct Scene {
    blocks: BTreeMap<String, Block>,
    /// Per-function summary for every function in this scene's
    /// module. Blocks reference these by name via
    /// `Block.triggers[..].function` / `Block.objects[..].research_function`
    /// — kept at scene level because PAL4 scripts mostly use
    /// generic `func1234` names that don't carry a block prefix.
    fns: BTreeMap<String, FunctionSummary>,
    /// All `giArenaLoad(scene, block, …)` transitions recovered
    /// from this scene's module. Aggregated at scene level for the
    /// same reason as `fns`.
    transitions: Vec<Transition>,
}

#[derive(Serialize, Default)]
struct Block {
    entry_fn: Option<String>,
    triggers: Vec<Trigger>,
    npcs: Vec<Npc>,
    objects: Vec<Object>,
}

#[derive(Serialize)]
struct Trigger {
    name: String,
    function: String,
    center: [f32; 3],
    half_size: [f32; 3],
    shape: &'static str,
}

#[derive(Serialize)]
struct Npc {
    name: String,
    position: [f32; 3],
    default_visible: bool,
}

#[derive(Serialize)]
struct Object {
    name: String,
    kind: &'static str,
    position: [f32; 3],
    research_function: String,
}

#[derive(Serialize)]
struct Transition {
    /// `[scene, block]` of the destination, when both literals were
    /// recovered. Lone-literal cases (only scene OR only block) are
    /// dropped.
    to: [String; 2],
    /// The function the transition lives in (so the agent can match
    /// it back to a trigger or scripted cutscene).
    via_fn: String,
}

#[derive(Serialize, Default)]
struct FunctionSummary {
    reads: Vec<u32>,
    writes: Vec<u32>,
    /// Distinct sysfn names called in this function, in encounter
    /// order. Useful for spotting `giNewArenaLoad`, `giPlayMov`,
    /// `giTalk`, … patterns at a glance.
    sysfns: Vec<String>,
}

// ---- main ---------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();

    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("resolve root: {}", cli.root.display()))?;
    eprintln!("Mounting PAL4 vfs from {}", root.display());
    let vfs = init_virtual_fs(&root, None);

    // Sysfn ordinal table comes from the same registration that the
    // live game uses — keeps the catalog in sync without a parallel
    // hard-coded list. Independent of which per-scene module we walk.
    let ctx: ScriptGlobalContext<Pal4AppContext> = create_context();
    let sysfns: Vec<String> = ctx
        .functions()
        .iter()
        .map(|f: &ScriptGlobalFunction<Pal4AppContext>| f.name.clone())
        .collect();
    eprintln!("Sysfn table: {} entries", sysfns.len());

    // Each scene has its own per-scene module under
    // `/gamedata/script/<Scene>.csb`; the engine loads
    // `script.csb` (the bootstrap) plus the active scene's module
    // on demand via `AssetLoader::load_script_module(scene_name)`.
    // We process every per-scene module so the catalog covers every
    // scene without the agent having to know the file layout.
    let script_dir = root.join("gamedata").join("script");
    let mut module_files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&script_dir)
        .with_context(|| format!("read_dir {}", script_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref()
            != Some("csb")
        {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        if stem.is_empty() {
            continue;
        }
        module_files.push((stem, path));
    }
    module_files.sort_by(|a, b| a.0.cmp(&b.0));
    eprintln!("Found {} script modules in {}", module_files.len(), script_dir.display());

    // The bootstrap module sets the canonical `global_count` for the
    // header. Other modules are per-scene and may carry their own
    // (typically zero) global counts.
    let mut bootstrap_global_count = 0usize;

    let mut catalog = Catalog {
        version: 1,
        sysfn_count: sysfns.len(),
        global_count: 0,
        scenes: BTreeMap::new(),
    };

    for (stem, path) in &module_files {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read {}", path.display()))?;
        let module = match ScriptModule::read_from_buffer(&bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("WARN: parse {} failed: {:#}", path.display(), e);
                continue;
            }
        };
        eprintln!(
            "  {}: {} fns, {} strings, {} globals",
            stem,
            module.functions.len(),
            module.strings.len(),
            module.globals.len()
        );
        if stem.eq_ignore_ascii_case("script") {
            bootstrap_global_count = module.globals.len();
            // The bootstrap doesn't carry `*_init` handlers; skip
            // block discovery for it.
            continue;
        }

        if let Some(needle) = cli.debug_fn.as_deref() {
            for func in module.functions.iter() {
                if func.name.contains(needle) {
                    eprintln!("--- {} :: {} ---", stem, func.name);
                    eprintln!("strings[..16]: {:?}", &module.strings[..16.min(module.strings.len())]);
                    for inst in disasm(func) {
                        eprintln!("{:04x}: {:?}", inst.addr, inst.inst);
                    }
                }
            }
            continue;
        }

        // Discover (scene, block) pairs inside this module.
        let mut blocks: BTreeMap<String, ()> = BTreeMap::new();
        for func in module.functions.iter() {
            if let Some((_scene, block)) = parse_init_name(&func.name) {
                blocks.insert(block.to_string(), ());
            }
        }
        if blocks.is_empty() {
            eprintln!("    no `*_init` fns in {} — skipped", stem);
            continue;
        }

        // The on-disk filename is the canonical scene name (lowercased
        // to match the directory layout under `gamedata/scenedata/`).
        let scene_name = stem.to_ascii_lowercase();
        let mut scene_out = Scene::default();

        // Walk every function in the module *once* — PAL4 scripts
        // mostly use generic `funcNNNN` names that don't carry the
        // block prefix, so we keep summaries at scene scope and let
        // blocks reference them by name through their EVF / GOB
        // handler fields.
        for func in module.functions.iter() {
            let insts = disasm(func);
            let summary = walk_function(
                &insts,
                &module,
                &sysfns,
                &func.name,
                &mut scene_out.transitions,
            );
            scene_out.fns.insert(func.name.clone(), summary);
        }

        for (block_name, _) in &blocks {
            match build_block(&vfs, &module, &scene_name, block_name) {
                Ok(block) => {
                    scene_out.blocks.insert(block_name.clone(), block);
                }
                Err(e) => {
                    eprintln!(
                        "    WARN: scene={} block={} skipped: {:#}",
                        scene_name, block_name, e
                    );
                }
            }
        }
        catalog
            .scenes
            .insert(scene_name, scene_out);
    }

    catalog.global_count = bootstrap_global_count;
    eprintln!(
        "Catalog: {} scenes / {} blocks / {} sysfns / {} globals (bootstrap)",
        catalog.scenes.len(),
        catalog.scenes.values().map(|s| s.blocks.len()).sum::<usize>(),
        catalog.sysfn_count,
        catalog.global_count
    );

    let file = File::create(&cli.out).with_context(|| format!("create {}", cli.out.display()))?;
    let mut writer = BufWriter::new(file);
    if cli.pretty {
        serde_json::to_writer_pretty(&mut writer, &catalog)?;
    } else {
        serde_json::to_writer(&mut writer, &catalog)?;
    }
    eprintln!("Wrote {}", cli.out.display());
    Ok(())
}

// ---- per-block assembly -------------------------------------------------

fn build_block(
    vfs: &mini_fs::MiniFs,
    module: &ScriptModule,
    scene: &str,
    block: &str,
) -> Result<Block> {
    let entry_fn = format!("{}_{}_init", scene, block);

    let triggers = load_evf(vfs, scene, block).map(build_triggers).unwrap_or_default();
    let (npcs, npc_err) = match load_npc_info(vfs, scene, block) {
        Ok(n) => (build_npcs(&n), None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let (objects, obj_err) = match load_gob(vfs, scene, block) {
        Ok(g) => (build_objects(&g), None),
        Err(e) => (Vec::new(), Some(e)),
    };
    if let Some(e) = npc_err {
        eprintln!("    npc_info missing for {}/{}: {:#}", scene, block, e);
    }
    if let Some(e) = obj_err {
        eprintln!("    gob missing for {}/{}: {:#}", scene, block, e);
    }

    let entry_fn_present = module
        .functions
        .iter()
        .any(|f| f.name == entry_fn);

    Ok(Block {
        entry_fn: entry_fn_present.then(|| entry_fn),
        triggers,
        npcs,
        objects,
    })
}

fn load_evf(vfs: &mini_fs::MiniFs, scene: &str, block: &str) -> Result<EvfFile> {
    let path = format!(
        "/gamedata/scenedata/{}/{}/{}.evf",
        scene, block, block
    );
    let bytes = vfs.read_to_end(&path)?;
    Ok(EvfFile::read(&mut std::io::Cursor::new(bytes))?)
}

fn load_gob(vfs: &mini_fs::MiniFs, scene: &str, block: &str) -> Result<GobFile> {
    let path = format!(
        "/gamedata/scenedata/{}/{}/GameObjs.gob",
        scene, block
    );
    let bytes = vfs.read_to_end(&path)?;
    Ok(GobFile::read(&mut std::io::Cursor::new(bytes))?)
}

fn load_npc_info(vfs: &mini_fs::MiniFs, scene: &str, block: &str) -> Result<NpcInfoFile> {
    let path = format!(
        "/gamedata/scenedata/{}/{}/npcInfo.npc",
        scene, block
    );
    let bytes = vfs.read_to_end(&path)?;
    Ok(NpcInfoFile::read(&mut std::io::Cursor::new(bytes))?)
}

fn build_triggers(evf: EvfFile) -> Vec<Trigger> {
    evf.events
        .into_iter()
        .map(|event| {
            let shape: &'static str = match event.vertex_count {
                4 => "plane",
                8 => "box",
                _ => "other",
            };
            let (center, half_size) = trigger_bounds(&event);
            Trigger {
                name: event.name.to_string().unwrap_or_default(),
                function: event
                    .function
                    .function
                    .to_string()
                    .unwrap_or_default(),
                center,
                half_size,
                shape,
            }
        })
        .collect()
}

fn trigger_bounds(event: &EvfEvent) -> ([f32; 3], [f32; 3]) {
    if event.vertices.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    let first = &event.vertices[0];
    let mut min = [
        first.center.x as f32,
        first.center.y as f32,
        first.center.z as f32,
    ];
    let mut max = min;
    for v in event.vertices.iter().skip(1) {
        let p = [v.center.x as f32, v.center.y as f32, v.center.z as f32];
        for i in 0..3 {
            if p[i] < min[i] {
                min[i] = p[i];
            }
            if p[i] > max[i] {
                max[i] = p[i];
            }
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let half_size = [
        (max[0] - min[0]) * 0.5,
        (max[1] - min[1]) * 0.5,
        (max[2] - min[2]) * 0.5,
    ];
    (center, half_size)
}

fn build_npcs(info: &NpcInfoFile) -> Vec<Npc> {
    info.data
        .iter()
        .map(|n| Npc {
            name: n.name.to_string().unwrap_or_default(),
            position: n.position,
            default_visible: n.default_visible == 1,
        })
        .collect()
}

fn build_objects(gob: &GobFile) -> Vec<Object> {
    gob.entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let kind = gob
                .header
                .object_types
                .get(i)
                .and_then(|t| GobObjectType::name(*t))
                .unwrap_or("unknown");
            Object {
                name: entry.name.to_string().unwrap_or_default(),
                kind,
                position: entry.position,
                research_function: entry
                    .research_function
                    .to_string()
                    .unwrap_or_default(),
            }
        })
        .collect()
}

// ---- abstract walker ----------------------------------------------------

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum Abs {
    Int(i32),
    Str(u16),
}

fn walk_function(
    insts: &[AsInstInstance],
    module: &ScriptModule,
    sysfns: &[String],
    fn_name: &str,
    transitions: &mut Vec<Transition>,
) -> FunctionSummary {
    let mut summary = FunctionSummary::default();
    let mut stack: Vec<Abs> = Vec::with_capacity(8);
    let mut sysfn_seen: BTreeMap<String, ()> = BTreeMap::new();

    // PAL4 scripts marshal a `giArenaLoad(scn, block, data, show)` call
    // through three string locals (one per string arg) plus the int
    // `show` directly. The compiler emits, per string arg:
    //
    //     Set4 { data: <param_index> }      // sometimes; not load-bearing
    //     Str { index: <strings[i]> }
    //     CallSys -19 (string@)             // construct AS string handle
    //     StoreObj { param_index: N }       // local[N] = handle
    //
    // ...then `Set4 { data: <show> }` and `GetObjRef 0/1/2` push the
    // locals onto the operand stack in StoreObj order before the final
    // `CallSys giArenaLoad`. Tracking the order of string-slot writes
    // is enough to recover the literal (scn, block) destination: AS
    // pops in order `scn, block, data, show`, which means the last
    // string written (top of the operand stack) is `scn` and the
    // one before it is `block`.
    let mut last_str_slot: Option<(u16, i16)> = None; // (string_idx, param_index) just after string@
    let mut recent_writes: Vec<u16> = Vec::with_capacity(4); // string-table indices of last few StoreObj writes

    for inst in insts {
        match &inst.inst {
            AsInst::Str { index } => {
                stack.push(Abs::Str(*index));
                last_str_slot = Some((*index, -1));
            }
            AsInst::Set4 { data } => stack.push(Abs::Int(*data as i32)),
            AsInst::PushZero => stack.push(Abs::Int(0)),
            AsInst::StoreObj { param_index: _ } => {
                if let Some((string_idx, _)) = last_str_slot.take() {
                    recent_writes.push(string_idx);
                    // Keep at most the last 4 string-slot writes;
                    // giArenaLoad needs only the most recent 3.
                    if recent_writes.len() > 4 {
                        recent_writes.remove(0);
                    }
                }
                stack.clear();
            }
            AsInst::Rdga4 { index } => {
                if *index >= 0 {
                    let i = *index as u32;
                    if !summary.reads.contains(&i) {
                        summary.reads.push(i);
                    }
                }
                stack.clear();
            }
            AsInst::Movga4 { index } => {
                if *index >= 0 {
                    let i = *index as u32;
                    if !summary.writes.contains(&i) {
                        summary.writes.push(i);
                    }
                }
                stack.clear();
            }
            AsInst::CallSys { function_index } => {
                // The AS VM encodes sysfn ordinals as `-(idx) - 1`
                // (see `ScriptVm::callsys` in
                // shared/src/scripting/angelscript/vm.rs:525-526).
                let idx = (-*function_index - 1) as usize;
                let name = sysfns
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| format!("?sys{}", idx));
                if !sysfn_seen.contains_key(&name) {
                    sysfn_seen.insert(name.clone(), ());
                    summary.sysfns.push(name.clone());
                }
                if name == "string@" {
                    // `string@` is the AS string constructor and a
                    // helper for our purposes — keep the operand
                    // stack untouched so the subsequent StoreObj
                    // can pair it with its slot.
                    continue;
                }
                if name == "giArenaLoad" && recent_writes.len() >= 2 {
                    let n = recent_writes.len();
                    // AS pop order = scn, block, data, show. The
                    // last string write therefore corresponds to
                    // `scn`; the one before it to `block`.
                    let scn_idx = recent_writes[n - 1];
                    let blk_idx = recent_writes[n - 2];
                    let scn = module.strings.get(scn_idx as usize).cloned();
                    let blk = module.strings.get(blk_idx as usize).cloned();
                    if let (Some(s), Some(b)) = (scn, blk) {
                        transitions.push(Transition {
                            to: [s, b],
                            via_fn: fn_name.to_string(),
                        });
                    }
                }
                stack.clear();
                last_str_slot = None;
                // Reset on any block boundary so unrelated string
                // assignments earlier in the function don't feed a
                // later giArenaLoad.
                if matches!(name.as_str(), "giArenaLoad" | "giArenaReady") {
                    recent_writes.clear();
                }
            }
            // Anything else: be conservative — flush the operand stack
            // so we don't reuse stale literals across control flow.
            _ => {
                stack.clear();
                last_str_slot = None;
            }
        }
    }

    summary
}

fn parse_init_name(name: &str) -> Option<(&str, &str)> {
    let body = name.strip_suffix("_init")?;
    let (scene, block) = body.rsplit_once('_')?;
    if scene.is_empty() || block.is_empty() {
        return None;
    }
    if !scene
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        || !block.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((scene, block))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_init_name_basic() {
        assert_eq!(parse_init_name("q01_01_init"), Some(("q01", "01")));
        assert_eq!(parse_init_name("m01_02_init"), Some(("m01", "02")));
    }

    #[test]
    fn parse_init_name_rejects_non_init() {
        assert_eq!(parse_init_name("q01_01_main"), None);
        assert_eq!(parse_init_name("setup_init"), None); // no scene/block split
    }
}
