//! Compile-only smoke for the declarative `openpal3/start_menu.p7`, which
//! is migrated onto the `radiance_scripting.ui` widget framework. Mounts
//! the real script asset bundle (engine bindings + radiance_scripting +
//! shared + yaobow, so `import radiance_scripting.ui;` resolves) and
//! loads the script through the production VFS module provider.

use crosscom::ComRc;
use p7::interpreter::context::Data;
use radiance_scripting::ScriptHost;
use radiance_scripting::services::ui_host_recording::{RecordingUiHost, UiCall};
use shared::openpal3::comdef::{
    IPal3SaveSlots, IPal3SaveSlotsImpl, IPal3UiAtlas, IPal3UiAtlasImpl,
};
use yaobow_lib::script_source::install_script_assets;

mod openpal3 {
    pub use shared::openpal3::comdef;
}

struct TestSlots;
shared::ComObject_Pal3SaveSlots!(crate::TestSlots);
impl IPal3SaveSlotsImpl for TestSlots {
    fn slot_count(&self) -> i32 {
        36
    }
    fn status(&self, slot: i32) -> i32 {
        match slot {
            1 | 36 => 1,
            2 => 2,
            _ => 0,
        }
    }
    fn scene(&self, _: i32) -> &str {
        "YZZ / AN"
    }
    fn play_seconds(&self, _: i32) -> i32 {
        3661
    }
    fn money(&self, _: i32) -> i32 {
        321
    }
    fn member_role(&self, _: i32, index: i32) -> i32 {
        if index == 0 { 0 } else { -1 }
    }
    fn role_level(&self, _: i32, _: i32) -> i32 {
        7
    }
}

struct EmptyAtlas;
shared::ComObject_Pal3UiAtlas!(crate::EmptyAtlas);
impl IPal3UiAtlasImpl for EmptyAtlas {
    fn has_sprite(&self, _: &str) -> bool {
        false
    }
    fn sprite(&self, _: &str) -> Option<ComRc<radiance_scripting::comdef::services::ISprite>> {
        None
    }
    fn native_width(&self) -> i32 {
        800
    }
    fn native_height(&self) -> i32 {
        600
    }
}

#[test]
fn pal3_start_menu_compiles_on_ui_framework() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source_from_path("/yaobow/openpal3/start_menu.p7")
        .expect("openpal3/start_menu.p7 must compile against radiance_scripting.ui");
}

#[test]
fn save_browser_pages_to_slot_36_rejects_bad_loads_and_confirms_overwrites() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import radiance;
        import shared.openpal3;
        import yaobow.openpal3.save_load;
        pub fn make(slots: &openpal3.IPal3SaveSlots) -> &save_load.Browser {
            let browser = save_load.make_browser();
            browser.begin(slots, false);
            browser
        }
        pub fn saving(browser: &save_load.Browser, slots: &openpal3.IPal3SaveSlots) {
            browser.begin(slots, true);
        }
        pub fn confirmation(browser: &save_load.Browser) -> int { browser.confirmation }
        pub fn first(browser: &save_load.Browser) -> int { browser.first }
        pub fn frame(browser: &save_load.Browser, ui: &radiance.IUiHost,
            atlas: &openpal3.IPal3UiAtlas, standalone: bool,
            x: float, y: float, press: bool, release: bool, down: bool) -> int {
            browser.paint(ui, atlas, standalone, x, y, press, release, down, false)
        }
    "#,
    )
    .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(1600, 600);
    let ui_id = host.intern(ui);
    let slots_id = host.intern(ComRc::<IPal3SaveSlots>::from_object(TestSlots));
    let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(EmptyAtlas));
    let ui = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let slots = host
        .foreign_box("shared.openpal3.comdef.IPal3SaveSlots", slots_id)
        .unwrap();
    let atlas = host
        .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
        .unwrap();
    let browser = host
        .call_returning_data("make", vec![slots.clone()])
        .unwrap();
    let frame = |standalone: bool, x: f64, y: f64, press: bool, release: bool, down: bool| {
        recorder.calls.borrow_mut().clear();
        host.call_returning_data(
            "frame",
            vec![
                browser.clone(),
                ui.clone(),
                atlas.clone(),
                Data::Int(standalone as i64),
                Data::Float(x + 400.0),
                Data::Float(y),
                Data::Int(press as i64),
                Data::Int(release as i64),
                Data::Int(down as i64),
            ],
        )
        .unwrap()
    };
    let click = |standalone, x, y| {
        assert_eq!(frame(standalone, x, y, true, false, true), Data::Int(0));
        frame(standalone, x, y, false, true, false)
    };
    frame(true, 0.0, 0.0, false, false, false);
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c, UiCall::TextAt { s, .. } if s == "01:01:01"))
    );
    assert_eq!(click(true, 200.0, 82.0), Data::Int(1));
    assert_eq!(click(true, 200.0, 174.0), Data::Int(0));
    assert_eq!(click(true, 200.0, 266.0), Data::Int(0));
    frame(true, 200.0, 82.0, true, false, true);
    assert_eq!(frame(true, 40.0, 82.0, false, true, false), Data::Int(0));
    for _ in 0..8 {
        assert_eq!(click(true, 710.0, 540.0), Data::Int(0));
    }
    assert_eq!(click(true, 200.0, 450.0), Data::Int(36));
    assert_eq!(click(true, 760.0, 560.0), Data::Int(-1));

    host.call_void("saving", vec![browser.clone(), slots])
        .unwrap();
    assert_eq!(click(false, 200.0, 60.0), Data::Int(0));
    assert_eq!(
        host.call_returning_data("confirmation", vec![browser.clone()])
            .unwrap(),
        Data::Int(1)
    );
    recorder.set_mouse_wheel(-1.0);
    assert_eq!(click(false, 487.0, 290.0), Data::Int(0));
    recorder.set_mouse_wheel(0.0);
    assert_eq!(
        host.call_returning_data("first", vec![browser.clone()])
            .unwrap(),
        Data::Int(0),
        "the modal consumes wheel input, including its closing frame"
    );
    assert_eq!(
        host.call_returning_data("confirmation", vec![browser.clone()])
            .unwrap(),
        Data::Int(0)
    );
    assert_eq!(
        frame(false, 487.0, 290.0, false, true, false),
        Data::Int(0),
        "the confirmation release must not activate the save row underneath"
    );
    recorder.set_mouse_wheel(-1.0);
    frame(false, 200.0, 244.0, false, false, false);
    assert_eq!(
        host.call_returning_data("first", vec![browser.clone()])
            .unwrap(),
        Data::Int(1),
        "normal scrolling resumes after the modal closes"
    );
    recorder.set_mouse_wheel(1.0);
    frame(false, 200.0, 244.0, false, false, false);
    recorder.set_mouse_wheel(0.0);
    click(false, 200.0, 60.0);
    assert_eq!(click(false, 321.0, 290.0), Data::Int(1));
    assert_eq!(click(false, 200.0, 244.0), Data::Int(3));
    click(false, 200.0, 152.0);
    assert_eq!(
        host.call_returning_data("confirmation", vec![browser.clone()])
            .unwrap(),
        Data::Int(2)
    );
    click(false, 487.0, 290.0);
    click(false, 450.0, 20.0);
    assert_eq!(click(false, 200.0, 152.0), Data::Int(0));
    assert_eq!(click(false, 200.0, 60.0), Data::Int(1));
}
