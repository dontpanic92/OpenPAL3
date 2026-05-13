//! Verifies the Rust→script director round-trip: a Rust-implemented
//! `IDirector` ComObject can be handed to a script as a `box<radiance.IDirector>`
//! foreign box, wrapped via `director.wrap_host_director`, and then driven by
//! a `ScriptedDirector` proxy exactly as if it were script-implemented.

use std::cell::Cell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance_scripting::{ScriptHost, ScriptedDirector};

struct CountingDirector {
    activated: Rc<Cell<u32>>,
    updated: Rc<Cell<u32>>,
}

radiance_scripting::ComObject_ScriptedDirector!(crate::CountingDirector);

impl IDirectorImpl for CountingDirector {
    fn activate(&self) {
        self.activated.set(self.activated.get() + 1);
    }

    fn update(&self, _delta_sec: f32) -> Option<ComRc<IDirector>> {
        self.updated.set(self.updated.get() + 1);
        None
    }
}

const SCRIPT: &str = r#"
import director;
import radiance;
import ui;

pub struct[director.Director] LocalHostDirector(pub inner: box<radiance.IDirector>) {
    pub fn activate(self: ref<Self>) {
        let _ = self.inner.activate();
    }
    pub fn deactivate(self: ref<Self>) {}
    pub fn render(self: ref<Self>, dt: float) -> ui.UiNode {
        return ui.dummy(0.0, 0.0);
    }
    pub fn dispatch(self: ref<Self>, command_id: int) -> array<box<director.Director>> {
        let result: array<box<director.Director>> = [];
        return result;
    }
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.Director>> {
        let _ = self.inner.update(dt);
        let result: array<box<director.Director>> = [];
        return result;
    }
}

pub fn wrap(host: box<radiance.IDirector>) -> box<director.Director> {
    return box(LocalHostDirector(host)) as box<director.Director>;
}
"#;

#[test]
fn script_can_wrap_rust_director_and_proxy_drives_it() {
    let activated = Rc::new(Cell::new(0u32));
    let updated = Rc::new(Cell::new(0u32));
    let rust_director: ComRc<IDirector> = ComRc::from_object(CountingDirector {
        activated: activated.clone(),
        updated: updated.clone(),
    });

    let host = ScriptHost::new();
    host.load_source(SCRIPT).expect("script should compile");

    let com_id = host.intern(rust_director);
    let foreign = host
        .foreign_box("radiance.comdef.IDirector", com_id)
        .expect("IDirector foreign box should construct");

    let wrapped = host
        .call_returning_data("wrap", vec![foreign])
        .expect("wrap should produce a Director box");

    let handle = host.root(wrapped);
    let proxy = ScriptedDirector::wrap(host.clone(), handle);

    proxy.activate();
    assert_eq!(
        activated.get(),
        1,
        "Rust IDirector::activate must fire through HostDirector adapter when driven by the proxy"
    );

    let _ = proxy.update(0.016);
    assert_eq!(
        updated.get(),
        1,
        "Rust IDirector::update must fire through HostDirector adapter when driven by the proxy"
    );
}
