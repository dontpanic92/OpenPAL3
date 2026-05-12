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

    fn read_bytes_internal(&self, _vfs_path: &str) -> Vec<u8> {
        Vec::new()
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
    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(include_str!("../scripts/welcome.p7"))
        .expect("welcome.p7 compiles");

    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(include_str!("../scripts/settings.p7"))
        .expect("settings.p7 compiles");

    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(include_str!("../scripts/main_tree.p7"))
        .expect("main_tree.p7 compiles");
}

#[test]
fn main_tree_script_renders_vfs_entries() {
    use crosscom::ComRc;
    use radiance_scripting::comdef::services::IVfsService;
    use radiance_scripting::ui_walker::{kinds, owned};

    let mut runtime = radiance_scripting::ScriptRuntime::new();
    runtime
        .load_source(include_str!("../scripts/main_tree.p7"))
        .expect("main_tree.p7 compiles");
    let vfs = ComRc::<IVfsService>::from_object(StubVfs {
        last_string: RefCell::new(String::new()),
    });
    let vfs_id = runtime.intern(vfs);
    let vfs = runtime
        .foreign_box("radiance_scripting.comdef.services.IVfsService", vfs_id)
        .expect("vfs foreign box");
    let state = runtime
        .call_returning_data("init", vec![vfs])
        .expect("main_tree init");
    runtime.store_state(state.clone());
    let node = runtime
        .call_returning_data(
            "render",
            vec![state, p7::interpreter::context::Data::Float(0.0)],
        )
        .expect("main_tree render");
    let owned = runtime
        .with_ctx(|ctx| owned::resolve(ctx, &node))
        .expect("main_tree UiNode should resolve");

    assert_eq!(owned.kind, kinds::COLUMN);
    assert_eq!(owned.children.len(), 2);
    assert_eq!(owned.children[0].kind, kinds::TREE_NODE);
    assert_eq!(owned.children[0].label, "dir");
    assert_eq!(owned.children[0].i1, 10);
    assert_eq!(owned.children[0].children[0].kind, kinds::TREE_LEAF);
    assert_eq!(owned.children[0].children[0].label, "child.txt");
    assert_eq!(owned.children[1].kind, kinds::TREE_LEAF);
    assert_eq!(owned.children[1].label, "root.txt");
}
