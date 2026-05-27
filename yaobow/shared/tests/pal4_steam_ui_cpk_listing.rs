//! Lists every file inside the Steam-PAL4 `ui.cpk` so we can see what
//! folder layout the previewer needs to support. Skips when the file
//! is not present.

use std::path::PathBuf;

use mini_fs::{EntryKind, MiniFs, StoreExt};
use packfs::cpk::CpkFs;

#[test]
fn list_steam_pal4_ui_cpk() {
    let p = PathBuf::from(r"F:\SteamLibrary\steamapps\common\Chinese Paladin 4\gamedata\ui.cpk");
    if !p.is_file() {
        eprintln!("Steam ui.cpk not present at {} — skipping", p.display());
        return;
    }
    let cpk = CpkFs::new(p).expect("open cpk");
    let vfs = MiniFs::new(false).mount("/", cpk);

    let mut stack: Vec<PathBuf> = vec![PathBuf::from("/")];
    let mut all: Vec<String> = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = vfs.entries(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = std::path::Path::new(&entry.name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let child = dir.join(name);
            match entry.kind {
                EntryKind::Dir => stack.push(child),
                EntryKind::File => {
                    if let Some(s) = child.to_str() {
                        all.push(s.replace('\\', "/"));
                    }
                }
            }
        }
    }
    all.sort();

    // Surface the layouts + imagesets to the test log so we can see
    // the actual paths.
    let layouts: Vec<&String> = all
        .iter()
        .filter(|p| p.to_lowercase().ends_with(".xml"))
        .collect();
    let imagesets: Vec<&String> = all
        .iter()
        .filter(|p| p.to_lowercase().ends_with(".imageset"))
        .collect();
    let pngs: Vec<&String> = all
        .iter()
        .filter(|p| p.to_lowercase().ends_with(".png"))
        .take(20)
        .collect();

    eprintln!("total files in ui.cpk: {}", all.len());
    eprintln!("--- .xml ({}): ---", layouts.len());
    for p in &layouts {
        eprintln!("  {}", p);
    }
    eprintln!("--- .imageset ({}): ---", imagesets.len());
    for p in imagesets.iter().take(40) {
        eprintln!("  {}", p);
    }
    eprintln!("--- .png (sample 20): ---");
    for p in &pngs {
        eprintln!("  {}", p);
    }
}
