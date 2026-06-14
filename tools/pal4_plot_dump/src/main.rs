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

use std::{collections::BTreeMap, fs::File, io::BufWriter, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use common::store_ext::StoreExt2;
use fileformats::{
    binrw::BinRead,
    npc::NpcInfoFile,
    pal4::{
        evf::{EvfEvent, EvfFile},
        gob::{GobFile, GobObjectType},
    },
};
use packfs::init_virtual_fs;
use serde::Serialize;
use shared::{
    openpal4::{scripting::create_context, vm_context::Pal4VmContext},
    scripting::angelscript::{
        AsInst, AsInstInstance, ScriptGlobalContext, ScriptGlobalFunction, ScriptModule, disasm,
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

/// Schema version recorded in the catalog's top-level `version` field.
/// Bump on every shape-breaking change so downstream loaders can
/// detect stale dumps. v2 adds `fns[..].cmp_literals` for gate-
/// predicate inference; see `docs/pal4_plot_catalog.md`.
const CATALOG_VERSION: u32 = 2;

#[derive(Serialize, Default)]
struct Catalog {
    version: u32,
    sysfn_count: usize,
    global_count: usize,
    scenes: BTreeMap<String, Scene>,
    /// Reverse index "to advance global[i] to value V, fire one of
    /// these triggers" — see the `Plot advancement` section of
    /// `docs/pal4_plot_catalog.md`. Built from the per-trigger /
    /// per-research_function call-graph closure (`TriggerSummary.writes`).
    /// Keys are stringified slot indices (JSON object keys are
    /// strings); values are sorted by `(scene, block, trigger, fn)`.
    plot_index: BTreeMap<String, Vec<PlotIndexEntry>>,
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
    /// Set when a `.csb` parsed cleanly but carried no `*_init`
    /// fns — i.e. the scene is reachable from elsewhere via
    /// `giArenaLoad` but doesn't define its own playable blocks
    /// (worldMap, M02, …). Lets agents following the transition
    /// list see *something* under the scene key instead of nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Serialize, Default)]
struct Block {
    entry_fn: Option<String>,
    triggers: Vec<Trigger>,
    npcs: Vec<Npc>,
    objects: Vec<Object>,
    /// `true` when this block entry was synthesised because some
    /// other block's `transitions` referenced `(scene, block)` but
    /// no `*_init` fn produced an entry. The block's
    /// `triggers/npcs/objects` are intentionally empty — consumers
    /// should fall back to live `/v1/scene/triggers` data once the
    /// engine actually loads the block. Resolves the "missing
    /// blocks" half of issue C1 in `generated/issues.md`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    synthesized: bool,
}

#[derive(Serialize)]
struct Trigger {
    name: String,
    function: String,
    center: [f32; 3],
    half_size: [f32; 3],
    shape: &'static str,
    /// Coarse classification: `"trigger"` for entries with a bound
    /// script function, `"wall"` for collision / camera-helper
    /// volumes (empty function + `shape == "other"`). The live
    /// engine itself skips `"wall"` entries when building collision
    /// — agents should use this field rather than re-deriving the
    /// same heuristic on the consumer side.
    kind: &'static str,
    /// Aggregated call-graph closure starting from `function`. See
    /// `TriggerSummary` for field semantics; flattened inline here so
    /// agents only need to inspect one struct per trigger.
    #[serde(flatten)]
    summary: TriggerSummary,
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
    /// Same call-graph closure as `Trigger.summary`, computed from
    /// `research_function` when it's non-empty. Empty `TriggerSummary`
    /// (no called_fns / no reads / no writes / no transitions) when
    /// the entry has no examine handler.
    #[serde(flatten)]
    summary: TriggerSummary,
}

#[derive(Serialize, Clone)]
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
    /// Shared-global slot indices read by this fn (decoded from
    /// negative `Rdga4` indices via `(-index - 1)`; positive indices
    /// reference module-local globals and are *not* part of the plot
    /// vector, so they are skipped).
    reads: Vec<u32>,
    /// Shared-global writes. `value` is `Some(v)` when the value
    /// stored is a literal `Set4 v` / `PushZero` immediately preceding
    /// the `Movga4` — i.e. when the catalog can prove what the fn
    /// writes. `None` when the value is computed (read from another
    /// global, arithmetic, function return, …).
    writes: Vec<ValueWrite>,
    /// Distinct sysfn names called in this function, in encounter
    /// order. Useful for spotting `giNewArenaLoad`, `giPlayMov`,
    /// `giTalk`, … patterns at a glance.
    sysfns: Vec<String>,
    /// Distinct module-local function names called via `Call`
    /// (opcode 12). Used to seed the per-trigger closure walker.
    calls: Vec<String>,
    /// Recovered "RHS literal" for `Jz` / `Jnz` instructions that
    /// follow the standard `Rdga4 slot ; Set4 V ; Subi|Cmpi ; Jcc`
    /// gate pattern. Map key is the **PC of the `Jcc` instruction**
    /// (matches `TraceEvent::Branch.pc`); value is the literal `V`
    /// the script compared the global against, or `None` when the
    /// RHS was computed. Lets the planner populate
    /// `gates.inferred_required_value` without re-reading the
    /// bytecode at agent-runtime. See `docs/pal4_planner.md`
    /// "Gate inference".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    cmp_literals: BTreeMap<u32, Option<i32>>,
}

/// A single concrete write to a shared global.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
struct ValueWrite {
    global: u32,
    value: Option<i32>,
}

/// Aggregated view of the call-graph closure starting from a trigger's
/// entry fn (or an object's `research_function`). All fields are the
/// union over the reachable fn set, computed by BFS-following `Call`
/// edges within the same module (capped at `CALL_CLOSURE_DEPTH` fns to
/// bound runtime — the catalog is a planning hint, not a simulator).
#[derive(Serialize, Default)]
struct TriggerSummary {
    /// Fn names visited during the closure walk, in BFS order.
    /// Always non-empty when a handler exists; first entry is the
    /// entry fn itself.
    called_fns: Vec<String>,
    /// Union of `FunctionSummary.reads` across `called_fns`.
    reads: Vec<u32>,
    /// Union of `FunctionSummary.writes` across `called_fns`,
    /// deduped by `(global, value)`.
    writes: Vec<ValueWrite>,
    /// `[scene, block]` destinations reachable via a `giArenaLoad`
    /// somewhere in the closure (deduped).
    transitions: Vec<[String; 2]>,
}

/// One row of `Catalog.plot_index` — answers "this trigger advances
/// `global[i]` to value `v`". `trigger` is `None` for entries seeded
/// from a fn that no live trigger references (init handlers, scripted
/// cutscenes invoked only by other fns); chase the `fn` field through
/// the per-scene `transitions` / `fns` tables to find a fireable
/// ancestor.
#[derive(Serialize, Clone, Debug)]
struct PlotIndexEntry {
    value: Option<i32>,
    scene: String,
    block: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    object: Option<String>,
    r#fn: String,
}

/// BFS depth cap for the per-trigger call-graph closure. PAL4 trigger
/// handlers chain at most 3–4 deep in practice; 16 leaves plenty of
/// headroom while bounding worst-case fan-out on weird modules.
const CALL_CLOSURE_DEPTH: usize = 16;

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
    let ctx: ScriptGlobalContext<Pal4VmContext> = create_context();
    let sysfns: Vec<String> = ctx
        .functions()
        .iter()
        .map(|f: &ScriptGlobalFunction<Pal4VmContext>| f.name.clone())
        .collect();
    eprintln!("Sysfn table: {} entries", sysfns.len());

    // Each scene has its own per-scene module under
    // `/gamedata/script/<Scene>.csb`; the engine loads
    // `script.csb` (the bootstrap) plus the active scene's module
    // on demand via `AssetLoader::load_script_module(scene_name)`.
    // We process every per-scene module so the catalog covers every
    // scene without the agent having to know the file layout.
    //
    // List the modules through the *vfs*, not the host filesystem —
    // PAL4 ships scripts inside `script.cpk` on Steam (and Origin),
    // so requiring an extracted `gamedata/script/` would lock the
    // dumper to one specific install layout. `mini_fs::Store`'s
    // `entries_path` walks the same merged store the engine uses, so
    // the dumper works on either layout.
    let mut module_files: Vec<String> = Vec::new();
    let script_dir = std::path::Path::new("/gamedata/script");
    let entries = <mini_fs::MiniFs as mini_fs::Store>::entries_path(&vfs, script_dir)
        .with_context(|| format!("list vfs {}", script_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        if !matches!(entry.kind, mini_fs::EntryKind::File) {
            continue;
        }
        // `Entry.name` is store-dependent: CpkFs hands back just the
        // basename ("Q10.csb"), while LocalFs hands back the relative
        // path from the mount root ("gamedata\script\Q10.csb"). Strip
        // back to the file_name in either case so the subsequent
        // `/gamedata/script/<stem>.csb` build is correct on both
        // layouts.
        let basename = std::path::Path::new(&entry.name)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.name.to_string_lossy().into_owned());
        let lower = basename.to_ascii_lowercase();
        if !lower.ends_with(".csb") {
            continue;
        }
        let stem = basename[..basename.len() - 4].to_string();
        if stem.is_empty() {
            continue;
        }
        module_files.push(stem);
    }
    module_files.sort();
    module_files.dedup();
    eprintln!(
        "Found {} script modules under vfs {}",
        module_files.len(),
        script_dir.display()
    );

    // The bootstrap module sets the canonical `global_count` for the
    // header. Other modules are per-scene and may carry their own
    // (typically zero) global counts.
    let mut bootstrap_global_count = 0usize;

    let mut catalog = Catalog {
        version: CATALOG_VERSION,
        sysfn_count: sysfns.len(),
        global_count: 0,
        scenes: BTreeMap::new(),
        plot_index: BTreeMap::new(),
    };

    for stem in &module_files {
        let vfs_path = format!("/gamedata/script/{}.csb", stem);
        let bytes = match vfs.read_to_end(&vfs_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("WARN: read {} failed: {:#}", vfs_path, e);
                continue;
            }
        };
        let module = match ScriptModule::read_from_buffer(&bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("WARN: parse {} failed: {:#}", vfs_path, e);
                // Still surface the scene so agents following a
                // transition list see *something* under
                // `scenes.<name>` instead of nothing. M02 is the
                // canonical example: 5855 bytes, fails to parse,
                // but reachable via giArenaLoad from q01.func1007.
                let scene_name = stem.to_ascii_lowercase();
                catalog.scenes.insert(
                    scene_name,
                    Scene {
                        note: Some(format!(
                            "parse failure: {} — scene reachable via giArenaLoad but its script is unavailable",
                            e
                        )),
                        ..Scene::default()
                    },
                );
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
                    eprintln!(
                        "strings[..16]: {:?}",
                        &module.strings[..16.min(module.strings.len())]
                    );
                    for inst in disasm(func) {
                        eprintln!("{:04x}: {:?}", inst.addr, inst.inst);
                    }
                }
            }
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

        // Discover (scene, block) pairs inside this module.
        let mut blocks: BTreeMap<String, ()> = BTreeMap::new();
        for func in module.functions.iter() {
            if let Some((_scene, block)) = parse_init_name(&func.name) {
                blocks.insert(block.to_string(), ());
            }
        }
        // Also discover blocks via outgoing transitions to this scene
        // (e.g. Q01/Q01#ev_Q01_Q01_1 transitions to Q01/N02 even
        // though no `Q01_N02_init` fn exists in the module). The
        // EVF/GOB files live on disk under
        // `gamedata/scenedata/<scene>/<block>/` — `build_block` will
        // pick them up just like for init-discovered blocks. This
        // closes the gap that previously left N02/N03/Q01Y as empty
        // "synthesized" stubs even when their scene data was present
        // on disk; without these the planner can't see, e.g., the
        // trigger in Q01/N02 that writes g[0]=11500.
        for tr in &scene_out.transitions {
            if tr.to[0].eq_ignore_ascii_case(&scene_name) {
                blocks.entry(tr.to[1].clone()).or_insert(());
            }
        }
        if blocks.is_empty() {
            // Still emit a stub so agents following the per-scene
            // transition list have *something* to land on under
            // `scenes.<name>` — the scene is reachable via giArenaLoad
            // from another module (worldMap, M02, …), it just doesn't
            // define playable blocks of its own.
            eprintln!(
                "    no `*_init` fns in {} — emitting stub (fns/transitions only)",
                stem
            );
            scene_out.note = Some(
                "no *_init fns; reachable only via giArenaLoad from another module".to_string(),
            );
            catalog.scenes.insert(scene_name, scene_out);
            continue;
        }

        // Snapshot transitions before iterating blocks so each block's
        // build can borrow them read-only.
        let scene_transitions_snapshot = scene_out.transitions.clone();
        for (block_name, _) in &blocks {
            match build_block(
                &vfs,
                &module,
                &scene_name,
                block_name,
                &scene_out.fns,
                &scene_transitions_snapshot,
            ) {
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
        catalog.scenes.insert(scene_name, scene_out);
    }

    catalog.global_count = bootstrap_global_count;
    synthesise_missing_blocks(&mut catalog);
    build_plot_index(&mut catalog);
    eprintln!(
        "Catalog: {} scenes / {} blocks / {} sysfns / {} globals (bootstrap) / {} plot_index entries",
        catalog.scenes.len(),
        catalog
            .scenes
            .values()
            .map(|s| s.blocks.len())
            .sum::<usize>(),
        catalog.sysfn_count,
        catalog.global_count,
        catalog.plot_index.values().map(|v| v.len()).sum::<usize>(),
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
    fns: &BTreeMap<String, FunctionSummary>,
    scene_transitions: &[Transition],
) -> Result<Block> {
    let entry_fn = format!("{}_{}_init", scene, block);

    let triggers = load_evf(vfs, scene, block)
        .map(|evf| build_triggers(evf, fns, scene_transitions))
        .unwrap_or_default();
    let (npcs, npc_err) = match load_npc_info(vfs, scene, block) {
        Ok(n) => (build_npcs(&n), None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let (objects, obj_err) = match load_gob(vfs, scene, block) {
        Ok(g) => (build_objects(&g, fns, scene_transitions), None),
        Err(e) => (Vec::new(), Some(e)),
    };
    if let Some(e) = npc_err {
        eprintln!("    npc_info missing for {}/{}: {:#}", scene, block, e);
    }
    if let Some(e) = obj_err {
        eprintln!("    gob missing for {}/{}: {:#}", scene, block, e);
    }

    let entry_fn_present = module.functions.iter().any(|f| f.name == entry_fn);

    Ok(Block {
        entry_fn: entry_fn_present.then(|| entry_fn),
        triggers,
        npcs,
        objects,
        synthesized: false,
    })
}

fn load_evf(vfs: &mini_fs::MiniFs, scene: &str, block: &str) -> Result<EvfFile> {
    let path = format!("/gamedata/scenedata/{}/{}/{}.evf", scene, block, block);
    let bytes = vfs.read_to_end(&path)?;
    Ok(EvfFile::read(&mut std::io::Cursor::new(bytes))?)
}

fn load_gob(vfs: &mini_fs::MiniFs, scene: &str, block: &str) -> Result<GobFile> {
    let path = format!("/gamedata/scenedata/{}/{}/GameObjs.gob", scene, block);
    let bytes = vfs.read_to_end(&path)?;
    Ok(GobFile::read(&mut std::io::Cursor::new(bytes))?)
}

fn load_npc_info(vfs: &mini_fs::MiniFs, scene: &str, block: &str) -> Result<NpcInfoFile> {
    let path = format!("/gamedata/scenedata/{}/{}/npcInfo.npc", scene, block);
    let bytes = vfs.read_to_end(&path)?;
    Ok(NpcInfoFile::read(&mut std::io::Cursor::new(bytes))?)
}

fn build_triggers(
    evf: EvfFile,
    fns: &BTreeMap<String, FunctionSummary>,
    scene_transitions: &[Transition],
) -> Vec<Trigger> {
    evf.events
        .into_iter()
        .map(|event| {
            let shape: &'static str = match event.vertex_count {
                4 => "plane",
                8 => "box",
                _ => "other",
            };
            let (center, half_size) = trigger_bounds(&event);
            let function = event.function.function.to_string().unwrap_or_default();
            let summary = build_trigger_summary(&function, fns, scene_transitions);
            let kind: &'static str = if function.is_empty() && shape == "other" {
                // Empty function + non-trigger shape = collision /
                // camera-helper volume (issue C4). The live engine
                // skips these; tag them so consumers can too.
                "wall"
            } else {
                "trigger"
            };
            Trigger {
                name: event.name.to_string().unwrap_or_default(),
                function,
                center,
                half_size,
                shape,
                kind,
                summary,
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

fn build_objects(
    gob: &GobFile,
    fns: &BTreeMap<String, FunctionSummary>,
    scene_transitions: &[Transition],
) -> Vec<Object> {
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
            let research_function = entry.research_function.to_string().unwrap_or_default();
            let summary = build_trigger_summary(&research_function, fns, scene_transitions);
            Object {
                name: entry.name.to_string().unwrap_or_default(),
                kind,
                position: entry.position,
                research_function,
                summary,
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

/// Scan a function's disassembly for `g[slot] cmp V` gate predicates
/// and return a map from **trace-visible PC** → recovered RHS literal.
///
/// PAL4's AngelScript bytecode encodes gates in two shapes:
///
/// ```text
///   Rdga4 -(slot+1)              // push g[slot]
///   Cmpii { rhs: V }             // pop, push (g[slot] - V) sign-result
///   Jz off  |  Jnz off           // branch on the result
/// ```
///
/// …or the long form…
///
/// ```text
///   Rdga4 -(slot+1)
///   Set4 V   |  PushZero
///   Subi     |  Cmpi
///   Jz off   |  Jnz off
/// ```
///
/// We track both with a single 3-state pipeline. Any instruction that
/// isn't part of the recognised sequence resets the state machine so a
/// stale `Set4` from earlier in the function can't be paired with a
/// much later branch.
///
/// The map key is the PC `TraceEvent::Branch.pc` reports — that's
/// `instruction.addr + 8` for `Jz`/`Jnz` (4-byte opcode + 4-byte i32
/// offset operand fetched by `ScriptVm::read_inst` and
/// `data_read::i32` before the handler is invoked in
/// `yaobow/shared/src/scripting/angelscript/vm.rs`). Value is the
/// literal `V`, or `None` when the RHS was computed (e.g. read from
/// another global).
fn extract_cmp_literals(insts: &[AsInstInstance]) -> BTreeMap<u32, Option<i32>> {
    enum GateState {
        Idle,
        AfterRdga4,
        AfterCmp(Option<i32>),
    }
    let mut state = GateState::Idle;
    // Long-form scratch: when we see `Rdga4 ; Set4 V` we stash V here
    // and confirm it on the following `Subi`/`Cmpi`. Reset on any
    // out-of-pattern instruction.
    let mut pending_lit: Option<i32> = None;
    let mut out = BTreeMap::new();

    for inst in insts {
        state = match (&state, &inst.inst) {
            (_, AsInst::Rdga4 { index }) if *index < 0 => {
                pending_lit = None;
                GateState::AfterRdga4
            }
            // Short form: compare-immediate folds the literal into the
            // opcode itself.
            (GateState::AfterRdga4, AsInst::Cmpii { rhs })
            | (GateState::AfterRdga4, AsInst::Subii { rhs }) => GateState::AfterCmp(Some(*rhs)),
            (GateState::AfterRdga4, AsInst::Cmpiui { rhs }) => {
                GateState::AfterCmp(Some(*rhs as i32))
            }
            // Long form: a separate literal push followed by Subi/Cmpi.
            (GateState::AfterRdga4, AsInst::Set4 { data }) => {
                pending_lit = Some(*data as i32);
                GateState::AfterRdga4
            }
            (GateState::AfterRdga4, AsInst::PushZero) => {
                pending_lit = Some(0);
                GateState::AfterRdga4
            }
            (GateState::AfterRdga4, AsInst::Subi) | (GateState::AfterRdga4, AsInst::Cmpi) => {
                let v = pending_lit.take();
                GateState::AfterCmp(v)
            }
            (GateState::AfterCmp(v), AsInst::Jz { .. })
            | (GateState::AfterCmp(v), AsInst::Jnz { .. }) => {
                let trace_pc = inst.addr + 8;
                out.entry(trace_pc).or_insert(*v);
                pending_lit = None;
                GateState::Idle
            }
            _ => {
                pending_lit = None;
                GateState::Idle
            }
        };
    }
    out
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
    let mut call_seen: BTreeMap<String, ()> = BTreeMap::new();

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

    summary.cmp_literals = extract_cmp_literals(insts);

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
                // Shared globals (the ones in /v1/script/globals and
                // the ones Pal4PersistentState serialises) are encoded
                // as -(slot + 1) — see ScriptVm::rdga4 in
                // yaobow/shared/src/scripting/angelscript/vm.rs:514.
                // Positive indices reference *module-local* globals
                // and are not part of the plot vector; skip those.
                if *index < 0 {
                    let slot = (-*index - 1) as u32;
                    if !summary.reads.contains(&slot) {
                        summary.reads.push(slot);
                    }
                }
                // Reading a global pushes its value onto the operand
                // stack; we can't track the runtime value, so clear.
                stack.clear();
                last_str_slot = None;
            }
            AsInst::Movga4 { index } => {
                if *index < 0 {
                    let slot = (-*index - 1) as u32;
                    // Peek the top-of-stack *before* clearing: if it's
                    // a literal we just pushed (Set4 / PushZero), we
                    // know exactly what value the fn writes. Otherwise
                    // the value is computed and we record `None`.
                    let value = match stack.last() {
                        Some(Abs::Int(v)) => Some(*v),
                        _ => None,
                    };
                    let entry = ValueWrite {
                        global: slot,
                        value,
                    };
                    if !summary.writes.contains(&entry) {
                        summary.writes.push(entry);
                    }
                }
                stack.clear();
                last_str_slot = None;
            }
            AsInst::Call { function } => {
                let name = module
                    .functions
                    .get(*function as usize)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| format!("?call{}", function));
                if !call_seen.contains_key(&name) {
                    call_seen.insert(name.clone(), ());
                    summary.calls.push(name);
                }
                stack.clear();
                last_str_slot = None;
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

// ---- per-trigger closure ------------------------------------------------

/// BFS the intra-module call graph starting from `entry_fn`, unioning
/// `FunctionSummary` fields and `giArenaLoad` destinations across every
/// reachable fn. Bounded by `CALL_CLOSURE_DEPTH` fns total (not depth)
/// to keep runtime predictable on weird modules.
fn build_trigger_summary(
    entry_fn: &str,
    fns: &BTreeMap<String, FunctionSummary>,
    scene_transitions: &[Transition],
) -> TriggerSummary {
    let mut summary = TriggerSummary::default();
    if entry_fn.is_empty() || !fns.contains_key(entry_fn) {
        return summary;
    }

    let mut visited: BTreeMap<String, ()> = BTreeMap::new();
    let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    queue.push_back(entry_fn.to_string());
    visited.insert(entry_fn.to_string(), ());

    while let Some(name) = queue.pop_front() {
        if summary.called_fns.len() >= CALL_CLOSURE_DEPTH {
            break;
        }
        summary.called_fns.push(name.clone());
        let Some(fs) = fns.get(&name) else { continue };

        for r in &fs.reads {
            if !summary.reads.contains(r) {
                summary.reads.push(*r);
            }
        }
        for w in &fs.writes {
            if !summary.writes.contains(w) {
                summary.writes.push(w.clone());
            }
        }
        for callee in &fs.calls {
            if !visited.contains_key(callee) {
                visited.insert(callee.clone(), ());
                queue.push_back(callee.clone());
            }
        }
        for t in scene_transitions {
            if t.via_fn == name {
                let pair = t.to.clone();
                if !summary.transitions.contains(&pair) {
                    summary.transitions.push(pair);
                }
            }
        }
    }

    summary
}

// ---- block synthesis ----------------------------------------------------

/// Add stub entries to `Catalog.scenes` for every `(scene, block)`
/// pair some other block's recorded `transitions` references but no
/// `*_init` discovery surfaced. Resolves the "missing blocks" half
/// of issue C1 from `generated/issues.md` — without this, agents
/// chasing the catalog's own transition list dead-end on, e.g.,
/// `Q01/N03` (which is reachable via `Q01/Q01/ev_Q01_Q01_6` but had
/// no `Q01_N03_init` fn to discover it from).
///
/// Synthesised blocks set `synthesized: true` and have empty
/// `triggers/npcs/objects` vectors; consumers should fall back to
/// live `/v1/scene/triggers` data once the engine actually loads
/// the block.
fn synthesise_missing_blocks(catalog: &mut Catalog) {
    // Snapshot every referenced (scene, block) pair before
    // mutating the catalog, so the loop's reads don't race the
    // writes.
    let mut needed: Vec<(String, String)> = Vec::new();
    for (_, scene) in &catalog.scenes {
        for (_, block) in &scene.blocks {
            for trig in &block.triggers {
                for dest in &trig.summary.transitions {
                    needed.push((dest[0].clone(), dest[1].clone()));
                }
            }
        }
    }
    let mut added = 0usize;
    for (scn, blk) in needed {
        // Existing scene keys are lower-cased (see `let scene_name
        // = stem.to_ascii_lowercase()` at the discovery sites);
        // match that so we don't create case-split duplicates.
        let scn = scn.to_ascii_lowercase();
        let scene = catalog
            .scenes
            .entry(scn.clone())
            .or_insert_with(Scene::default);
        if scene.blocks.contains_key(&blk) {
            continue;
        }
        scene.blocks.insert(
            blk.clone(),
            Block {
                synthesized: true,
                ..Block::default()
            },
        );
        added += 1;
    }
    if added > 0 {
        eprintln!("Synthesised {} stub block(s) from transitions", added);
    }
}

// ---- plot_index ---------------------------------------------------------

/// Build `Catalog.plot_index` from the per-trigger / per-object closure
/// `writes`. Entries are sorted by `(scene, block, trigger.unwrap_or(""),
/// object.unwrap_or(""), fn)` so successive runs of the dumper produce
/// stable diffs.
fn build_plot_index(catalog: &mut Catalog) {
    let mut index: BTreeMap<String, Vec<PlotIndexEntry>> = BTreeMap::new();

    for (scene_name, scene) in &catalog.scenes {
        for (block_name, block) in &scene.blocks {
            for trig in &block.triggers {
                if trig.function.is_empty() {
                    continue;
                }
                for w in &trig.summary.writes {
                    index
                        .entry(w.global.to_string())
                        .or_default()
                        .push(PlotIndexEntry {
                            value: w.value,
                            scene: scene_name.clone(),
                            block: block_name.clone(),
                            trigger: Some(trig.name.clone()),
                            object: None,
                            r#fn: trig.function.clone(),
                        });
                }
            }
            for obj in &block.objects {
                if obj.research_function.is_empty() {
                    continue;
                }
                for w in &obj.summary.writes {
                    index
                        .entry(w.global.to_string())
                        .or_default()
                        .push(PlotIndexEntry {
                            value: w.value,
                            scene: scene_name.clone(),
                            block: block_name.clone(),
                            trigger: None,
                            object: Some(obj.name.clone()),
                            r#fn: obj.research_function.clone(),
                        });
                }
            }
        }
    }

    for entries in index.values_mut() {
        entries.sort_by(|a, b| {
            (
                &a.scene,
                &a.block,
                a.trigger.as_deref().unwrap_or(""),
                a.object.as_deref().unwrap_or(""),
                &a.r#fn,
                a.value,
            )
                .cmp(&(
                    &b.scene,
                    &b.block,
                    b.trigger.as_deref().unwrap_or(""),
                    b.object.as_deref().unwrap_or(""),
                    &b.r#fn,
                    b.value,
                ))
        });
        entries.dedup_by(|a, b| {
            a.scene == b.scene
                && a.block == b.block
                && a.trigger == b.trigger
                && a.object == b.object
                && a.r#fn == b.r#fn
                && a.value == b.value
        });
    }

    catalog.plot_index = index;
}

fn parse_init_name(name: &str) -> Option<(&str, &str)> {
    let body = name.strip_suffix("_init")?;
    let (scene, block) = body.rsplit_once('_')?;
    if scene.is_empty() || block.is_empty() {
        return None;
    }
    if !scene.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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

    fn fn_with_writes(writes: Vec<(u32, Option<i32>)>) -> FunctionSummary {
        FunctionSummary {
            reads: vec![],
            writes: writes
                .into_iter()
                .map(|(global, value)| ValueWrite { global, value })
                .collect(),
            sysfns: vec![],
            calls: vec![],
            cmp_literals: BTreeMap::new(),
        }
    }

    fn fn_with_calls(calls: Vec<&str>) -> FunctionSummary {
        FunctionSummary {
            reads: vec![],
            writes: vec![],
            sysfns: vec![],
            calls: calls.into_iter().map(|s| s.to_string()).collect(),
            cmp_literals: BTreeMap::new(),
        }
    }

    #[test]
    fn closure_unions_reads_writes_calls_and_transitions() {
        // entry -> child -> leaf; entry sets g[5]=1, leaf sets g[7]=Some(2)
        // and a giArenaLoad-derived transition lives on `leaf`.
        let mut fns: BTreeMap<String, FunctionSummary> = BTreeMap::new();
        let mut entry = fn_with_calls(vec!["child"]);
        entry.writes.push(ValueWrite {
            global: 5,
            value: Some(1),
        });
        entry.reads.push(3);
        fns.insert("entry".into(), entry);
        let mut child = fn_with_calls(vec!["leaf"]);
        child.reads.push(11);
        fns.insert("child".into(), child);
        let leaf = fn_with_writes(vec![(7, Some(2))]);
        fns.insert("leaf".into(), leaf);
        let transitions = vec![Transition {
            to: ["q02".into(), "Q01".into()],
            via_fn: "leaf".into(),
        }];

        let s = build_trigger_summary("entry", &fns, &transitions);

        assert_eq!(s.called_fns, vec!["entry", "child", "leaf"]);
        assert!(s.reads.contains(&3) && s.reads.contains(&11));
        assert!(s.writes.contains(&ValueWrite {
            global: 5,
            value: Some(1)
        }));
        assert!(s.writes.contains(&ValueWrite {
            global: 7,
            value: Some(2)
        }));
        assert_eq!(s.transitions, vec![["q02".to_string(), "Q01".to_string()]]);
    }

    #[test]
    fn closure_empty_for_missing_fn() {
        let fns: BTreeMap<String, FunctionSummary> = BTreeMap::new();
        let s = build_trigger_summary("does_not_exist", &fns, &[]);
        assert!(s.called_fns.is_empty());
        assert!(s.reads.is_empty());
        assert!(s.writes.is_empty());
        assert!(s.transitions.is_empty());
    }

    #[test]
    fn closure_is_dedup_and_terminates_on_cycle() {
        // a -> b -> a (cycle): closure must include {a, b} exactly once and return.
        let mut fns: BTreeMap<String, FunctionSummary> = BTreeMap::new();
        fns.insert("a".into(), fn_with_calls(vec!["b"]));
        fns.insert("b".into(), fn_with_calls(vec!["a"]));
        let s = build_trigger_summary("a", &fns, &[]);
        assert_eq!(s.called_fns, vec!["a", "b"]);
    }

    #[test]
    fn plot_index_groups_by_global_and_dedupes() {
        let mut catalog = Catalog {
            version: CATALOG_VERSION,
            sysfn_count: 0,
            global_count: 0,
            scenes: BTreeMap::new(),
            plot_index: BTreeMap::new(),
        };

        let mut scene = Scene::default();
        scene
            .fns
            .insert("f1".into(), fn_with_writes(vec![(5, Some(1))]));
        let mut block = Block::default();
        let trig_summary = build_trigger_summary("f1", &scene.fns, &[]);
        block.triggers.push(Trigger {
            name: "ev01".into(),
            function: "f1".into(),
            center: [0.0; 3],
            half_size: [0.0; 3],
            shape: "plane",
            kind: "trigger",
            summary: trig_summary,
        });
        scene.blocks.insert("01".into(), block);
        catalog.scenes.insert("q01".into(), scene);

        build_plot_index(&mut catalog);

        let entries = catalog.plot_index.get("5").expect("global 5 not indexed");
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.value, Some(1));
        assert_eq!(e.scene, "q01");
        assert_eq!(e.block, "01");
        assert_eq!(e.trigger.as_deref(), Some("ev01"));
        assert_eq!(e.r#fn, "f1");
    }

    /// `extract_cmp_literals` must recover the RHS literal from both
    /// forms of the AS gate pattern, keyed by the trace-visible PC
    /// (`addr + 8` — opcode is 4 bytes, the offset operand another 4).
    #[test]
    fn extract_cmp_literals_short_and_long_form() {
        // `addr` values chosen so the (addr + 8) keys are easy to spot.
        let insts = vec![
            // Short form: Rdga4 ; Cmpii { rhs: 11400 } ; Jnz off
            // Branch at addr=0x10 → trace pc=0x18.
            AsInstInstance {
                addr: 0x00,
                inst: AsInst::Rdga4 { index: -1 },
            },
            AsInstInstance {
                addr: 0x08,
                inst: AsInst::Cmpii { rhs: 11_400 },
            },
            AsInstInstance {
                addr: 0x10,
                inst: AsInst::Jnz { offset: 100 },
            },
            // Long form: Rdga4 ; Set4 V ; Subi ; Jz off
            AsInstInstance {
                addr: 0x20,
                inst: AsInst::Rdga4 { index: -1 },
            },
            AsInstInstance {
                addr: 0x28,
                inst: AsInst::Set4 { data: 10_600 },
            },
            AsInstInstance {
                addr: 0x30,
                inst: AsInst::Subi,
            },
            AsInstInstance {
                addr: 0x38,
                inst: AsInst::Jz { offset: 50 },
            },
            // Pattern broken by an unrelated CallSys: must NOT
            // attribute a stale literal to this Jcc.
            AsInstInstance {
                addr: 0x50,
                inst: AsInst::Rdga4 { index: -1 },
            },
            AsInstInstance {
                addr: 0x58,
                inst: AsInst::CallSys {
                    function_index: -100,
                },
            },
            AsInstInstance {
                addr: 0x60,
                inst: AsInst::Jnz { offset: 4 },
            },
            // PushZero-as-RHS short cut: Rdga4 ; PushZero ; Cmpi ; Jz
            AsInstInstance {
                addr: 0x70,
                inst: AsInst::Rdga4 { index: -1 },
            },
            AsInstInstance {
                addr: 0x78,
                inst: AsInst::PushZero,
            },
            AsInstInstance {
                addr: 0x80,
                inst: AsInst::Cmpi,
            },
            AsInstInstance {
                addr: 0x88,
                inst: AsInst::Jz { offset: 8 },
            },
        ];

        let lits = extract_cmp_literals(&insts);
        assert_eq!(lits.get(&0x18), Some(&Some(11_400)));
        assert_eq!(lits.get(&0x40), Some(&Some(10_600)));
        assert_eq!(lits.get(&0x68), None);
        assert_eq!(lits.get(&0x90), Some(&Some(0)));
    }

    /// Positive-index `Rdga4` references module-local globals (not
    /// part of the plot vector) and must not trigger gate recognition.
    #[test]
    fn extract_cmp_literals_skips_module_local_rdga4() {
        let insts = vec![
            AsInstInstance {
                addr: 0x00,
                inst: AsInst::Rdga4 { index: 3 },
            },
            AsInstInstance {
                addr: 0x08,
                inst: AsInst::Cmpii { rhs: 1 },
            },
            AsInstInstance {
                addr: 0x10,
                inst: AsInst::Jnz { offset: 8 },
            },
        ];
        assert!(extract_cmp_literals(&insts).is_empty());
    }
}
