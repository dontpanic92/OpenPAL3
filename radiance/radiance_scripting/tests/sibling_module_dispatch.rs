//! Reproduces the editor's failing flow more accurately:
//! - Module A (`adapter_mod`) defines HostDirector(1 field) + wrap_host
//! - Module B (`game_mod`) defines a multi-field MainEditorDirector and
//!   returns it through a host service. Its `update` body accesses
//!   field 1 (a box<array<int>>). The host service is exposed via a
//!   foreign proto method that calls back into the script and returns
//!   the multi-field director as a `box<director.ImmediateDirector>` —
//!   exactly the editor's open_game path.
//! - User-main entry just calls `adapter_mod.wrap_host(host_director)`,
//!   where `host_director` is a Rust-side ScriptedImmediateDirector
//!   wrapping the MainEditorDirector.
//! When the proxy drives update, the dispatch chain is:
//!   outer ScriptedImmediateDirector(HostDirector).update
//!     → HostDirector.update body: self.inner.update(dt)  (self is 1 field)
//!       → inner ScriptedImmediateDirector(GameDirector).update
//!         → GameDirector.update body: self.b read    (self is 3 fields)
//! A bug in cross-module dispatch can route either method body to the
//! wrong receiver and panic on field index.

use std::cell::Cell;
use std::rc::Rc;

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance_scripting::services::ui_host_recording::RecordingUiHost;
use radiance_scripting::{ScriptHost, ScriptedImmediateDirector};

struct CountingDirector {
    updated: Rc<Cell<u32>>,
}

radiance_scripting::ComObject_ScriptedDirector!(crate::CountingDirector);

impl IDirectorImpl for CountingDirector {
    fn activate(&self) {}
    fn update(&self, _delta_sec: f32) -> Option<ComRc<IDirector>> {
        self.updated.set(self.updated.get() + 1);
        None
    }
}

const ADAPTER_MOD: &str = r#"
import director;
import radiance;
import immediate_director;

pub struct[director.ImmediateDirector] HostDirector(pub inner: box<radiance.IDirector>) {
    pub fn activate(self: ref<Self>) { let _ = self.inner.activate(); }
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) {}
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let _ = self.inner.update(dt);
        let r: array<box<director.ImmediateDirector>> = []; return r;
    }
}

pub fn wrap_host(inner: box<radiance.IDirector>) -> box<director.ImmediateDirector> {
    return box(HostDirector(inner)) as box<director.ImmediateDirector>;
}
"#;

const GAME_MOD: &str = r#"
import director;
import immediate_director;

pub struct[director.ImmediateDirector] GameDirector(
    pub a: box<array<int>>,
    pub b: box<array<int>>,
    pub c: box<array<int>>,
) {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) {
        // Touch field 1 the same way the real MainEditorDirector does
        // (`self.tabs.len()`-style).
        let n = self.b.len();
    }
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let n = self.b.len();
        let r: array<box<director.ImmediateDirector>> = []; return r;
    }
}

pub fn make_game_director() -> box<director.ImmediateDirector> {
    let arr1: array<int> = [];
    let arr2: array<int> = [];
    let arr3: array<int> = [];
    return box(GameDirector(box(arr1), box(arr2), box(arr3))) as box<director.ImmediateDirector>;
}
"#;

const MAIN_MOD: &str = r#"
import director;
import radiance;
import adapter_mod;
import game_mod;

pub fn make_game() -> box<director.ImmediateDirector> {
    return game_mod.make_game_director();
}

pub fn wrap_host_director(inner: box<radiance.IDirector>) -> box<director.ImmediateDirector> {
    return adapter_mod.wrap_host(inner);
}
"#;

#[test]
fn host_director_wrapping_a_scripted_multi_field_director_dispatches_correctly() {
    let host = ScriptHost::new();
    host.add_binding("adapter_mod", ADAPTER_MOD);
    host.add_binding("game_mod", GAME_MOD);
    host.load_source(MAIN_MOD).expect("main script should compile");

    // Step 1: build the inner game director (multi-field) and wrap it in
    // a Rust-side ScriptedImmediateDirector — this is what
    // app_service::open_game returns in the editor.
    let inner_data = host
        .call_returning_data("make_game", vec![])
        .expect("make_game should succeed");
    let inner_handle = host.root(inner_data);
    let (_inner_recording, inner_ui) = RecordingUiHost::create();
    let inner_director: ComRc<IDirector> =
        ScriptedImmediateDirector::wrap(host.clone(), inner_handle, inner_ui);

    // Step 2: hand it back into the script as a foreign IDirector and
    // wrap it in the HostDirector adapter — this is welcome.update
    // returning [editor_consts.wrap_host_im(next!)] in the editor.
    let inner_com_id = host.intern(inner_director);
    let inner_foreign = host
        .foreign_box("radiance.comdef.IDirector", inner_com_id)
        .expect("inner foreign box");
    let outer_data = host
        .call_returning_data("wrap_host_director", vec![inner_foreign])
        .expect("wrap_host_director should produce an ImmediateDirector box");

    let outer_handle = host.root(outer_data);
    let (_outer_recording, outer_ui) = RecordingUiHost::create();
    let proxy = ScriptedImmediateDirector::wrap(host.clone(), outer_handle, outer_ui);

    proxy.activate();
    let _ = proxy.update(0.016);
    // No assertion needed — the test passes as long as neither layer's
    // update bytecode crashes with a field-index error from cross-module
    // dispatch confusion.
}

const GC_STRESS_MOD: &str = r#"
import director;
import immediate_director;

pub struct[director.ImmediateDirector] CountingDir(pub tick: box<array<int>>) {
    pub fn activate(self: ref<Self>) {}
    pub fn deactivate(self: ref<Self>) {}
    pub fn render_im(self: ref<Self>, ui: box<immediate_director.IUiHost>, dt: float) {}
    pub fn update(self: ref<Self>, dt: float) -> array<box<director.ImmediateDirector>> {
        let n = self.tick[0];
        self.tick[0] = n + 1;
        let r: array<box<director.ImmediateDirector>> = []; return r;
    }
}

pub fn make() -> box<director.ImmediateDirector> {
    let arr: array<int> = [0];
    return box(CountingDir(box(arr))) as box<director.ImmediateDirector>;
}

// Forces ~150 box allocations per call, well past the default
// `gc_threshold = 100`, so calling this between two method dispatches
// is enough to trigger a GC sweep mid-sequence.
pub fn churn() -> int {
    let mut i = 0;
    let raw: array<box<array<int>>> = [];
    let sink = box(raw);
    while i < 150 {
        let arr: array<int> = [i];
        sink.push(box(arr));
        i = i + 1;
    }
    return i;
}
"#;

#[test]
fn host_cached_data_survives_mid_call_gc() {
    // The host-facing contract: a Rust caller may hold a `Data` snapshot
    // across multiple script calls — even ones that trigger GC — without
    // it being silently invalidated. Stable-handle box slots + per-slot
    // generation make this safe; the test asserts dispatch still
    // resolves to the right method body after a forced collection.
    let host = ScriptHost::new();
    host.add_binding("gc_stress", GC_STRESS_MOD);
    host.load_source(
        r#"
import director;
import gc_stress;

pub fn make_dir() -> box<director.ImmediateDirector> {
    return gc_stress.make();
}

pub fn churn() -> int {
    return gc_stress.churn();
}
"#,
    )
    .expect("entry script should compile");

    let director = host
        .call_returning_data("make_dir", vec![])
        .expect("make should produce a director");

    // Root the director so it survives GC. Without this its slot would
    // be freed and any cached `Data` snapshot would correctly fail-fast
    // with `StaleBoxHandle` on next use.
    let root = host.root(director.clone());

    // First call uses `director` directly.
    let _ = host
        .call_method_returning_data(director.clone(), "update", vec![Data::Float(0.016)])
        .expect("first update should succeed");

    // Force a GC sweep between calls. `churn` allocates ~150 boxes,
    // which is enough to trip the default `gc_threshold` of 100. After
    // this, the previous box heap layout is gone — but `director`'s
    // stable slot handle should still resolve to the same script
    // object, because GC frees in place rather than compacting.
    let _ = host
        .call_returning_data("churn", vec![])
        .expect("churn should succeed");

    // Reuse the *same* cached Data snapshot. Before the slab + gen
    // change this would silently dispatch into whichever box now
    // occupied the old compacted slot; with stable handles it must
    // still hit `CountingDir.update`.
    let _ = host
        .call_method_returning_data(director, "update", vec![Data::Float(0.016)])
        .expect("second update should succeed even after a mid-sequence GC");

    host.unroot(root);
}
