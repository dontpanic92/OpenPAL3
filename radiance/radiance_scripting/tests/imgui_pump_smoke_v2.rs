//! Smoke tests for the runtime-typed CCW factory's fat-CCW behaviour
//! for IDirector + IImmediateDirector aggregation, and the
//! `ImguiImmediateDirectorPump::dispatch_render_im` helper.
//!
//! Drives a script-side
//! `struct[immediate_director.IImmediateDirector, IDirectorLifecycle] StubIm`
//! through:
//!  - `wrap_director(handle, data) -> ComRc<IDirector>`,
//!  - QI to `ComRc<IImmediateDirector>` (validates the fat-CCW slot
//!    for the derived interface),
//!  - `dispatch_render_im(&director, &recording_ui_host, dt)`
//!    invoking the script's `render_im` which forwards to the
//!    `RecordingUiHost`.

use std::sync::Mutex;

use crosscom::ComRc;
use p7::errors::RuntimeError;
use p7::interpreter::context::{Context, Data};
use radiance::comdef::IDirector;
use radiance_scripting::comdef::immediate_director::IImmediateDirector;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use radiance_scripting::{
    register_immediate_director_proto, wrap_director, ImguiImmediateDirectorPump, ScriptHost,
};

const SCRIPT: &str = r#"
import radiance;
import immediate_director;

@intrinsic(name="imp_test.record_event")
fn record_event(seed: int, event: int);

struct[immediate_director.IImmediateDirector] StubIm(
    seed: int,
) {
    pub fn activate(self: ref<Self>) -> int {
        record_event(self.seed, 1);
        0
    }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> {
        record_event(self.seed, 2);
        return null;
    }
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) -> int {
        // Forward a single recognisable call to the UI host so the
        // RecordingUiHost picks it up. `text` is the simplest leaf
        // method we can verify.
        ui.text("im");
        record_event(self.seed, 4);
        0
    }
    pub fn deactivate(self: ref<Self>) -> int {
        record_event(self.seed, 3);
        0
    }
}

pub fn make_im(seed: int) -> box<immediate_director.IImmediateDirector> {
    let d = box(StubIm(seed));
    d as box<immediate_director.IImmediateDirector>
}
"#;

static EVENTS: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn record_event_host_fn(ctx: &mut Context) -> Result<(), RuntimeError> {
    let frame = ctx.stack_frame_mut()?;
    let event = frame.stack.pop().expect("event arg");
    let seed = frame.stack.pop().expect("seed arg");
    match (seed, event) {
        (Data::Int(s), Data::Int(e)) => {
            EVENTS.lock().unwrap().push((s, e));
            Ok(())
        }
        other => panic!("expected (int, int), got {:?}", other),
    }
}

fn host_runtime_handle(host: &std::rc::Rc<ScriptHost>) -> crosscom_protosept::RuntimeHandle {
    let mut out: Option<crosscom_protosept::RuntimeHandle> = None;
    <ScriptHost as crosscom_protosept::RuntimeAccess>::with_ctx(host, &mut |_ctx| {
        let h = crosscom_protosept::with_services(|s| s.runtime_handle())
            .expect("with_services inside scope");
        out = Some(h);
    });
    out.expect("with_ctx ran body")
}

#[test]
fn wrap_director_round_trips_qi_to_immediate_director() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();
    register_immediate_director_proto();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("imp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let data = host
        .call_returning_data("make_im", vec![Data::Int(11)])
        .expect("make_im");
    let handle = host_runtime_handle(&host);
    let as_director: ComRc<IDirector> = wrap_director(&handle, data).expect("wrap_director");
    let im: ComRc<IImmediateDirector> = as_director
        .query_interface::<IImmediateDirector>()
        .expect("QI IDirector -> IImmediateDirector via fat CCW");

    // Drive activate + update via the QI'd IDirector ComRc.
    as_director.activate(); // (11, 1)
    let next = as_director.update(0.016); // (11, 2)
    assert!(next.is_none());

    assert_eq!(*EVENTS.lock().unwrap(), vec![(11, 1), (11, 2)]);

    // Drop both ComRcs without first calling deactivate; no implicit
    // release hook fires (lifecycle is now driven explicitly by
    // `ISceneManager::deactivate()`).
    drop(as_director);
    drop(im);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        vec![(11, 1), (11, 2)],
        "drop does not invoke deactivate; SceneManager owns that call"
    );
}

#[test]
fn dispatch_render_im_forwards_to_script_and_recording_ui_host() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();
    register_immediate_director_proto();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("imp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(SCRIPT).expect("load_source");

    let data = host
        .call_returning_data("make_im", vec![Data::Int(7)])
        .expect("make_im");
    let handle = host_runtime_handle(&host);
    let as_director: ComRc<IDirector> = wrap_director(&handle, data).expect("wrap_director");
    let im: ComRc<IImmediateDirector> = as_director
        .query_interface::<IImmediateDirector>()
        .expect("QI IDirector -> IImmediateDirector via fat CCW");

    // Build a RecordingUiHost and drive the script's render_im
    // through the pump's headless dispatch helper. The script's
    // render_im body issues `ui.text("im")`, which RecordingUiHost
    // captures, and emits a `(seed, 4)` event so we can confirm
    // dispatch landed.
    let (recorder, ui_host) = RecordingUiHost::create();
    let fired = ImguiImmediateDirectorPump::dispatch_render_im(&as_director, &ui_host, 0.016);
    assert!(fired, "QI to IImmediateDirector should succeed");

    assert_eq!(*EVENTS.lock().unwrap(), vec![(7, 4)]);
    let calls: Vec<UiCall> = recorder.calls.borrow().clone();
    assert!(
        matches!(calls.first(), Some(UiCall::Text(s)) if s == "im"),
        "RecordingUiHost should record the text('im') call from script render_im; got {:?}",
        calls
    );

    drop(im);
    drop(as_director);
}

#[test]
fn dispatch_render_im_returns_false_for_non_immediate_director() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Build a plain `wrap_director` ComRc<IDirector> (NOT an
    // IImmediateDirector). dispatch_render_im should QI-miss and
    // return false without invoking any script method.
    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("imp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(
        r#"
import radiance;

@intrinsic(name="imp_test.record_event")
fn record_event(seed: int, event: int);

struct[radiance.IDirector] PlainDir(seed: int) {
    pub fn activate(self: ref<Self>) -> int { record_event(self.seed, 1); 0 }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> { return null; }
    pub fn deactivate(self: ref<Self>) -> int { record_event(self.seed, 3); 0 }
}

pub fn make_plain(seed: int) -> box<radiance.IDirector> {
    let d = box(PlainDir(seed));
    d as box<radiance.IDirector>
}
"#,
    )
    .expect("load");

    let data = host
        .call_returning_data("make_plain", vec![Data::Int(99)])
        .expect("make_plain");
    let handle = host_runtime_handle(&host);
    let director: ComRc<IDirector> =
        radiance_scripting::wrap_director(&handle, data).expect("wrap_director");

    let (_recorder, ui_host) = RecordingUiHost::create();
    let fired = ImguiImmediateDirectorPump::dispatch_render_im(&director, &ui_host, 0.016);
    assert!(
        !fired,
        "QI to IImmediateDirector must fail for a plain IDirector"
    );
}

/// Regression: when a script-side struct conforms to both
/// `IImmediateDirector` and `IDirector` but the dispatcher marshals
/// it through an `IDirector?`-typed return (the upstream IDL shape of
/// `IDirector::update`), the resulting CCW must still QI to
/// `IImmediateDirector`. Otherwise the engine's immediate-mode pump
/// QI-misses on the transition and the screen goes black.
#[test]
fn idirector_update_returning_im_director_qis_to_immediate() {
    let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EVENTS.lock().unwrap().clear();
    register_immediate_director_proto();

    let host = ScriptHost::new();
    host.with_ctx_mut(|ctx| {
        ctx.register_host_function("imp_test.record_event".to_string(), record_event_host_fn);
    });
    host.load_source(
        r#"
import radiance;
import immediate_director;

@intrinsic(name="imp_test.record_event")
fn record_event(seed: int, event: int);

struct[immediate_director.IImmediateDirector, radiance.IDirector] NextIm(seed: int) {
    pub fn activate(self: ref<Self>) -> int { record_event(self.seed, 1); 0 }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> { return null; }
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) -> int {
        ui.text("next");
        record_event(self.seed, 4);
        0
    }
    pub fn deactivate(self: ref<Self>) -> int { record_event(self.seed, 3); 0 }
}

struct[radiance.IDirector] WelcomeLike(next_seed: int) {
    pub fn activate(self: ref<Self>) -> int { 0 }
    pub fn update(self: ref<Self>, dt: float) -> ?box<radiance.IDirector> {
        let n = box(NextIm(self.next_seed));
        return n as box<radiance.IDirector>;
    }
    pub fn deactivate(self: ref<Self>) -> int { 0 }
}

pub fn make_welcome(seed: int) -> box<radiance.IDirector> {
    let d = box(WelcomeLike(seed));
    d as box<radiance.IDirector>
}
"#,
    )
    .expect("load");

    let data = host
        .call_returning_data("make_welcome", vec![Data::Int(42)])
        .expect("make_welcome");
    let handle = host_runtime_handle(&host);
    let welcome: ComRc<IDirector> =
        radiance_scripting::wrap_director(&handle, data).expect("wrap_director");

    // Drive update; the script returns a NextIm wrapped as
    // box<radiance.IDirector>. Without the most-derived-uuid upgrade,
    // QI to IImmediateDirector here returns None and the pump silently
    // bails (black screen). With the fix, the upgrade picks
    // IImmediateDirector because NextIm conforms to it.
    let next = welcome.update(0.016).expect("transition expected");
    let (_recorder, ui_host) = RecordingUiHost::create();
    let fired = ImguiImmediateDirectorPump::dispatch_render_im(&next, &ui_host, 0.016);
    assert!(
        fired,
        "transitioned director must QI to IImmediateDirector for the pump to render"
    );
    assert_eq!(*EVENTS.lock().unwrap(), vec![(42, 4)]);
}
