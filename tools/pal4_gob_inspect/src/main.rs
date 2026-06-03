//! Walks every `scenedata/<scene>/<block>/GameObjs.gob` in a PAL4 install
//! and prints diagnostics aimed at uncovering which fields control
//! load-time visibility. Pure inspection — does not touch the engine.
//!
//! Output sections:
//!   1. Per-tag mesh-name distribution        — confirms whether
//!      SOUND / MARKER tags really do ship placeholder meshes
//!      across the entire corpus.
//!   2. Distribution of every fixed property  — HIDE / VALIDFOR /
//!      AUTO_DISAPPEAR / `active` / `flags[]` counts and unique
//!      `VALIDFOR` string values (the one with unknown semantics).
//!   3. Spot-print of all entries with VALIDFOR set, grouped by
//!      `(scene, block)`, so we can cross-reference against
//!      `pal4_plot.json` and the original game.
//!   4. NpcInfo `default_visible` distribution and dump of NPCs
//!      with `default_visible == 0`.

use std::{
    collections::BTreeMap,
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::Parser;
use common::store_ext::StoreExt2;
use fileformats::{
    binrw::BinRead,
    npc::NpcInfoFile,
    pal4::gob::{
        GobFile, GobFixedProperty, GobObjectType, GobProperty,
    },
};
use packfs::init_virtual_fs;

#[derive(Parser)]
#[command(about = "Inspect PAL4 GOB / NPC visibility-related fields")]
struct Cli {
    /// PAL4 install root (the dir containing `gamedata/`).
    #[arg(long)]
    root: PathBuf,

    /// When set, prints every (scene, block, object_name) triple. Off
    /// by default to keep stdout small.
    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("resolve root: {}", cli.root.display()))?;
    eprintln!("Mounting PAL4 vfs from {}", root.display());
    let vfs = init_virtual_fs(&root, None);

    let mut blocks: Vec<(String, String)> = Vec::new();
    let scenedata = std::path::Path::new("/gamedata/scenedata");
    let scene_entries =
        <mini_fs::MiniFs as mini_fs::Store>::entries_path(&vfs, scenedata)
            .with_context(|| format!("list {}", scenedata.display()))?;
    for entry in scene_entries {
        let entry = entry?;
        if !matches!(entry.kind, mini_fs::EntryKind::Dir) {
            continue;
        }
        let scene_name = std::path::Path::new(&entry.name)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if scene_name.is_empty() {
            continue;
        }
        let scene_dir = scenedata.join(&scene_name);
        let block_entries =
            match <mini_fs::MiniFs as mini_fs::Store>::entries_path(&vfs, &scene_dir) {
                Ok(it) => it,
                Err(e) => {
                    eprintln!("WARN: list {}: {:#}", scene_dir.display(), e);
                    continue;
                }
            };
        for be in block_entries {
            let be = be?;
            if !matches!(be.kind, mini_fs::EntryKind::Dir) {
                continue;
            }
            let block_name = std::path::Path::new(&be.name)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !block_name.is_empty() {
                blocks.push((scene_name.clone(), block_name));
            }
        }
    }
    blocks.sort();
    eprintln!("Discovered {} (scene, block) pairs", blocks.len());

    // -- Aggregates --------------------------------------------------------
    let mut tag_mesh_counts: BTreeMap<u32, BTreeMap<String, usize>> = BTreeMap::new();
    let mut hide_counts: BTreeMap<i32, usize> = BTreeMap::new();
    let mut auto_disappear_counts: BTreeMap<i32, usize> = BTreeMap::new();
    let mut active_counts: BTreeMap<u32, usize> = BTreeMap::new();
    let mut flag_counts: [BTreeMap<u32, usize>; 3] =
        [BTreeMap::new(), BTreeMap::new(), BTreeMap::new()];
    let mut validfor_value_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut validfor_entries: Vec<(String, String, String, u32, ValidforRepr)> = Vec::new();
    let mut hide_entries: Vec<(String, String, String, u32, i32)> = Vec::new();
    let mut auto_dis_entries: Vec<(String, String, String, u32, i32)> = Vec::new();

    let mut npc_default_visible_counts: BTreeMap<i32, usize> = BTreeMap::new();
    let mut npc_invisible_entries: Vec<(String, String, String)> = Vec::new();

    let mut gob_missing = 0usize;
    let mut npc_missing = 0usize;

    for (scene, block) in &blocks {
        let gob_path = format!("/gamedata/scenedata/{}/{}/GameObjs.gob", scene, block);
        match vfs.read_to_end(&gob_path) {
            Ok(bytes) => {
                match GobFile::read(&mut std::io::Cursor::new(bytes)) {
                    Ok(gob) => {
                        for (i, entry) in gob.entries.iter().enumerate() {
                            let tag = gob
                                .header
                                .object_types
                                .get(i)
                                .copied()
                                .unwrap_or(u32::MAX);
                            let mesh = entry
                                .file_name
                                .to_string()
                                .unwrap_or_else(|_| "<bad-utf8>".to_string());
                            *tag_mesh_counts
                                .entry(tag)
                                .or_default()
                                .entry(mesh.clone())
                                .or_default() += 1;
                            *active_counts.entry(entry.active).or_default() += 1;
                            for (i, slot) in flag_counts.iter_mut().enumerate() {
                                *slot.entry(entry.flags[i]).or_default() += 1;
                            }

                            let object_name = entry
                                .name
                                .to_string()
                                .unwrap_or_else(|_| "<bad-utf8>".to_string());

                            if let Some(hide) = entry
                                .get_property(GobFixedProperty::HIDE)
                                .and_then(|p| p.value_i32())
                            {
                                *hide_counts.entry(hide).or_default() += 1;
                                if hide != 0 {
                                    hide_entries.push((
                                        scene.clone(),
                                        block.clone(),
                                        object_name.clone(),
                                        tag,
                                        hide,
                                    ));
                                }
                            }
                            if let Some(ad) = entry
                                .get_property(GobFixedProperty::AUTO_DISAPPEAR)
                                .and_then(|p| p.value_i32())
                            {
                                *auto_disappear_counts.entry(ad).or_default() += 1;
                                if ad != 0 {
                                    auto_dis_entries.push((
                                        scene.clone(),
                                        block.clone(),
                                        object_name.clone(),
                                        tag,
                                        ad,
                                    ));
                                }
                            }

                            if let Some(vf) = entry.get_property(GobFixedProperty::VALIDFOR) {
                                let repr = ValidforRepr::from_property(vf);
                                let key = repr.to_canonical_string();
                                *validfor_value_counts
                                    .entry(key.clone())
                                    .or_default() += 1;
                                validfor_entries.push((
                                    scene.clone(),
                                    block.clone(),
                                    object_name.clone(),
                                    tag,
                                    repr,
                                ));
                            }
                        }
                    }
                    Err(e) => eprintln!("WARN: parse {} failed: {:#}", gob_path, e),
                }
            }
            Err(_) => gob_missing += 1,
        }

        let npc_path = format!("/gamedata/scenedata/{}/{}/npcInfo.npc", scene, block);
        match vfs.read_to_end(&npc_path) {
            Ok(bytes) => match NpcInfoFile::read(&mut std::io::Cursor::new(bytes)) {
                Ok(npc) => {
                    for n in &npc.data {
                        *npc_default_visible_counts
                            .entry(n.default_visible)
                            .or_default() += 1;
                        if n.default_visible != 1 {
                            npc_invisible_entries.push((
                                scene.clone(),
                                block.clone(),
                                n.name.to_string().unwrap_or_else(|_| "<bad-utf8>".into()),
                            ));
                        }
                    }
                }
                Err(e) => eprintln!("WARN: parse {} failed: {:#}", npc_path, e),
            },
            Err(_) => npc_missing += 1,
        }
    }

    println!("== summary ==");
    println!(
        "  blocks scanned : {}",
        blocks.len()
    );
    println!(
        "  gob missing    : {}    npc missing    : {}",
        gob_missing, npc_missing
    );

    println!("\n== object-type / mesh distribution ==");
    for (tag, meshes) in &tag_mesh_counts {
        let name = GobObjectType::name(*tag).unwrap_or("?");
        let total: usize = meshes.values().sum();
        println!("  tag {:>3} ({:<8}) entries={:<5}", tag, name, total);
        let mut sorted: Vec<(&String, &usize)> = meshes.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (mesh, count) in sorted.iter().take(10) {
            println!("      {:<40} x{}", mesh, count);
        }
        if sorted.len() > 10 {
            println!("      … {} more meshes", sorted.len() - 10);
        }
    }

    println!("\n== flag distributions ==");
    println!("  HIDE              : {:?}", hide_counts);
    println!("  AUTO_DISAPPEAR    : {:?}", auto_disappear_counts);
    println!("  active            : {:?}", active_counts);
    for (i, slot) in flag_counts.iter().enumerate() {
        println!("  flags[{}]          : {:?}", i, slot);
    }
    println!("  NPC default_visible : {:?}", npc_default_visible_counts);

    println!("\n== unique VALIDFOR values ({} distinct) ==", validfor_value_counts.len());
    let mut sorted_vf: Vec<(&String, &usize)> = validfor_value_counts.iter().collect();
    sorted_vf.sort_by(|a, b| b.1.cmp(a.1));
    for (val, count) in &sorted_vf {
        println!("  {:<6} x{}", val, count);
    }

    println!(
        "\n== VALIDFOR-flagged entries ({} total) ==",
        validfor_entries.len()
    );
    if cli.verbose {
        for (s, b, n, t, v) in &validfor_entries {
            println!(
                "  {}/{:<6} {:>16}  tag={} ({:<8}) validfor={}",
                s,
                b,
                n,
                t,
                GobObjectType::name(*t).unwrap_or("?"),
                v.to_canonical_string()
            );
        }
    }

    println!(
        "\n== HIDE-flagged entries ({} total) ==",
        hide_entries.len()
    );
    if cli.verbose {
        for (s, b, n, t, h) in &hide_entries {
            println!(
                "  {}/{:<6} {:>16}  tag={} ({:<8}) hide={}",
                s,
                b,
                n,
                t,
                GobObjectType::name(*t).unwrap_or("?"),
                h
            );
        }
    }

    println!(
        "\n== AUTO_DISAPPEAR-flagged entries ({} total) ==",
        auto_dis_entries.len()
    );
    if cli.verbose {
        for (s, b, n, t, a) in &auto_dis_entries {
            println!(
                "  {}/{:<6} {:>16}  tag={} ({:<8}) auto_disappear={}",
                s,
                b,
                n,
                t,
                GobObjectType::name(*t).unwrap_or("?"),
                a
            );
        }
    }

    println!(
        "\n== NPCs with default_visible != 1 ({} total) ==",
        npc_invisible_entries.len()
    );
    if cli.verbose {
        for (s, b, n) in &npc_invisible_entries {
            println!("  {}/{:<6} {}", s, b, n);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum ValidforRepr {
    Int(i32),
    Float(f32),
    Str(String),
    Other,
}

impl ValidforRepr {
    fn from_property(p: &GobProperty) -> Self {
        match p {
            GobProperty::GobPropertyI32(v) => Self::Int(v.value),
            GobProperty::GobPropertyF32(v) => Self::Float(v.value),
            GobProperty::GobPropertyString(v) => Self::Str(v.value.clone()),
            _ => Self::Other,
        }
    }

    fn to_canonical_string(&self) -> String {
        match self {
            Self::Int(v) => format!("i32:{}", v),
            Self::Float(v) => format!("f32:{}", v),
            Self::Str(s) => format!("str:{:?}", s),
            Self::Other => "other".to_string(),
        }
    }
}
