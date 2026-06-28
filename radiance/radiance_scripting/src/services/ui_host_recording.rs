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

use radiance::comdef::{IUiHost, IUiHostImpl};

#[derive(Debug, Clone, PartialEq)]
pub enum UiCall {
    Window {
        title: String,
        w: f32,
        h: f32,
        flags: i32,
    },
    WindowCentered {
        title: String,
        w: f32,
        h: f32,
    },
    WindowFullscreen {
        title: String,
        flags: i32,
    },
    ChildWindow {
        id: String,
        w: f32,
        h: f32,
    },
    Table {
        id: String,
        cols: i32,
    },
    TabBar {
        id: String,
    },
    TreeNode {
        label: String,
    },
    Group,
    StyleAlpha {
        alpha: f32,
    },
    WithFont {
        font_idx: i32,
    },
    TabItem {
        label: String,
        closable: bool,
    },
    SameLine,
    Dummy {
        w: f32,
        h: f32,
    },
    Spacer {
        w: f32,
        h: f32,
    },
    TableNextColumn,
    Text(String),
    TextWithFont {
        font_idx: i32,
        s: String,
    },
    Button {
        label: String,
        w: f32,
        h: f32,
    },
    Checkbox {
        label: String,
        value: bool,
    },
    SliderInt {
        label: String,
        value: i32,
        min: i32,
        max: i32,
    },
    SetNextItemWidth(f32),
    Image {
        texture_com_id: i32,
        w: f32,
        h: f32,
    },
    ImageFit {
        texture_com_id: i32,
        src_w: f32,
        src_h: f32,
    },
    ImageUv {
        texture_com_id: i32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    },
    MultilineText {
        content: String,
        w: f32,
        h: f32,
    },
    SetTextBuffer {
        key: i32,
        content: String,
    },
    ShowTextBuffer {
        key: i32,
        w: f32,
        h: f32,
    },
    CopyTextBuffer {
        key: i32,
    },
    TreeLeaf {
        label: String,
        selected: bool,
    },
    ListClipped {
        count: i32,
    },
    TreeNodeOpen {
        label: String,
    },
    TreePop,
    MainMenuBar,
    Menu {
        label: String,
    },
    MenuItem {
        label: String,
        selected: bool,
    },
    FillRect {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },
    ImageRect {
        texture_com_id: i32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    },
    TextAt {
        x: f32,
        y: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        s: String,
    },
    BodyEnter(&'static str),
    BodyExit(&'static str),
}

#[derive(Default)]
pub struct RecordingUiHost {
    pub calls: RefCell<Vec<UiCall>>,
    pub button_results: RefCell<std::collections::HashMap<String, bool>>,
    pub tab_item_results: RefCell<std::collections::HashMap<String, bool>>,
    pub tree_leaf_results: RefCell<std::collections::HashMap<String, bool>>,
    pub tree_node_open_results: RefCell<std::collections::HashMap<String, bool>>,
    pub menu_item_results: RefCell<std::collections::HashMap<String, bool>>,
    /// Logical viewport size reported by `display_size_x/y`. Defaults to
    /// `(0, 0)`; set via [`RecordingUiHost::set_display_size`] to exercise
    /// layout code that fits content into the viewport.
    pub display_size: RefCell<(i32, i32)>,
    /// Per-character advance (w) and line height (h) returned by
    /// `calc_text_size_x/y` as `chars * w` / `h`. Defaults to `(0.0, 0.0)`
    /// so existing tests see the historic zero measurement.
    pub char_size: RefCell<(f32, f32)>,
    /// Seeded result of `is_item_hovered()` (defaults to `false`). Lets
    /// tests drive the press-latch path of `ImageButton` / `LayoutImage`
    /// / `TextButton`, which gate hover/click on this flag.
    pub item_hovered: RefCell<bool>,
    /// Seeded result of `mouse_down(_)` (defaults to `false`). Toggle
    /// across paint passes to simulate a press (down) then release (up).
    pub mouse_is_down: RefCell<bool>,
    /// Optional scripted `mouse_down(_)` results, consumed front-to-back
    /// (one per call). When non-empty it takes precedence over
    /// `mouse_is_down`; when exhausted it falls back to `mouse_is_down`.
    /// Lets a single test drive a multi-frame press→release latch.
    pub mouse_down_script: RefCell<std::collections::VecDeque<bool>>,
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

    /// Seed the logical viewport size reported to scripts.
    pub fn set_display_size(&self, w: i32, h: i32) {
        *self.display_size.borrow_mut() = (w, h);
    }

    /// Seed the per-character advance / line height used to synthesize
    /// `calc_text_size_x/y` results (`chars * w`, `h`).
    pub fn set_char_size(&self, w: f32, h: f32) {
        *self.char_size.borrow_mut() = (w, h);
    }

    /// Seed the `is_item_hovered()` result reported to scripts.
    pub fn set_item_hovered(&self, hovered: bool) {
        *self.item_hovered.borrow_mut() = hovered;
    }

    /// Seed the `mouse_down(_)` result reported to scripts.
    pub fn set_mouse_down(&self, down: bool) {
        *self.mouse_is_down.borrow_mut() = down;
    }

    /// Seed a scripted sequence of `mouse_down(_)` results, consumed one
    /// per call (front-to-back), then falling back to `mouse_is_down`.
    pub fn set_mouse_down_script(&self, seq: impl IntoIterator<Item = bool>) {
        *self.mouse_down_script.borrow_mut() = seq.into_iter().collect();
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
        self.inner.record(UiCall::Window {
            title: title.into(),
            w,
            h,
            flags,
        });
        invoke_body("window", &self.inner, body);
    }

    fn window_centered(&self, title: &str, w: f32, h: f32, body: ComRc<IAction>) {
        self.inner.record(UiCall::WindowCentered {
            title: title.into(),
            w,
            h,
        });
        invoke_body("window_centered", &self.inner, body);
    }

    fn window_fullscreen(&self, title: &str, flags: i32, body: ComRc<IAction>) {
        self.inner.record(UiCall::WindowFullscreen {
            title: title.into(),
            flags,
        });
        invoke_body("window_fullscreen", &self.inner, body);
    }

    fn child_window(&self, id: &str, w: f32, h: f32, body: ComRc<IAction>) {
        self.inner.record(UiCall::ChildWindow {
            id: id.into(),
            w,
            h,
        });
        invoke_body("child_window", &self.inner, body);
    }

    fn table(&self, id: &str, cols: i32, body: ComRc<IAction>) {
        self.inner.record(UiCall::Table {
            id: id.into(),
            cols,
        });
        invoke_body("table", &self.inner, body);
    }

    fn tab_bar(&self, id: &str, body: ComRc<IAction>) {
        self.inner.record(UiCall::TabBar { id: id.into() });
        invoke_body("tab_bar", &self.inner, body);
    }

    fn tree_node(&self, label: &str, body: ComRc<IAction>) {
        self.inner.record(UiCall::TreeNode {
            label: label.into(),
        });
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

    fn with_font(&self, font_idx: i32, body: ComRc<IAction>) {
        self.inner.record(UiCall::WithFont { font_idx });
        invoke_body("with_font", &self.inner, body);
    }

    fn tab_item(&self, label: &str, closable: bool, body: ComRc<IAction>) -> bool {
        self.inner.record(UiCall::TabItem {
            label: label.into(),
            closable,
        });
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
        self.inner.record(UiCall::TextWithFont {
            font_idx,
            s: s.into(),
        });
    }

    fn button(&self, label: &str, w: f32, h: f32) -> bool {
        self.inner.record(UiCall::Button {
            label: label.into(),
            w,
            h,
        });
        self.inner
            .button_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }

    fn checkbox(&self, label: &str, value: bool) -> bool {
        self.inner.record(UiCall::Checkbox {
            label: label.into(),
            value,
        });
        value
    }

    fn slider_int(&self, label: &str, value: i32, min: i32, max: i32) -> i32 {
        self.inner.record(UiCall::SliderInt {
            label: label.into(),
            value,
            min,
            max,
        });
        value
    }

    fn set_next_item_width(&self, w: f32) {
        self.inner.record(UiCall::SetNextItemWidth(w));
    }

    fn image(&self, texture_com_id: i32, w: f32, h: f32) {
        self.inner.record(UiCall::Image {
            texture_com_id,
            w,
            h,
        });
    }

    fn image_fit(&self, texture_com_id: i32, src_w: f32, src_h: f32) {
        self.inner.record(UiCall::ImageFit {
            texture_com_id,
            src_w,
            src_h,
        });
    }

    fn image_uv(&self, texture_com_id: i32, w: f32, h: f32, u0: f32, v0: f32, u1: f32, v1: f32) {
        self.inner.record(UiCall::ImageUv {
            texture_com_id,
            w,
            h,
            u0,
            v0,
            u1,
            v1,
        });
    }

    fn multiline_text(&self, content: &str, w: f32, h: f32) {
        self.inner.record(UiCall::MultilineText {
            content: content.into(),
            w,
            h,
        });
    }

    fn set_text_buffer(&self, key: i32, content: &str) {
        self.inner.record(UiCall::SetTextBuffer {
            key,
            content: content.into(),
        });
    }

    fn show_text_buffer(&self, key: i32, w: f32, h: f32) -> bool {
        self.inner.record(UiCall::ShowTextBuffer { key, w, h });
        // Report the key as unknown so a script under test calls
        // `set_text_buffer` and the recorded stream still captures the
        // text content (matching the real host's first-frame behaviour).
        false
    }

    fn copy_text_buffer(&self, key: i32) {
        self.inner.record(UiCall::CopyTextBuffer { key });
    }

    fn tree_leaf(&self, label: &str, selected: bool) -> bool {
        self.inner.record(UiCall::TreeLeaf {
            label: label.into(),
            selected,
        });
        self.inner
            .tree_leaf_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }

    // The recording host invokes the body once per row to mirror the
    // imgui clipper's per-visible-row callback shape, but without an
    // actual imgui viewport every row is "visible". Tests that need
    // to assert specific visible windows can stub the count.
    fn list_clipped(&self, count: i32, body: ComRc<IAction>) {
        self.inner.record(UiCall::ListClipped { count });
        invoke_body("list_clipped", &self.inner, body);
    }

    fn list_clipped_index(&self) -> i32 {
        -1
    }

    fn tree_node_open(&self, label: &str) -> bool {
        self.inner.record(UiCall::TreeNodeOpen {
            label: label.into(),
        });
        self.inner
            .tree_node_open_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }

    fn tree_pop(&self) {
        self.inner.record(UiCall::TreePop);
    }

    // Mouse-query methods are no-ops in the recording host: the
    // recording UI is only used by tests that don't drive real mouse
    // input, and returning fixed defaults keeps script-side branches
    // predictable.
    fn is_item_hovered(&self) -> bool {
        *self.inner.item_hovered.borrow()
    }
    fn mouse_down(&self, _button: i32) -> bool {
        if let Some(v) = self.inner.mouse_down_script.borrow_mut().pop_front() {
            return v;
        }
        *self.inner.mouse_is_down.borrow()
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
    fn fill_rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, g: f32, b: f32, a: f32) {
        self.inner.record(UiCall::FillRect {
            x0,
            y0,
            x1,
            y1,
            r,
            g,
            b,
            a,
        });
    }
    fn image_rect(
        &self,
        texture_com_id: i32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) {
        self.inner.record(UiCall::ImageRect {
            texture_com_id,
            x0,
            y0,
            x1,
            y1,
            u0,
            v0,
            u1,
            v1,
        });
    }
    fn text_at(&self, x: f32, y: f32, r: f32, g: f32, b: f32, a: f32, s: &str) {
        self.inner.record(UiCall::TextAt {
            x,
            y,
            r,
            g,
            b,
            a,
            s: s.into(),
        });
    }
    fn game_font_size(&self) -> f32 {
        0.0
    }
    fn text_at_small(&self, x: f32, y: f32, r: f32, g: f32, b: f32, a: f32, s: &str) {
        self.inner.record(UiCall::TextAt {
            x,
            y,
            r,
            g,
            b,
            a,
            s: s.into(),
        });
    }
    fn game_font_size_small(&self) -> f32 {
        0.0
    }
    fn style_color(&self, _slot: i32, _r: f32, _g: f32, _b: f32, _a: f32, body: ComRc<IAction>) {
        invoke_body("style_color", &self.inner, body);
    }
    fn set_cursor_pos(&self, _x: f32, _y: f32) {}
    fn cursor_pos_x(&self) -> f32 {
        0.0
    }
    fn cursor_pos_y(&self) -> f32 {
        0.0
    }
    fn display_size_x(&self) -> i32 {
        self.inner.display_size.borrow().0
    }
    fn display_size_y(&self) -> i32 {
        self.inner.display_size.borrow().1
    }
    fn calc_text_size_x(&self, s: &str) -> f32 {
        s.chars().count() as f32 * self.inner.char_size.borrow().0
    }
    fn calc_text_size_y(&self, _s: &str) -> f32 {
        self.inner.char_size.borrow().1
    }
    fn any_key_or_mouse_down(&self) -> bool {
        false
    }
    fn dock_space(&self, _root_id: &str, body: crosscom::ComRc<crosscom::IAction>) {
        body.invoke();
    }
    fn window_docked(&self, _title: &str, body: crosscom::ComRc<crosscom::IAction>) {
        body.invoke();
    }
    fn dock_layout_once(
        &self,
        _root_id: &str,
        _left_window: &str,
        _right_window: &str,
        _bottom_window: &str,
        _center_window: &str,
        _left_ratio: f32,
        _right_ratio: f32,
        _bottom_ratio: f32,
    ) {
    }
    fn dock_layout_assets_once(
        &self,
        _root_id: &str,
        _left_window: &str,
        _right_window: &str,
        _left_ratio: f32,
    ) {
    }
    fn dock_layout_scene_once(
        &self,
        _root_id: &str,
        _inspector_window: &str,
        _hierarchy_window: &str,
        _scene_view_window: &str,
        _resource_window: &str,
        _preview_window: &str,
        _right_ratio: f32,
        _bottom_ratio: f32,
        _hierarchy_ratio: f32,
        _resource_ratio: f32,
    ) {
    }
    fn main_menu_bar(&self, body: crosscom::ComRc<crosscom::IAction>) {
        self.inner.record(UiCall::MainMenuBar);
        invoke_body("main_menu_bar", &self.inner, body);
    }
    fn menu(&self, label: &str, body: crosscom::ComRc<crosscom::IAction>) {
        self.inner.record(UiCall::Menu {
            label: label.into(),
        });
        invoke_body("menu", &self.inner, body);
    }
    fn menu_item(&self, label: &str, selected: bool) -> bool {
        self.inner.record(UiCall::MenuItem {
            label: label.into(),
            selected,
        });
        self.inner
            .menu_item_results
            .borrow()
            .get(label)
            .copied()
            .unwrap_or(false)
    }
}
