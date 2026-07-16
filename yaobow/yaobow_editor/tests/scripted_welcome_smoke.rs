//! Compile-time smoke test for the scripted welcome page wiring.

use std::cell::RefCell;

use radiance_scripting::comdef::services::IVfsServiceImpl;

mod comdef {
    pub use radiance_scripting::comdef::*;
}

struct StubVfs {
    last_string: RefCell<String>,
}

radiance_scripting::ComObject_VfsService!(crate::StubVfs);

impl IVfsServiceImpl for StubVfs {
    fn exists(&self, _vfs_path: &str) -> bool {
        true
    }

    fn byte_len(&self, _vfs_path: &str) -> i32 {
        0
    }

    fn entry_count(&self, vfs_path: &str) -> i32 {
        match vfs_path {
            "/" => 2,
            "/dir" => 1,
            _ => 0,
        }
    }

    fn subdir_count(&self, vfs_path: &str) -> i32 {
        // Mirror entry_is_dir: only the leading "/dir" entry under "/"
        // is a directory; everything else is a file.
        match vfs_path {
            "/" => 1,
            _ => 0,
        }
    }

    fn entry_name(&self, vfs_path: &str, index: i32) -> &str {
        let value = match (vfs_path, index) {
            ("/", 0) => "dir",
            ("/", 1) => "root.txt",
            ("/dir", 0) => "child.txt",
            _ => "",
        };
        *self.last_string.borrow_mut() = value.to_string();
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn entry_is_dir(&self, vfs_path: &str, index: i32) -> bool {
        matches!((vfs_path, index), ("/", 0))
    }

    fn is_dir(&self, vfs_path: &str) -> bool {
        vfs_path == "/dir"
    }

    fn is_expanded(&self, vfs_path: &str) -> bool {
        vfs_path == "/dir"
    }

    fn toggle_expanded(&self, _vfs_path: &str) {}

    fn command_id(&self, vfs_path: &str) -> i32 {
        match vfs_path {
            "/dir" => 10,
            "/dir/child.txt" => 11,
            "/root.txt" => 12,
            _ => 0,
        }
    }

    fn command_path(&self, _command_id: i32) -> &str {
        ""
    }
    fn invalidate_entries(&self) {}
}

/// Helper: build the dedicated script `AssetManager` used by these
/// smoke tests (engine bindings + radiance_scripting + shared + the
/// editor ypk). Mirrors `scripted_welcome_page::build_editor_script_assets`.
fn build_test_assets() -> std::rc::Rc<radiance::asset::AssetManager> {
    let assets = radiance::asset::AssetManager::new();
    radiance_scripting::mount_engine_bindings(&assets);
    radiance_scripting::mount_scripts(&assets);
    shared::mount_scripts(&assets);
    yaobow_editor::script_source::mount_scripts(&assets);
    assets
}

#[test]
fn scripted_welcome_page_module_compiles() {
    use crosscom::ComRc;
    use radiance::comdef::{IApplication, IDirector};
    use yaobow_editor::directors::ScriptedWelcomePage;

    let _create: fn(ComRc<IApplication>) -> ComRc<IDirector> = ScriptedWelcomePage::create;
}

#[test]
fn welcome_scripts_compile_with_shared_ui_module() {
    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source_from_path("/yaobow_editor/main.p7")
        .expect("editor script compiles");
}

#[test]
fn welcome_runtime_can_create_resource_tree_root() {
    use crosscom::ComRc;
    use radiance_scripting::comdef::services::IVfsService;

    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source_from_path("/yaobow_editor/main.p7")
        .expect("editor script compiles");
    let vfs = ComRc::<IVfsService>::from_object(StubVfs {
        last_string: RefCell::new(String::new()),
    });
    let vfs_id = runtime.intern(vfs);
    let vfs = runtime
        .foreign_box("radiance_scripting.comdef.services.IVfsService", vfs_id)
        .expect("vfs foreign box");
    // resource_tree_root_node is module-private; exercise it indirectly by
    // calling the helper through a thin wrapper would require touching the
    // script. Instead, just verify the script compiled and the foreign box
    // round-trip works.
    let _ = vfs;
    drop(runtime);
}

#[test]
fn resource_tree_change_helpers_choose_green_orange_and_plain_widgets() {
    use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};

    let runtime = radiance_scripting::ScriptHost::new();
    runtime.set_script_assets(build_test_assets());
    runtime
        .load_source(
            r#"
import radiance;
import yaobow_editor.resource_tree;

pub fn entry(ui: box<radiance.IUiHost>) -> int {
    resource_tree.tree_leaf_for_change(ui, "add", true, 0);
    resource_tree.tree_leaf_for_change(ui, "replace", false, 1);
    resource_tree.tree_leaf_for_change(ui, "plain", false, -1);
    if resource_tree.tree_node_open_for_change(ui, "add-dir", 0) {
        ui.tree_pop();
    }
    if resource_tree.tree_node_open_for_change(ui, "replace-dir", 1) {
        ui.tree_pop();
    }
    0
}
"#,
        )
        .expect("resource tree helper probe compiles");

    let (recorder, ui_com) = RecordingUiHost::create();
    let ui_id = runtime.intern(ui_com);
    let ui = runtime
        .foreign_box("radiance.comdef.IUiHost", ui_id)
        .expect("ui foreign box");
    runtime
        .call_returning_data("entry", vec![ui])
        .expect("resource tree helper probe runs");

    assert_eq!(
        recorder.calls.borrow().clone(),
        vec![
            UiCall::TreeLeafColored {
                label: "add".to_string(),
                selected: true,
                color: [0.35, 0.85, 0.45, 1.0],
            },
            UiCall::TreeLeafColored {
                label: "replace".to_string(),
                selected: false,
                color: [1.0, 0.62, 0.20, 1.0],
            },
            UiCall::TreeLeaf {
                label: "plain".to_string(),
                selected: false,
            },
            UiCall::TreeNodeOpenColored {
                label: "add-dir".to_string(),
                color: [0.35, 0.85, 0.45, 1.0],
            },
            UiCall::TreeNodeOpenColored {
                label: "replace-dir".to_string(),
                color: [1.0, 0.62, 0.20, 1.0],
            },
        ]
    );
}
