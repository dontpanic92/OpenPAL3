//! Verifies the Rust→script director round-trip: a Rust-implemented
//! `IDirector` ComObject can be handed to a script as a
//! `box<radiance.IDirector>` foreign box, wrapped in a script-side
//! adapter struct conforming to `director.ImmediateDirector`, and then
//! driven by a `ScriptedImmediateDirector` proxy exactly as if it were
//! script-implemented.

use std::cell::Cell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance_scripting::services::ui_host_recording::RecordingUiHost;
use radiance_scripting::{ScriptHost, ScriptedImmediateDirector};

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
import immediate_director;

pub struct[director.ImmediateDirector] LocalHostDirector(pub inner: box<radiance.IDirector>) {
    pub fn activate(self: ref<Self>) {
        let _ = self.inner.activate();
    }
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) {}
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let _ = self.inner.update(dt);
        let result: array<box<director.ImmediateDirector>> = [];
        return result;
    }
}

pub fn wrap(host: box<radiance.IDirector>) -> box<director.ImmediateDirector> {
    return box(LocalHostDirector(host)) as box<director.ImmediateDirector>;
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
        .expect("wrap should produce an ImmediateDirector box");

    let handle = host.root(wrapped);
    let (_recording, ui_host) = RecordingUiHost::create();
    let proxy = ScriptedImmediateDirector::wrap(host.clone(), handle, ui_host);

    proxy.activate();
    assert_eq!(
        activated.get(),
        1,
        "Rust IDirector::activate must fire through the script-side adapter when driven by the proxy"
    );

    let _ = proxy.update(0.016);
    assert_eq!(
        updated.get(),
        1,
        "Rust IDirector::update must fire through the script-side adapter when driven by the proxy"
    );
}
