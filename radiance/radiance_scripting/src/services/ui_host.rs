//! `ImguiUiHost` — production `IUiHostImpl` backed by imgui-rs.
//!
//! The ComObject itself is stateless: every method routes through a
//! thread-local `ImguiFrameState` pointer that the host (typically
//! `ImguiImmediateDirectorPump::pump`) sets up at the start of each frame and
//! tears down at the end via [`with_imgui_frame`]. This mirrors
//! `crosscom_protosept::scope_context` and keeps the `ComRc<IUiHost>`
//! itself a 'static value the script can intern once and re-use
//! across frames.
//!
//! ## Re-entrancy
//!
//! Each `IUiHostImpl` method enters imgui via `&imgui::Ui`. imgui-rs's
//! `Ui` is `Cell`-backed internally, so aliased `&Ui` borrows that
//! cross the script/host boundary mid-`begin/end` are safe. Texture
//! and per-frame counter state lives in `Cell` / `RefCell` for the
//! same reason — a body closure that re-enters the script and the
//! script calls another `ui.*` method must not deadlock.
//!
//! ## Body closures
//!
//! Pairing widgets receive `body: ComRc<IAction>`. The Rust impl runs
//! `body.invoke()` *inside* the imgui-rs scope guard (which fires
//! `End*` via `Drop` even if the body panics or short-circuits),
//! preserving begin/end pairing safety by construction. Errors from
//! `invoke` already surface to `eprintln!` in `proto_ccw.rs`.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use crosscom::{ComRc, IAction};

use crate::services::texture_resolver::TextureResolver;
use radiance::comdef::{IUiHost, IUiHostImpl};

/// Per-frame backing state shared by every `IUiHost` call. Constructed
/// by the renderer for the duration of a single `render` traversal
/// and parked in a thread-local cell so the ComObject method bodies
/// can recover it.
pub struct ImguiFrameState<'a> {
    pub ui: &'a imgui::Ui,
    pub textures: RefCell<&'a mut dyn TextureResolver>,
    pub fonts: &'a [imgui::FontId],
    pub dpi_scale: f32,
    table_counter: Cell<u32>,
    multiline_counter: Cell<u32>,
    /// Current row index inside an active `list_clipped(...)` body,
    /// or `-1` when no clipper body is on the stack. Scripts read this
    /// via `IUiHost.list_clipped_index()`. A single cell is enough
    /// because clippers are not nested (the body is a flat row
    /// renderer); if a body invokes `list_clipped` recursively the
    /// outer index would be clobbered, which we treat as a misuse.
    list_clipped_index: Cell<i32>,
}

impl<'a> ImguiFrameState<'a> {
    pub fn new(
        ui: &'a imgui::Ui,
        textures: &'a mut dyn TextureResolver,
        fonts: &'a [imgui::FontId],
        dpi_scale: f32,
    ) -> Self {
        Self {
            ui,
            textures: RefCell::new(textures),
            fonts,
            dpi_scale,
            table_counter: Cell::new(0),
            multiline_counter: Cell::new(0),
            list_clipped_index: Cell::new(-1),
        }
    }

    fn scaled_size(&self, w: f32, h: f32) -> [f32; 2] {
        [
            scale_script_dimension(w, self.dpi_scale),
            scale_script_dimension(h, self.dpi_scale),
        ]
    }
}

fn scale_script_dimension(value: f32, dpi_scale: f32) -> f32 {
    if value > 0.0 {
        value * dpi_scale
    } else {
        value
    }
}

thread_local! {
    /// Type-erased pointer to the currently-active `ImguiFrameState`.
    /// Stored as `*mut ImguiFrameState<'static>` because the real
    /// lifetime is tied to the dynamic extent of [`with_imgui_frame`].
    static FRAME: Cell<*mut ImguiFrameState<'static>> = const { Cell::new(std::ptr::null_mut()) };
}

/// Install `frame` as the active per-frame state for the duration of
/// `body`. Nested calls are stacked: the previous pointer is restored
/// on exit (including on panic via the RAII guard).
pub fn with_imgui_frame<'a, R>(frame: &mut ImguiFrameState<'a>, body: impl FnOnce() -> R) -> R {
    // SAFETY: we lifetime-erase the frame for the thread-local but
    // restore the previous value on scope exit (Guard::drop), so the
    // erased pointer never outlives the borrow.
    let raw = frame as *mut ImguiFrameState<'a> as *mut ImguiFrameState<'static>;
    let prev = FRAME.with(|cell| cell.replace(raw));
    struct Guard {
        prev: *mut ImguiFrameState<'static>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            FRAME.with(|cell| cell.set(self.prev));
        }
    }
    let _guard = Guard { prev };
    body()
}

fn with_frame<R>(label: &str, body: impl FnOnce(&ImguiFrameState<'_>) -> R) -> Option<R> {
    let raw = FRAME.with(|cell| cell.get());
    if raw.is_null() {
        eprintln!(
            "ImguiUiHost::{}: called outside with_imgui_frame; script call dropped",
            label
        );
        return None;
    }
    // SAFETY: `raw` is non-null only while `with_imgui_frame` is on
    // the stack, which keeps the underlying `ImguiFrameState` alive
    // and borrowable as `&`. imgui-rs uses interior mutability so
    // aliased `&Ui` is safe; the texture resolver is gated by a
    // RefCell to make script re-entrancy at the script-impl method
    // boundary observable rather than UB.
    Some(body(unsafe { &*(raw as *const ImguiFrameState<'_>) }))
}

/// Stateless production `IUiHost` ComObject. Construct once per
/// scripting session and intern via `ScriptHost::intern` so scripts
/// can hold a stable `box<IUiHost>` handle across frames.
pub struct ImguiUiHost {
    /// Tracks which `dock_layout_once(root_id, ...)` invocations have
    /// already seeded their dock-builder layout this session, so the
    /// helper is idempotent when scripts call it every frame.
    dock_layouts_built: RefCell<HashSet<String>>,
}

ComObject_UiHost!(super::ImguiUiHost);

impl ImguiUiHost {
    pub fn create() -> ComRc<IUiHost> {
        ComRc::<IUiHost>::from_object(ImguiUiHost {
            dock_layouts_built: RefCell::new(HashSet::new()),
        })
    }
}

impl IUiHostImpl for ImguiUiHost {
    fn window(&self, title: &str, w: f32, h: f32, flags: i32, body: ComRc<IAction>) {
        with_frame("window", |f| {
            let [w, h] = f.scaled_size(w, h);
            f.ui.window(title)
                .size([w, h], imgui::Condition::FirstUseEver)
                .flags(imgui::WindowFlags::from_bits_truncate(flags as u32))
                .build(|| {
                    body.invoke();
                });
        });
    }

    fn window_centered(&self, title: &str, w: f32, h: f32, body: ComRc<IAction>) {
        with_frame("window_centered", |f| {
            let [w, h] = f.scaled_size(w, h);
            let [display_w, display_h] = f.ui.io().display_size;
            let cx = (display_w - w) / 2.0;
            let cy = (display_h - h) / 2.0;
            f.ui.window(title)
                .size([w, h], imgui::Condition::Always)
                .position([cx, cy], imgui::Condition::Always)
                .movable(false)
                .title_bar(false)
                .build(|| {
                    body.invoke();
                });
        });
    }

    fn window_fullscreen(&self, title: &str, flags: i32, body: ComRc<IAction>) {
        with_frame("window_fullscreen", |f| {
            let display = f.ui.io().display_size;
            let chrome = imgui::WindowFlags::NO_TITLE_BAR
                | imgui::WindowFlags::NO_RESIZE
                | imgui::WindowFlags::NO_MOVE
                | imgui::WindowFlags::NO_COLLAPSE
                | imgui::WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                | imgui::WindowFlags::NO_NAV_FOCUS
                | imgui::WindowFlags::NO_SCROLLBAR
                | imgui::WindowFlags::NO_SCROLL_WITH_MOUSE;
            let extra = imgui::WindowFlags::from_bits_truncate(flags as u32);
            let pad_token =
                f.ui.push_style_var(imgui::StyleVar::WindowPadding([0.0, 0.0]));
            let rounding_token = f.ui.push_style_var(imgui::StyleVar::WindowRounding(0.0));
            let border_token = f.ui.push_style_var(imgui::StyleVar::WindowBorderSize(0.0));
            f.ui.window(title)
                .position([0.0, 0.0], imgui::Condition::Always)
                .size(display, imgui::Condition::Always)
                .flags(chrome | extra)
                .build(|| {
                    body.invoke();
                });
            border_token.pop();
            rounding_token.pop();
            pad_token.pop();
        });
    }

    fn child_window(&self, id: &str, w: f32, h: f32, body: ComRc<IAction>) {
        with_frame("child_window", |f| {
            let [w, h] = f.scaled_size(w, h);
            f.ui.child_window(id).size([w, h]).build(|| {
                body.invoke();
            });
        });
    }

    fn table(&self, id: &str, cols: i32, body: ComRc<IAction>) {
        with_frame("table", |f| {
            // Disambiguate against repeat table ids per imgui's
            // BeginTable invariant (same id requires same column
            // count). Per-frame monotonic counter scoped to this
            // frame's IUiHost.
            let n = f.table_counter.get();
            f.table_counter.set(n + 1);
            let table_id = format!("{}##ui_host_table_{}", id, n);
            if let Some(_t) = f.ui.begin_table(&table_id, cols as usize) {
                body.invoke();
            }
        });
    }

    fn tab_bar(&self, id: &str, body: ComRc<IAction>) {
        with_frame("tab_bar", |f| {
            let bar = imgui::TabBar::new(id).flags(
                imgui::TabBarFlags::REORDERABLE
                    | imgui::TabBarFlags::FITTING_POLICY_DEFAULT
                    | imgui::TabBarFlags::AUTO_SELECT_NEW_TABS,
            );
            bar.build(f.ui, || {
                body.invoke();
            });
        });
    }

    fn tree_node(&self, label: &str, body: ComRc<IAction>) {
        with_frame("tree_node", |f| {
            if let Some(_node) = f.ui.tree_node_config(label).push() {
                body.invoke();
            }
        });
    }

    fn group(&self, body: ComRc<IAction>) {
        with_frame("group", |f| {
            f.ui.group(|| {
                body.invoke();
            });
        });
    }

    fn style_alpha(&self, alpha: f32, body: ComRc<IAction>) {
        with_frame("style_alpha", |f| {
            let token = f.ui.push_style_var(imgui::StyleVar::Alpha(alpha));
            body.invoke();
            token.pop();
        });
    }

    fn with_font(&self, font_idx: i32, body: ComRc<IAction>) {
        with_frame("with_font", |f| {
            let font = f
                .fonts
                .get(font_idx as usize)
                .copied()
                .unwrap_or_else(|| f.ui.fonts().fonts()[0]);
            let token = f.ui.push_font(font);
            body.invoke();
            token.pop();
        });
    }

    fn tab_item(&self, label: &str, closable: bool, body: ComRc<IAction>) -> bool {
        with_frame("tab_item", |f| {
            let mut opened = true;
            let item = if closable {
                imgui::TabItem::new(label).opened(&mut opened)
            } else {
                imgui::TabItem::new(label)
            };
            item.build(f.ui, || {
                body.invoke();
            });
            // `opened` is set to false by imgui-rs when the user
            // clicks the close button on this frame. If the script
            // declared the tab as non-closable, never report a close.
            closable && !opened
        })
        .unwrap_or(false)
    }

    fn same_line(&self) {
        let _ = with_frame("same_line", |f| f.ui.same_line());
    }

    fn dummy(&self, w: f32, h: f32) {
        let _ = with_frame("dummy", |f| {
            let [w, h] = f.scaled_size(w, h);
            f.ui.dummy([w, h]);
        });
    }

    fn spacer(&self, w: f32, h: f32) {
        let _ = with_frame("spacer", |f| {
            let [w, h] = f.scaled_size(w, h);
            f.ui.dummy([w, h]);
        });
    }

    fn table_next_column(&self) {
        let _ = with_frame("table_next_column", |f| f.ui.table_next_column());
    }

    fn text(&self, s: &str) {
        let _ = with_frame("text", |f| f.ui.text(s));
    }

    fn text_with_font(&self, font_idx: i32, s: &str) {
        let _ = with_frame("text_with_font", |f| {
            let font = f
                .fonts
                .get(font_idx as usize)
                .copied()
                .unwrap_or_else(|| f.ui.fonts().fonts()[0]);
            let token = f.ui.push_font(font);
            f.ui.text(s);
            token.pop();
        });
    }

    fn button(&self, label: &str, w: f32, h: f32) -> bool {
        with_frame("button", |f| {
            let [w, h] = f.scaled_size(w, h);
            f.ui.button_with_size(label, [w, h])
        })
        .unwrap_or(false)
    }

    fn image(&self, texture_com_id: i32, w: f32, h: f32) {
        let _ = with_frame("image", |f| {
            let [w, h] = f.scaled_size(w, h);
            let id = f.textures.borrow_mut().resolve(texture_com_id as i64);
            match id {
                Some(texture_id) => imgui::Image::new(texture_id, [w, h]).build(f.ui),
                None if texture_com_id >= 0 => {
                    imgui::Image::new(imgui::TextureId::new(texture_com_id as usize), [w, h])
                        .build(f.ui)
                }
                None => f.ui.text("[missing texture]"),
            }
        });
    }

    fn image_fit(&self, texture_com_id: i32, src_w: f32, src_h: f32) {
        let _ = with_frame("image_fit", |f| {
            if src_w <= 0.0 || src_h <= 0.0 {
                return;
            }
            let texture_id = f
                .textures
                .borrow_mut()
                .resolve(texture_com_id as i64)
                .or_else(|| {
                    (texture_com_id >= 0).then(|| imgui::TextureId::new(texture_com_id as usize))
                });
            let Some(texture_id) = texture_id else {
                f.ui.text("[missing texture]");
                return;
            };
            let [avail_w, avail_h] = f.ui.content_region_avail();
            if avail_w <= 0.0 || avail_h <= 0.0 {
                return;
            }
            let scale = (avail_w / src_w).min(avail_h / src_h);
            let target = [src_w * scale, src_h * scale];
            let cursor = f.ui.cursor_pos();
            f.ui.set_cursor_pos([
                cursor[0] + (avail_w - target[0]) * 0.5,
                cursor[1] + (avail_h - target[1]) * 0.5,
            ]);
            imgui::Image::new(texture_id, target).build(f.ui);
        });
    }

    fn image_uv(&self, texture_com_id: i32, w: f32, h: f32, u0: f32, v0: f32, u1: f32, v1: f32) {
        let _ = with_frame("image_uv", |f| {
            let [w, h] = f.scaled_size(w, h);
            let id = f.textures.borrow_mut().resolve(texture_com_id as i64);
            let texture_id = match id {
                Some(id) => id,
                None if texture_com_id >= 0 => imgui::TextureId::new(texture_com_id as usize),
                None => {
                    f.ui.text("[missing texture]");
                    return;
                }
            };
            imgui::Image::new(texture_id, [w, h])
                .uv0([u0, v0])
                .uv1([u1, v1])
                .build(f.ui);
        });
    }

    fn multiline_text(&self, content: &str, w: f32, h: f32) {
        let _ = with_frame("multiline_text", |f| {
            let n = f.multiline_counter.get();
            f.multiline_counter.set(n + 1);
            let id = format!("##ui_host_multi_{}", n);
            let [w, h] = f.scaled_size(w, h);
            let mut buf = content.to_string();
            imgui::InputTextMultiline::new(f.ui, &id, &mut buf, [w, h])
                .read_only(true)
                .build();
        });
    }

    fn tree_leaf(&self, label: &str) -> bool {
        with_frame("tree_leaf", |f| {
            let _node = f.ui.tree_node_config(label).leaf(true).push();
            f.ui.is_item_clicked()
        })
        .unwrap_or(false)
    }

    fn list_clipped(&self, count: i32, body: ComRc<IAction>) {
        with_frame("list_clipped", |f| {
            if count <= 0 {
                return;
            }
            // Time the full clipped section (begin + per-row body
            // invocations + end). Cheap when perf is disabled (no
            // syscalls/allocations).
            radiance::perf::time("editor.tree.render_us", || {
                let prev = f.list_clipped_index.replace(-1);
                let clipper = imgui::ListClipper::new(count);
                let tok = clipper.begin(f.ui);
                let mut visible: u64 = 0;
                for row in tok.iter() {
                    f.list_clipped_index.set(row);
                    body.invoke();
                    visible = visible.saturating_add(1);
                }
                f.list_clipped_index.set(prev);
                radiance::perf::count("editor.ui.list_clipped.rows", visible);
            });
        });
    }

    fn list_clipped_index(&self) -> i32 {
        with_frame("list_clipped_index", |f| f.list_clipped_index.get()).unwrap_or(-1)
    }

    fn tree_node_open(&self, label: &str) -> bool {
        with_frame("tree_node_open", |f| {
            let token = f.ui.tree_node_config(label).push();
            let is_open = token.is_some();
            // Defuse the RAII drop guard: the matching `tree_pop`
            // (called by the script after rendering the body) issues
            // the real `igTreePop`. Dropping the token here would
            // pop the imgui scope before the body widgets are
            // submitted, which is exactly what we don't want.
            std::mem::forget(token);
            is_open
        })
        .unwrap_or(false)
    }

    fn tree_pop(&self) {
        let _ = with_frame("tree_pop", |_f| {
            // imgui-rs only exposes TreePop via the RAII token, but
            // it's a thin wrapper over `igTreePop`. We invoke the
            // sys binding directly so callers can pair manually.
            unsafe { imgui::sys::igTreePop() }
        });
    }

    fn is_item_hovered(&self) -> bool {
        with_frame("is_item_hovered", |f| f.ui.is_item_hovered()).unwrap_or(false)
    }

    fn mouse_down(&self, button: i32) -> bool {
        with_frame("mouse_down", |f| {
            f.ui.is_mouse_down(map_mouse_button(button))
        })
        .unwrap_or(false)
    }

    fn mouse_drag_delta_x(&self, button: i32) -> f32 {
        with_frame("mouse_drag_delta_x", |f| {
            f.ui.mouse_drag_delta_with_button(map_mouse_button(button))[0]
        })
        .unwrap_or(0.0)
    }

    fn mouse_drag_delta_y(&self, button: i32) -> f32 {
        with_frame("mouse_drag_delta_y", |f| {
            f.ui.mouse_drag_delta_with_button(map_mouse_button(button))[1]
        })
        .unwrap_or(0.0)
    }

    fn reset_mouse_drag_delta(&self, button: i32) {
        let _ = with_frame("reset_mouse_drag_delta", |f| {
            f.ui.reset_mouse_drag_delta(map_mouse_button(button))
        });
    }

    fn mouse_wheel(&self) -> f32 {
        with_frame("mouse_wheel", |f| f.ui.io().mouse_wheel).unwrap_or(0.0)
    }

    fn content_region_avail_x(&self) -> i32 {
        with_frame("content_region_avail_x", |f| {
            let [w, _] = f.ui.content_region_avail();
            // Return in script-side (unscaled) units to match how
            // widgets accept sizes; the value is meant to be fed back
            // into `image(...)` (which scales it via `scaled_size`) or
            // `IRenderTarget.resize(int, int)`.
            let logical = if f.dpi_scale > 0.0 {
                w / f.dpi_scale
            } else {
                w
            };
            logical.max(0.0) as i32
        })
        .unwrap_or(0)
    }

    fn content_region_avail_y(&self) -> i32 {
        with_frame("content_region_avail_y", |f| {
            let [_, h] = f.ui.content_region_avail();
            let logical = if f.dpi_scale > 0.0 {
                h / f.dpi_scale
            } else {
                h
            };
            logical.max(0.0) as i32
        })
        .unwrap_or(0)
    }

    fn style_color(&self, slot: i32, r: f32, g: f32, b: f32, a: f32, body: ComRc<IAction>) {
        with_frame("style_color", |f| {
            let token = f.ui.push_style_color(map_style_color(slot), [r, g, b, a]);
            body.invoke();
            token.pop();
        });
    }

    fn set_cursor_pos(&self, x: f32, y: f32) {
        let _ = with_frame("set_cursor_pos", |f| {
            // Mirror the `scaled_size` convention used by other
            // widgets: positive values are script-logical pixels
            // multiplied by `dpi_scale`; non-positive values pass
            // through unchanged so callers can emit imgui sentinels
            // like 0.0 or negative offsets if needed.
            let sx = scale_script_dimension(x, f.dpi_scale);
            let sy = scale_script_dimension(y, f.dpi_scale);
            f.ui.set_cursor_pos([sx, sy]);
        });
    }

    fn cursor_pos_x(&self) -> f32 {
        with_frame("cursor_pos_x", |f| {
            let [x, _] = f.ui.cursor_pos();
            if f.dpi_scale > 0.0 {
                x / f.dpi_scale
            } else {
                x
            }
        })
        .unwrap_or(0.0)
    }

    fn cursor_pos_y(&self) -> f32 {
        with_frame("cursor_pos_y", |f| {
            let [_, y] = f.ui.cursor_pos();
            if f.dpi_scale > 0.0 {
                y / f.dpi_scale
            } else {
                y
            }
        })
        .unwrap_or(0.0)
    }

    fn display_size_x(&self) -> i32 {
        with_frame("display_size_x", |f| {
            let [w, _] = f.ui.io().display_size;
            let logical = if f.dpi_scale > 0.0 {
                w / f.dpi_scale
            } else {
                w
            };
            logical.max(0.0) as i32
        })
        .unwrap_or(0)
    }

    fn display_size_y(&self) -> i32 {
        with_frame("display_size_y", |f| {
            let [_, h] = f.ui.io().display_size;
            let logical = if f.dpi_scale > 0.0 {
                h / f.dpi_scale
            } else {
                h
            };
            logical.max(0.0) as i32
        })
        .unwrap_or(0)
    }

    fn calc_text_size_x(&self, s: &str) -> f32 {
        with_frame("calc_text_size_x", |f| {
            let [w, _] = f.ui.calc_text_size(s);
            if f.dpi_scale > 0.0 {
                w / f.dpi_scale
            } else {
                w
            }
        })
        .unwrap_or(0.0)
    }

    fn calc_text_size_y(&self, s: &str) -> f32 {
        with_frame("calc_text_size_y", |f| {
            let [_, h] = f.ui.calc_text_size(s);
            if f.dpi_scale > 0.0 {
                h / f.dpi_scale
            } else {
                h
            }
        })
        .unwrap_or(0.0)
    }

    fn any_key_or_mouse_down(&self) -> bool {
        with_frame("any_key_or_mouse_down", |f| {
            let io = f.ui.io();
            io.keys_down.iter().any(|&k| k) || io.mouse_down.iter().any(|&k| k)
        })
        .unwrap_or(false)
    }

    fn dock_space(&self, root_id: &str, body: ComRc<IAction>) {
        with_frame("dock_space", |f| {
            let display = f.ui.io().display_size;
            let pad_token =
                f.ui.push_style_var(imgui::StyleVar::WindowPadding([0.0, 0.0]));
            let rounding_token = f.ui.push_style_var(imgui::StyleVar::WindowRounding(0.0));
            let border_token = f.ui.push_style_var(imgui::StyleVar::WindowBorderSize(0.0));
            let flags = imgui::WindowFlags::NO_TITLE_BAR
                | imgui::WindowFlags::NO_COLLAPSE
                | imgui::WindowFlags::NO_RESIZE
                | imgui::WindowFlags::NO_MOVE
                | imgui::WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                | imgui::WindowFlags::NO_NAV_FOCUS
                | imgui::WindowFlags::NO_DOCKING;
            f.ui.window(&format!("##{}_host", root_id))
                .position([0.0, 0.0], imgui::Condition::Always)
                .size(display, imgui::Condition::Always)
                .flags(flags)
                .build(|| {
                    // Push WindowBg=transparent for the dockspace call only,
                    // mirroring `radiance_editor::MainPageDirector` so the
                    // central node stays see-through even when populated.
                    let bg_token =
                        f.ui.push_style_color(imgui::StyleColor::WindowBg, [0.0, 0.0, 0.0, 0.0]);
                    let id = dock_string_id(root_id);
                    unsafe {
                        imgui::sys::igDockSpace(
                            id,
                            imgui::sys::ImVec2::new(0.0, 0.0),
                            imgui::sys::ImGuiDockNodeFlags::from_le(
                                imgui::sys::ImGuiDockNodeFlags_PassthruCentralNode as i32,
                            ),
                            ::std::ptr::null::<imgui::sys::ImGuiWindowClass>(),
                        );
                    }
                    bg_token.pop();
                    body.invoke();
                });
            border_token.pop();
            rounding_token.pop();
            pad_token.pop();
        });
    }

    fn window_docked(&self, title: &str, body: ComRc<IAction>) {
        with_frame("window_docked", |f| {
            f.ui.window(title).build(|| {
                body.invoke();
            });
        });
    }

    fn dock_layout_once(
        &self,
        root_id: &str,
        left_window: &str,
        right_window: &str,
        bottom_window: &str,
        center_window: &str,
        left_ratio: f32,
        right_ratio: f32,
        bottom_ratio: f32,
    ) {
        // Idempotent: only seed the layout the first time we see this root_id.
        {
            let mut built = self.dock_layouts_built.borrow_mut();
            if built.contains(root_id) {
                return;
            }
            built.insert(root_id.to_string());
        }
        // Defer to imgui-sys's dock builder. We split off Left first,
        // then Right, then Bottom from the remaining centre node so the
        // ratios are taken with respect to the shrinking centre region.
        // This matches the typical 4-pane editor layout.
        let root = dock_string_id(root_id);
        let size = with_frame("dock_layout_once.size", |f| f.ui.io().display_size)
            .unwrap_or([1280.0, 720.0]);
        unsafe {
            imgui::sys::igDockBuilderRemoveNode(root);
            imgui::sys::igDockBuilderAddNode(
                root,
                imgui::sys::ImGuiDockNodeFlags::from_le(
                    imgui::sys::ImGuiDockNodeFlags_DockSpace as i32,
                ),
            );
            imgui::sys::igDockBuilderSetNodeSize(root, imgui::sys::ImVec2::new(size[0], size[1]));

            let mut left_id: imgui::sys::ImGuiID = 0;
            let mut center_after_left: imgui::sys::ImGuiID = 0;
            imgui::sys::igDockBuilderSplitNode(
                root,
                imgui::sys::ImGuiDir_Left,
                left_ratio.clamp(0.05, 0.9),
                &mut left_id,
                &mut center_after_left,
            );

            let mut right_id: imgui::sys::ImGuiID = 0;
            let mut center_after_right: imgui::sys::ImGuiID = 0;
            // Ratio is relative to the remaining centre node, so scale
            // the user-provided fraction-of-total by 1/(1-left_ratio).
            let remaining_after_left = (1.0 - left_ratio).max(0.05);
            let right_rel = (right_ratio / remaining_after_left).clamp(0.05, 0.9);
            imgui::sys::igDockBuilderSplitNode(
                center_after_left,
                imgui::sys::ImGuiDir_Right,
                right_rel,
                &mut right_id,
                &mut center_after_right,
            );

            let mut bottom_id: imgui::sys::ImGuiID = 0;
            let mut center_id: imgui::sys::ImGuiID = 0;
            let remaining_centre = remaining_after_left * (1.0 - right_rel).max(0.05);
            let bottom_rel = if remaining_centre > 0.0 {
                (bottom_ratio / remaining_centre).clamp(0.05, 0.9)
            } else {
                0.25
            };
            imgui::sys::igDockBuilderSplitNode(
                center_after_right,
                imgui::sys::ImGuiDir_Down,
                bottom_rel,
                &mut bottom_id,
                &mut center_id,
            );

            dock_window(left_window, left_id);
            dock_window(right_window, right_id);
            dock_window(bottom_window, bottom_id);
            dock_window(center_window, center_id);

            imgui::sys::igDockBuilderFinish(root);
        }
    }
}

fn dock_string_id(s: &str) -> imgui::sys::ImGuiID {
    unsafe {
        let p = s.as_ptr() as *const std::os::raw::c_char;
        imgui::sys::igGetID_StrStr(p, p.add(s.len()))
    }
}

fn dock_window(name: &str, node_id: imgui::sys::ImGuiID) {
    if name.is_empty() {
        return;
    }
    let cstr =
        std::ffi::CString::new(name).unwrap_or_else(|_| std::ffi::CString::new("##").unwrap());
    unsafe {
        imgui::sys::igDockBuilderDockWindow(cstr.as_ptr(), node_id);
    }
}

fn map_style_color(slot: i32) -> imgui::StyleColor {
    // Mirror the IDL's documented mapping. Restricted to the slots
    // the title-page port actually needs; out-of-range values fall
    // back to `Text` so a script typo can't take down the renderer.
    match slot {
        0 => imgui::StyleColor::Text,
        2 => imgui::StyleColor::WindowBg,
        21 => imgui::StyleColor::Button,
        22 => imgui::StyleColor::ButtonHovered,
        23 => imgui::StyleColor::ButtonActive,
        _ => imgui::StyleColor::Text,
    }
}

fn map_mouse_button(button: i32) -> imgui::MouseButton {
    match button {
        0 => imgui::MouseButton::Left,
        1 => imgui::MouseButton::Right,
        2 => imgui::MouseButton::Middle,
        3 => imgui::MouseButton::Extra1,
        4 => imgui::MouseButton::Extra2,
        _ => imgui::MouseButton::Left,
    }
}
