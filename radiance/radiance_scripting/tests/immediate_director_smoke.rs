//! H3 smoke test: drive a script-side `ImmediateDirector` through the
//! `ScriptedImmediateDirector` proxy, verifying the full
//! Rust→p7→IUiHost→IAction.invoke→p7 round-trip per render frame.
//!
//! Uses `RecordingUiHost` (no imgui dependency) so the test exercises
//! every piece of plumbing — SAM closure synthesis, foreign-box arg
//! marshalling, `wrap_action`-driven script-impl-of-IAction CCWs,
//! and the IDirector proxy lifecycle — without needing a real imgui
//! frame.

use radiance::comdef::IDirector;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::{ScriptHost, ScriptedImmediateDirector};

const SCRIPT_BASIC: &str = r#"
import director;
import ui_host;

pub struct[director.ImmediateDirector] HelloPage(pub greet: string) {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}

    pub fn render_im(self: ref<Self>, ui: box<ui_host.IUiHost>, dt: float) {
        let _ = ui.window_centered(self.greet, 200.0, 100.0, () => {
            let _ = ui.text(self.greet);
            let _ = ui.button("ok", 80.0, 24.0);
        });
    }

    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let r: array<box<director.ImmediateDirector>> = [];
        return r;
    }
}

pub fn root() -> box<director.ImmediateDirector> {
    box(HelloPage("hello")) as box<director.ImmediateDirector>
}
"#;

#[test]
fn immediate_director_proxy_drives_render_each_frame() {
    let host = ScriptHost::new();
    host.load_source(SCRIPT_BASIC).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();

    let root_value = host
        .call_returning_data("root", Vec::new())
        .expect("root() returns an ImmediateDirector box");
    let handle = host.root(root_value);

    let proxy: crosscom::ComRc<IDirector> =
        ScriptedImmediateDirector::wrap(host.clone(), handle, ui_com);

    // Two consecutive frames must each invoke render_im end-to-end.
    let _ = proxy.update(0.016);
    let _ = proxy.update(0.016);

    let calls = recorder.calls.borrow().clone();
    let per_frame = vec![
        UiCall::WindowCentered {
            title: "hello".to_string(),
            w: 200.0,
            h: 100.0,
        },
        UiCall::BodyEnter("window_centered"),
        UiCall::Text("hello".to_string()),
        UiCall::Button {
            label: "ok".to_string(),
            w: 80.0,
            h: 24.0,
        },
        UiCall::BodyExit("window_centered"),
    ];
    let mut expected = per_frame.clone();
    expected.extend(per_frame);
    assert_eq!(calls, expected);
}

const SCRIPT_WITH_TRANSITION: &str = r#"
import director;
import ui_host;

pub struct[director.ImmediateDirector] SecondPage() {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<ui_host.IUiHost>, dt: float) {
        let _ = ui.text("second");
    }
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let r: array<box<director.ImmediateDirector>> = [];
        return r;
    }
}

pub struct[director.ImmediateDirector] FirstPage() {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<ui_host.IUiHost>, dt: float) {
        let _ = ui.text("first");
    }
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        return [box(SecondPage()) as box<director.ImmediateDirector>];
    }
}

pub fn root() -> box<director.ImmediateDirector> {
    box(FirstPage()) as box<director.ImmediateDirector>
}
"#;

#[test]
fn immediate_director_proxy_chains_transition() {
    let host = ScriptHost::new();
    host.load_source(SCRIPT_WITH_TRANSITION).expect("compile");

    let (recorder, ui_com) = RecordingUiHost::create();

    let root_value = host
        .call_returning_data("root", Vec::new())
        .expect("root() returns an ImmediateDirector box");
    let handle = host.root(root_value);

    let first: crosscom::ComRc<IDirector> =
        ScriptedImmediateDirector::wrap(host.clone(), handle, ui_com);

    let next = first
        .update(0.0)
        .expect("first frame's update should hand off a new director");

    // Drive the wrapped second director one frame and assert it
    // renders its own page through the same `IUiHost` ComObject (the
    // intern id is propagated by `wrap_next`).
    let _ = next.update(0.0);

    let calls = recorder.calls.borrow().clone();
    assert_eq!(
        calls,
        vec![
            UiCall::Text("first".to_string()),
            UiCall::Text("second".to_string()),
        ]
    );
}
