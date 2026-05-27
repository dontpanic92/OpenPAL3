//! Live Steam PAL4 smoke for the UI layout previewer pipeline.
//!
//! Mounts the production CPK-backed vfs and verifies that a known
//! layout XML inside `gamedata/ui.cpk` classifies as `<GUILayout>`,
//! parses, resolves every referenced imageset, and can read every
//! atlas PNG. This catches the path-translation issues that pure
//! LocalFs smokes miss (e.g. CPK mount points, forward-slash
//! consistency on Windows).

use std::path::PathBuf;

use packfs::init_virtual_fs;
use shared::loaders::cegui::{
    imageset as cegui_imageset,
    layout::{self as cegui_layout, ImageRef},
};

fn steam_pal4_root() -> Option<PathBuf> {
    let p = PathBuf::from(r"F:\SteamLibrary\steamapps\common\Chinese Paladin 4");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

fn classify_xml(vfs: &mini_fs::MiniFs, path: &str) -> bool {
    use mini_fs::StoreExt;
    use std::io::Read;
    let Ok(mut file) = vfs.open(path) else {
        return false;
    };
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf).unwrap_or(0);
    cegui_layout::looks_like_gui_layout(&buf[..n])
}

#[test]
fn steam_pal4_combat_main_window_layout_resolves_end_to_end() {
    let Some(root) = steam_pal4_root() else {
        eprintln!("Steam PAL4 install not present — skipping");
        return;
    };
    let vfs = init_virtual_fs(&root, None);

    // The Steam version's ui.cpk mounts at /gamedata/ui/, so
    // layouts live at /gamedata/ui/layouts/X.xml (no `ui2/` segment).
    let layout_path = "/gamedata/ui/layouts/CombatMainWindow.xml";
    run_smoke(&vfs, layout_path, "/gamedata/ui/imagesets");
}

#[test]
fn steam_pal4_equipment_window_resolves_end_to_end() {
    let Some(root) = steam_pal4_root() else {
        eprintln!("Steam PAL4 install not present — skipping");
        return;
    };
    let vfs = init_virtual_fs(&root, None);
    run_smoke(
        &vfs,
        "/gamedata/ui/layouts/EquipmentWindow.xml",
        "/gamedata/ui/imagesets",
    );
}

fn run_smoke(vfs: &mini_fs::MiniFs, layout_path: &str, imageset_dir: &str) {
    assert!(
        classify_xml(vfs, layout_path),
        "classify_xml should detect <GUILayout> in {layout_path}"
    );

    let layout = cegui_layout::load_layout(vfs, layout_path).expect("parse layout");
    eprintln!("{}: {} windows", layout_path, layout.windows.len());
    assert!(layout.windows.len() > 0);

    // Collect referenced sets.
    let mut sets: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    for w in &layout.windows {
        for slot in [
            &w.image,
            &w.overlay_normal_image,
            &w.overlay_highlight_image,
            &w.overlay_pushed_image,
            &w.overlay_disabled_image,
        ] {
            if let Some(ImageRef { set, .. }) = slot {
                if seen.insert(set.to_lowercase()) {
                    sets.push(set.clone());
                }
            }
        }
    }
    eprintln!("references {} imageset(s): {:?}", sets.len(), sets);
    assert!(!sets.is_empty());

    let mut unresolved: Vec<String> = Vec::new();
    let mut missing_png: Vec<String> = Vec::new();

    // Pre-scan the dir to mirror `ImagesetIndex::scan`.
    use mini_fs::{EntryKind, StoreExt};
    let mut name_to_path: std::collections::HashMap<String, String> = Default::default();
    if let Ok(entries) = vfs.entries(imageset_dir) {
        for entry in entries.flatten() {
            if !matches!(entry.kind, EntryKind::File) {
                continue;
            }
            let entry_name = std::path::Path::new(&entry.name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !entry_name.to_lowercase().ends_with(".imageset") {
                continue;
            }
            let p = format!("{}/{}", imageset_dir, entry_name);
            if let Ok((imageset, _)) = cegui_imageset::load_imageset_with_atlas(vfs, &p) {
                name_to_path.insert(imageset.name.to_lowercase(), p);
            }
        }
    }
    eprintln!("scanned {} imagesets via dir listing", name_to_path.len());

    for set_name in &sets {
        let cands = [
            format!("{}/{}.imageset", imageset_dir, set_name),
            format!("{}/{}_0.imageset", imageset_dir, set_name),
        ];
        let mut matched: Option<(cegui_imageset::Imageset, String)> = None;
        for c in &cands {
            if let Ok((imageset, _png)) = cegui_imageset::load_imageset_with_atlas(vfs, c) {
                if imageset.name.eq_ignore_ascii_case(set_name) {
                    matched = Some((imageset, c.clone()));
                    break;
                }
            }
        }
        if matched.is_none() {
            if let Some(p) = name_to_path.get(&set_name.to_lowercase()) {
                if let Ok((imageset, _)) = cegui_imageset::load_imageset_with_atlas(vfs, p) {
                    matched = Some((imageset, p.clone()));
                }
            }
        }
        let Some((imageset, p)) = matched else {
            unresolved.push(set_name.clone());
            continue;
        };
        eprintln!("set:{} -> {} (atlas={})", set_name, p, imageset.image_path);
        if cegui_imageset::read_atlas_png(vfs, &p, &imageset).is_err() {
            missing_png.push(format!("{} -> {}", set_name, imageset.image_path));
        }
    }
    assert!(
        unresolved.is_empty(),
        "{layout_path}: unresolved set references: {:?}",
        unresolved
    );
    assert!(
        missing_png.is_empty(),
        "{layout_path}: missing atlas PNGs: {:?}",
        missing_png
    );
}
