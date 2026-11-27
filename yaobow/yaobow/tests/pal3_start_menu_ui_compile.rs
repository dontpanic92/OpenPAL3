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
    fn thumbnail(&self, slot: i32) -> Option<ComRc<radiance_scripting::comdef::services::ISprite>> {
        matches!(slot, 1 | 36).then(|| ComRc::from_object(Thumbnail))
    }
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
        "\u{74a7}\u{5c71}\u{5c71}\u{6d1e}\u{58f9}"
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

mod comdef {
    pub use radiance_scripting::comdef::*;
    pub use yaobow_lib::comdef::yaobow_services;
}

struct TestHost;
yaobow_lib::ComObject_YaobowHostContext!(crate::TestHost);
impl radiance_scripting::comdef::services::IHostContextImpl for TestHost {
    fn scene_manager(&self) -> ComRc<radiance::comdef::ISceneManager> {
        unimplemented!()
    }
    fn audio(&self) -> ComRc<radiance_scripting::comdef::services::IAudioService> {
        unimplemented!()
    }
    fn textures(&self) -> ComRc<radiance_scripting::comdef::services::ITextureService> {
        unimplemented!()
    }
    fn vfs(&self) -> ComRc<radiance_scripting::comdef::services::IVfsService> {
        unimplemented!()
    }
    fn input(&self) -> ComRc<radiance_scripting::comdef::services::IInputService> {
        unimplemented!()
    }
    fn games(&self) -> ComRc<radiance_scripting::comdef::services::IGameRegistry> {
        unimplemented!()
    }
    fn app(&self) -> ComRc<radiance_scripting::comdef::services::IAppService> {
        unimplemented!()
    }
    fn random(&self) -> ComRc<radiance_scripting::comdef::services::IRandomService> {
        unimplemented!()
    }
    fn config(&self) -> ComRc<radiance_scripting::comdef::services::IConfigService> {
        unimplemented!()
    }
}
impl yaobow_lib::comdef::yaobow_services::IYaobowHostContextImpl for TestHost {
    fn pal3(&self) -> ComRc<shared::openpal3::comdef::IPal3Service> {
        unimplemented!()
    }
    fn pal4(&self) -> ComRc<shared::openpal4::comdef::IPal4Service> {
        unimplemented!()
    }
    fn pal5(&self) -> ComRc<shared::openpal5::comdef::IPal5Service> {
        unimplemented!()
    }
    fn swd5(&self) -> ComRc<shared::openswd5::comdef::ISwd5Service> {
        unimplemented!()
    }
}

struct CreditsAtlas {
    native: bool,
    credits: bool,
}
mod credits_atlas {
    shared::ComObject_Pal3UiAtlas!(crate::CreditsAtlas);
}
impl IPal3UiAtlasImpl for CreditsAtlas {
    fn has_sprite(&self, name: &str) -> bool {
        match name {
            "ui/menu/btn_qianchenyimeng0.tga" => self.native,
            "ui/uilib/zzq.tga" => self.credits,
            _ => false,
        }
    }
    fn sprite(&self, name: &str) -> Option<ComRc<radiance_scripting::comdef::services::ISprite>> {
        (name == "ui/menu/log0.tga" || (name == "ui/uilib/zzq.tga" && self.credits))
            .then(|| ComRc::from_object(Thumbnail))
    }
    fn native_width(&self) -> i32 {
        800
    }
    fn native_height(&self) -> i32 {
        600
    }
}
struct Thumbnail;
radiance_scripting::ComObject_Sprite!(crate::Thumbnail);
impl radiance_scripting::comdef::services::ISpriteImpl for Thumbnail {
    fn com_id(&self) -> i32 {
        -12345
    }
    fn width(&self) -> i32 {
        160
    }
    fn height(&self) -> i32 {
        120
    }
    fn u0(&self) -> f32 {
        0.
    }
    fn v0(&self) -> f32 {
        0.
    }
    fn u1(&self) -> f32 {
        1.
    }
    fn v1(&self) -> f32 {
        1.
    }
}

struct CreditsPage;
radiance_scripting::ComObject_AtlasPage!(crate::CreditsPage);
impl radiance_scripting::comdef::services::IAtlasPageImpl for CreditsPage {
    fn width(&self) -> i32 {
        512
    }
    fn height(&self) -> i32 {
        1024
    }
    fn sprite(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> ComRc<radiance_scripting::comdef::services::ISprite> {
        assert_eq!(
            (x, y, w, h),
            (0, 0, 366, 1024),
            "native credits crop, not a squeezed 512px image"
        );
        ComRc::from_object(Thumbnail)
    }
    fn whole(&self) -> ComRc<radiance_scripting::comdef::services::ISprite> {
        panic!("credits use a crop")
    }
}
struct CreditsLoader {
    loads: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    available: bool,
}
radiance_scripting::ComObject_SpriteService!(crate::CreditsLoader);
impl radiance_scripting::comdef::services::ISpriteServiceImpl for CreditsLoader {
    fn load_sprite(&self, _: &str) -> Option<ComRc<radiance_scripting::comdef::services::ISprite>> {
        panic!("load the page to preserve native source cropping")
    }
    fn load_atlas_page(
        &self,
        path: &str,
    ) -> Option<ComRc<radiance_scripting::comdef::services::IAtlasPage>> {
        self.loads.borrow_mut().push(path.into());
        self.available.then(|| ComRc::from_object(CreditsPage))
    }
}

#[test]
fn credits_load_eagerly_outside_manifest_only_for_pal3() {
    use shared::openpal3::ui_atlas::{AtlasManifest, Pal3UiAtlas};
    for (kind, available) in [
        (AtlasManifest::Tli, true),
        (AtlasManifest::Tli, false),
        (AtlasManifest::Plug, true),
    ] {
        let loads = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let atlas = Pal3UiAtlas::create(
            ComRc::from_object(CreditsLoader {
                loads: loads.clone(),
                available,
            }),
            b"",
            kind,
        );
        let expected = kind == AtlasManifest::Tli && available;
        for _ in 0..2 {
            assert_eq!(atlas.has_sprite("UI\\UILIB\\ZZQ.TGA"), expected);
            assert_eq!(atlas.sprite("UI\\UILIB\\ZZQ.TGA").is_some(), expected);
        }
        assert_eq!(
            *loads.borrow(),
            if kind == AtlasManifest::Tli {
                vec!["/basedata/basedata/ui/UILib/zzq.tga"]
            } else {
                vec![]
            },
            "sprite lookups must not upload while a UI frame holds the texture cache"
        );
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
fn native_taiji_accelerates_per_frame_clamps_and_reverses() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(
        r#"
        import yaobow.openpal3.start_menu;
        pub fn make() -> &start_menu.TaijiMotion { &start_menu.TaijiMotion() }
        pub fn step(m: &start_menu.TaijiMotion, closing: bool) -> float {
            m.advance(closing); m.angle
        }
        pub fn reset(m: &start_menu.TaijiMotion) { m.reset(); }
    "#,
    )
    .unwrap();
    let motion = host.call_returning_data("make", vec![]).unwrap();
    let step = |closing| {
        host.call_returning_data("step", vec![motion.clone(), Data::Int(closing)])
            .unwrap()
    };
    for frame in 1..=13 {
        let Data::Float(angle) = step(0) else {
            panic!("expected angle");
        };
        assert!(
            (angle - (2.8 * frame as f64 + 0.25 * frame as f64 * (frame + 1) as f64)).abs()
                < 0.0001
        );
    }
    assert_eq!(step(0), Data::Float(82.));
    assert_eq!(step(0), Data::Float(82.));
    assert_eq!(step(1), Data::Float(78.7));
    for _ in 0..13 {
        step(1);
    }
    assert_eq!(step(1), Data::Float(0.));
    host.call_void("reset", vec![motion]).unwrap();
}

#[test]
fn native_credits_route_scroll_fade_and_dismiss_without_click_through() {
    let host = ScriptHost::new();
    host.set_script_assets(install_script_assets());
    host.load_source(r#"
        import radiance;
        import yaobow.yaobow_services;
        import shared.openpal3;
        import radiance_scripting.ui as gui;
        import yaobow.openpal3.start_menu;
        import yaobow.openpal3.save_load;
        pub fn make(host: &yaobow_services.IYaobowHostContext, atlas: &openpal3.IPal3UiAtlas)
            -> &start_menu.Pal3StartMenuDirector {
            let xs: array<float> = [];
            let names: array<string> = [];
            &start_menu.Pal3StartMenuDirector(host, "", atlas, null,
                start_menu.Intent.None, 0, -1, 0.0, 0.4, false,
                box(xs), box(xs), box(xs), box(names), null, false,
                gui.make_ctx(), save_load.make_browser(), false)
        }
        pub fn click(d: &start_menu.Pal3StartMenuDirector, wid: int) { d.dispatch_click(wid); }
        pub fn active(d: &start_menu.Pal3StartMenuDirector) -> bool { d.credits.active }
        pub fn new_game(d: &start_menu.Pal3StartMenuDirector) -> bool { d.intent == start_menu.Intent.NewGame }
        pub fn y(d: &start_menu.Pal3StartMenuDirector) -> float { d.credits.y }
        pub fn alpha(d: &start_menu.Pal3StartMenuDirector) -> float { d.credits.logo_alpha }
        pub fn step(d: &start_menu.Pal3StartMenuDirector, dt: float) { d.credits.advance(dt, false); }
        pub fn frame(d: &start_menu.Pal3StartMenuDirector, ui: &radiance.IUiHost) {
            let xs: array<float> = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            let names: array<string> = ["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""];
            d.cloud_x = box(xs); d.cloud_y = box(xs); d.cloud_speed = box(xs); d.cloud_names = box(names);
            d.render(ui, 0.0);
        }
    "#).unwrap();
    let host_id = host.intern(ComRc::<
        yaobow_lib::comdef::yaobow_services::IYaobowHostContext,
    >::from_object(TestHost));
    let host_box = host
        .foreign_box("yaobow.comdef.yaobow_services.IYaobowHostContext", host_id)
        .unwrap();
    let (recorder, ui) = RecordingUiHost::create();
    recorder.set_display_size(1600, 600);
    let ui_id = host.intern(ui);
    let ui_box = host.foreign_box("radiance.comdef.IUiHost", ui_id).unwrap();
    let roots =
        std::cell::RefCell::new(vec![host.root(host_box.clone()), host.root(ui_box.clone())]);
    let make = |native, credits| {
        let atlas_id = host.intern(ComRc::<IPal3UiAtlas>::from_object(CreditsAtlas {
            native,
            credits,
        }));
        let atlas = host
            .foreign_box("shared.openpal3.comdef.IPal3UiAtlas", atlas_id)
            .unwrap();
        let data = host
            .call_returning_data("make", vec![host_box.clone(), atlas])
            .unwrap();
        roots.borrow_mut().push(host.root(data.clone()));
        data
    };
    let d = make(true, true);
    let click = |d: &Data, wid| {
        host.call_void("click", vec![d.clone(), Data::Int(wid)])
            .unwrap()
    };
    let value = |name, d: &Data| host.call_returning_data(name, vec![d.clone()]).unwrap();
    let step = |dt| {
        host.call_void("step", vec![d.clone(), Data::Float(dt)])
            .unwrap()
    };
    let frame = || {
        host.call_void("frame", vec![d.clone(), ui_box.clone()])
            .unwrap()
    };
    click(&d, 5);
    assert_eq!(value("active", &d), Data::Int(1));
    frame();
    assert!(
        recorder.calls.borrow().iter().any(|c| matches!(c,
        UiCall::ImageRect { x0, y0, x1, y1, .. }
        if (*x0,*y0,*x1,*y1) == (620.,518.,986.,1542.))),
        "credits retain native width, height, start position and letterboxing"
    );
    click(&d, 3);
    assert_eq!(value("new_game", &d), Data::Int(0));
    // Same sequence executed by the original-binary oracle (0x44c8a0).
    for (dt, expected) in [
        (0.1, 515.7319946),
        (1.0, 493.052002),
        (0.016, 492.6891174),
        (60.0, -868.1109009),
    ] {
        step(dt);
        let Data::Float(y) = value("y", &d) else {
            panic!("expected scroll position")
        };
        assert!((y - expected).abs() < 0.001);
        assert_eq!(value("active", &d), Data::Int(1));
    }
    assert_eq!(value("alpha", &d), Data::Float(0.0));
    step(10.0);
    assert_eq!(value("active", &d), Data::Int(0));
    step(1.0);
    let Data::Float(alpha) = value("alpha", &d) else {
        panic!("expected opacity")
    };
    assert!((alpha - 80.0 / 255.0).abs() < 0.00001);
    for button in [0, 1] {
        click(&d, 5);
        assert_eq!(value("y", &d), Data::Float(518.0));
        recorder.set_mouse_down(true);
        recorder.mouse_clicked_buttons.borrow_mut()[button] = true;
        recorder.press_latch.set(3);
        frame();
        assert_eq!(value("active", &d), Data::Int(0));
        assert_eq!(recorder.press_latch.get(), -1);
        recorder.mouse_clicked_buttons.borrow_mut()[button] = false;
        frame();
        click(&d, 3);
        assert_eq!(
            value("new_game", &d),
            Data::Int(0),
            "held dismissal cannot activate the cover"
        );
        recorder.set_mouse_down(false);
        frame();
    }
    for unavailable in [make(false, true), make(true, false)] {
        click(&unavailable, 5);
        assert_eq!(
            value("active", &unavailable),
            Data::Int(0),
            "PAL3A and missing credits stay inert"
        );
    }
    click(&d, 3);
    assert_eq!(value("new_game", &d), Data::Int(1));
    for root in roots.into_inner() {
        host.unroot(root);
    }
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
        recorder.calls.borrow().iter().any(|call| matches!(call,
            UiCall::TextAt { s, .. } if s == "\u{74a7}\u{5c71}\u{5c71}\u{6d1e}\u{58f9}")),
        "the shared browser must display localized slot metadata"
    );
    assert!(
        recorder
            .calls
            .borrow()
            .iter()
            .any(|c| matches!(c, UiCall::TextAt { s, .. } if s == "01:01:01"))
    );
    assert_eq!(click(true, 200.0, 82.0), Data::Int(1));
    assert!(
        recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id: -12345, x0, y0, x1, y1, .. }
        if (*x0,*y0,*x1,*y1)==(970.,74.,1063.,143.))),
        "standalone previews use the original row's 93x69 rectangle and letterboxing"
    );
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
    assert!(
        recorder.calls.borrow().iter().any(|call| matches!(call,
        UiCall::ImageRect { texture_com_id: -12345, x0, y0, x1, y1, .. }
        if (*x0,*y0,*x1,*y1)==(902.,52.,995.,121.))),
        "in-game save previews must use the same row-relative geometry"
    );
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
