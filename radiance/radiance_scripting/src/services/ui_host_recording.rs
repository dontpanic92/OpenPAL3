//! `RecordingUiHost` — test-only `IUiHost` impl that records every
//! method call and synchronously invokes pairing-widget bodies via
//! `IAction.invoke()`. Used by `ui_host_smoke.rs` to verify the
//! end-to-end SAM + crosscom + p7 plumbing without dragging imgui-rs
//! into the test surface. A real imgui-backed implementation will
//! replace `RecordingUiHost` in radiance_scripting once the migration
//! progresses past H2.
//!
//! ## Why two structs (state + facade)?
//!
//! `ComObject_UiHost!` generates a CCW that *owns* its inner impl
//! value, which means the test would lose access to the recorded
//! calls the moment the `ComRc<IUiHost>` is built. We therefore split
//! responsibilities:
//!
//! - [`RecordingUiHost`] holds the call log and the programmable
//!   return tables; the test keeps an `Rc<RecordingUiHost>` to read
//!   from and to seed.
//! - [`HostFacade`] is the ComObject the script sees; it holds a
//!   clone of the `Rc<RecordingUiHost>` and forwards every method.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::{ComRc, IAction};

use crate::comdef::immediate_director::{IUiHost, IUiHostImpl};

#[derive(Debug, Clone, PartialEq)]
pub enum UiCall {
    Window { title: String, w: f32, h: f32, flags: i32 },
    WindowCentered { title: String, w: f32, h: f32 },
    WindowFullscreen { title: String, flags: i32 },
    ChildWindow { id: String, w: f32, h: f32 },
    Table { id: String, cols: i32 },
    TabBar { id: String },
    TreeNode { label: String },
    Group,
    StyleAlpha { alpha: f32 },
    TabItem { label: String, closable: bool },
    SameLine,
    Dummy { w: f32, h: f32 },
    Spacer { w: f32, h: f32 },
    TableNextColumn,
    Text(String),
    TextWithFont { font_idx: i32, s: String },
    Button { label: String, w: f32, h: f32 },
    Image { texture_com_id: i32, w: f32, h: f32 },
    ImageFit { texture_com_id: i32, src_w: f32, src_h: f32 },
    MultilineText { content: String, w: f32, h: f32 },
    TreeLeaf { label: String },
    BodyEnter(&'static str),
    BodyExit(&'static str),
}

#[derive(Default)]
pub struct RecordingUiHost {
    pub calls: RefCell<Vec<UiCall>>,
    pub button_results: RefCell<std::collections::HashMap<String, bool>>,
    pub tab_item_results: RefCell<std::collections::HashMap<String, bool>>,
    pub tree_leaf_results: RefCell<std::collections::HashMap<String, bool>>,
}

impl RecordingUiHost {
    /// Build a fresh recorder and its paired `ComRc<IUiHost>`. The test
    /// keeps the `Rc<RecordingUiHost>` for assertions and seeds; the
    /// `ComRc<IUiHost>` is what gets interned into the script.
    pub fn create() -> (Rc<RecordingUiHost>, ComRc<IUiHost>) {
        let host = Rc::new(RecordingUiHost::default());
        let facade = HostFacade {
            inner: host.clone(),
        };
        let com = ComRc::<IUiHost>::from_object(facade);
        (host, com)
    }

    fn record(&self, call: UiCall) {
        self.calls.borrow_mut().push(call);
    }
}

pub struct HostFacade {
    inner: Rc<RecordingUiHost>,
}

ComObject_UiHost!(super::HostFacade);

fn invoke_body(label: &'static str, host: &RecordingUiHost, body: ComRc<IAction>) {
    host.record(UiCall::BodyEnter(label));
    body.invoke();
    host.record(UiCall::BodyExit(label));
}

impl IUiHostImpl for HostFacade {
    fn window(&self, title: &str, w: f32, h: f32, flags: i32, body: ComRc<IAction>) {
        self.inner
            .record(UiCall::Window { title: title.into(), w, h, flags });
        invoke_body("window", &self.inner, body);
    }

    fn window_centered(&self, title: &str, w: f32, h: f32, body: ComRc<IAction>) {
        self.inner
            .record(UiCall::WindowCentered { title: title.into(), w, h });
        invoke_body("window_centered", &self.inner, body);
    }

    fn window_fullscreen(&self, title: &str, flags: i32, body: ComRc<IAction>) {
        self.inner
            .record(UiCall::WindowFullscreen { title: title.into(), flags });
        invoke_body("window_fullscreen", &self.inner, body);
    }

    fn child_window(&self, id: &str, w: f32, h: f32, body: ComRc<IAction>) {
        self.inner
            .record(UiCall::ChildWindow { id: id.into(), w, h });
        invoke_body("child_window", &self.inner, body);
    }

    fn table(&self, id: &str, cols: i32, body: ComRc<IAction>) {
        self.inner.record(UiCall::Table { id: id.into(), cols });
        invoke_body("table", &self.inner, body);
    }

    fn tab_bar(&self, id: &str, body: ComRc<IAction>) {
        self.inner.record(UiCall::TabBar { id: id.into() });
        invoke_body("tab_bar", &self.inner, body);
    }

    fn tree_node(&self, label: &str, body: ComRc<IAction>) {
        self.inner.record(UiCall::TreeNode { label: label.into() });
        invoke_body("tree_node", &self.inner, body);
    }

    fn group(&self, body: ComRc<IAction>) {
        self.inner.record(UiCall::Group);
        invoke_body("group", &self.inner, body);
    }

    fn style_alpha(&self, alpha: f32, body: ComRc<IAction>) {
        self.inner.record(UiCall::StyleAlpha { alpha });
        invoke_body("style_alpha", &self.inner, body);
    }

    fn tab_item(&self, label: &str, closable: bool, body: ComRc<IAction>) -> bool {
        self.inner
            .record(UiCall::TabItem { label: label.into(), closable });
        invoke_body("tab_item", &self.inner, body);
        self.inner
            .tab_item_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }

    fn same_line(&self) {
        self.inner.record(UiCall::SameLine);
    }

    fn dummy(&self, w: f32, h: f32) {
        self.inner.record(UiCall::Dummy { w, h });
    }

    fn spacer(&self, w: f32, h: f32) {
        self.inner.record(UiCall::Spacer { w, h });
    }

    fn table_next_column(&self) {
        self.inner.record(UiCall::TableNextColumn);
    }

    fn text(&self, s: &str) {
        self.inner.record(UiCall::Text(s.into()));
    }

    fn text_with_font(&self, font_idx: i32, s: &str) {
        self.inner
            .record(UiCall::TextWithFont { font_idx, s: s.into() });
    }

    fn button(&self, label: &str, w: f32, h: f32) -> bool {
        self.inner
            .record(UiCall::Button { label: label.into(), w, h });
        self.inner
            .button_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }

    fn image(&self, texture_com_id: i32, w: f32, h: f32) {
        self.inner
            .record(UiCall::Image { texture_com_id, w, h });
    }

    fn image_fit(&self, texture_com_id: i32, src_w: f32, src_h: f32) {
        self.inner
            .record(UiCall::ImageFit { texture_com_id, src_w, src_h });
    }

    fn multiline_text(&self, content: &str, w: f32, h: f32) {
        self.inner
            .record(UiCall::MultilineText { content: content.into(), w, h });
    }

    fn tree_leaf(&self, label: &str) -> bool {
        self.inner.record(UiCall::TreeLeaf { label: label.into() });
        self.inner
            .tree_leaf_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }

    // Mouse-query methods are no-ops in the recording host: the
    // recording UI is only used by tests that don't drive real mouse
    // input, and returning fixed defaults keeps script-side branches
    // predictable.
    fn is_item_hovered(&self) -> bool {
        false
    }
    fn mouse_down(&self, _button: i32) -> bool {
        false
    }
    fn mouse_drag_delta_x(&self, _button: i32) -> f32 {
        0.0
    }
    fn mouse_drag_delta_y(&self, _button: i32) -> f32 {
        0.0
    }
    fn reset_mouse_drag_delta(&self, _button: i32) {}
    fn mouse_wheel(&self) -> f32 {
        0.0
    }
    fn content_region_avail_x(&self) -> i32 {
        0
    }
    fn content_region_avail_y(&self) -> i32 {
        0
    }
}
