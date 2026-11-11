use std::rc::Rc;

use mini_fs::{LocalFs, MiniFs};
use radiance_scripting::services::{GameRegistry, VfsService};

#[test]
fn game_registry_exposes_static_game_data() {
    let games = GameRegistry::create();
    assert!(games.count() >= 10);
    assert_eq!(games.game_at(0), 0);
    assert_eq!(games.config_key(0), "pal3");
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

#[test]
fn native_scene_transfer_keeps_original_carrier_and_rejects_managed_and_script_proxies() {
    use crosscom::ComRc;
    use crosscom_protosept::wrap_proto;
    use radiance::comdef::{IScene, ISceneManager};
    use radiance::scene::{CoreScene, DefaultSceneManager, ISceneExt};
    use radiance_scripting::{RuntimeAccess, ScriptHost, with_services};

    let runtime = ScriptHost::new();
    runtime.load_source(r#"
import radiance;
pub fn transfer(manager: &radiance.ISceneManager, scene: box<radiance.IScene>) -> box<radiance.IScene> {
    manager.push_scene(scene);
    manager.camera()!.set_position(7.0, 8.0, 9.0);
    manager.pop_scene()!
}

@foreign(dispatcher="com.invoke", type_tag="test.NativeScene",
         uuid="27e705f1-d035-4e91-8735-3a006fab870d")
proto NativeScene {}
struct[NativeScene] ScriptScene() {}
pub fn fake_scene() -> box<NativeScene> { box(ScriptScene()) }
"#).unwrap();
    let scene = CoreScene::create();
    let manager = ComRc::<ISceneManager>::from_object(DefaultSceneManager::new());
    let scene_id = runtime.intern(scene.clone());
    let manager_id = runtime.intern(manager.clone());
    let scene_data = runtime
        .foreign_box("radiance.comdef.IScene", scene_id)
        .unwrap();
    let manager_data = runtime
        .foreign_box("radiance.comdef.ISceneManager", manager_id)
        .unwrap();
    let retained = runtime.with_ctx_mut(|ctx| ctx.retain_reference(&scene_data).unwrap());
    let result = runtime
        .call_returning_data("transfer", vec![manager_data.clone(), scene_data])
        .unwrap();
    let original = wrap_proto::<IScene>(&runtime.runtime_handle(), result).unwrap();
    assert_eq!(
        original.ptr_value(),
        scene.ptr_value(),
        "native extensions must see the original CoreScene layout"
    );
    let position = scene.camera().transform().position();
    assert_eq!((position.x, position.y, position.z), (7.0, 8.0, 9.0));
    assert!(manager.scene().is_none());
    let error = runtime
        .call_method_void(manager_data.clone(), "push_scene", vec![retained.clone()])
        .unwrap_err();
    assert!(
        error.to_string().contains("original owned foreign carrier"),
        "{error}"
    );
    let error = wrap_proto::<IScene>(&runtime.runtime_handle(), retained)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("original owned foreign carrier"),
        "{error}"
    );
    let fake = runtime.call_returning_data("fake_scene", vec![]).unwrap();
    let error = runtime
        .call_method_void(manager_data, "push_scene", vec![fake.clone()])
        .unwrap_err();
    assert!(
        error.to_string().contains("original owned foreign carrier"),
        "{error}"
    );
    let error = wrap_proto::<IScene>(&runtime.runtime_handle(), fake)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("original owned foreign carrier"),
        "{error}"
    );
    assert!(
        manager.scene().is_none(),
        "rejected exports never reach native scene storage"
    );
    drop(original);
    <ScriptHost as RuntimeAccess>::with_ctx(&runtime, &mut |ctx| {
        ctx.collect_garbage().unwrap();
        with_services(|services| {
            assert!(services.com_table_mut().release(scene_id));
            assert!(services.com_table_mut().release(manager_id));
            assert_eq!(services.com_table_mut().live(), 0);
        })
        .unwrap();
    });
}
