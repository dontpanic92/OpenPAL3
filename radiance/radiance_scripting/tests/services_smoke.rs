use std::rc::Rc;

use mini_fs::{LocalFs, MiniFs};
use radiance_scripting::services::{GameRegistry, VfsService};

#[test]
fn game_registry_exposes_static_game_data() {
    let games = GameRegistry::create();
    assert!(games.count() >= 10);
    assert_eq!(games.game_at(0), 0);
    assert_eq!(games.config_key(0), "OpenPAL3");
    assert!(!games.full_name(0).is_empty());
}

#[test]
fn vfs_service_exposes_stable_command_paths() {
    let vfs = VfsService::create(Rc::new(MiniFs::new(false)));

    let first = vfs.command_id("/foo.txt");
    let second = vfs.command_id("/foo.txt");
    let other = vfs.command_id("/bar.txt");

    assert_eq!(first, second);
    assert_ne!(first, other);
    assert_eq!(vfs.command_path(first), "/foo.txt");
    assert_eq!(vfs.command_path(other), "/bar.txt");
    assert_eq!(vfs.command_path(-1), "");
}

#[test]
fn vfs_service_tracks_expanded_paths() {
    let vfs = VfsService::create(Rc::new(MiniFs::new(false)));

    assert!(!vfs.is_expanded("/dir"));
    vfs.toggle_expanded("/dir");
    assert!(vfs.is_expanded("/dir"));
    vfs.toggle_expanded("/dir");
    assert!(!vfs.is_expanded("/dir"));
}

#[test]
fn vfs_service_classifies_cached_entry_paths() {
    let root =
        std::env::temp_dir().join(format!("openpal3-vfs-service-test-{}", std::process::id()));
    let dir = root.join("dir");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("file.txt"), b"hello").expect("write temp file");
    let vfs = VfsService::create(Rc::new(MiniFs::new(false).mount("/", LocalFs::new(&root))));

    assert_eq!(vfs.entry_count("/"), 1);
    assert_eq!(vfs.entry_name("/", 0), "dir");
    assert!(vfs.entry_is_dir("/", 0));
    assert!(vfs.is_dir("/dir"));

    assert_eq!(vfs.entry_count("/dir"), 1);
    assert_eq!(vfs.entry_name("/dir", 0), "file.txt");
    assert!(!vfs.entry_is_dir("/dir", 0));
    assert!(!vfs.is_dir("/dir/file.txt"));

    std::fs::remove_dir_all(root).ok();
}
