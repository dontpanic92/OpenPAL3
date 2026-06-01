use std::rc::Rc;

use imgui::{Condition, Ui};
use radiance::radiance::UiManager;

use crate::openpal4::asset_loader::ImageSetImage;
use crate::ui::dialog_markup::{self, Segment};

/// Widget-default text colour used both as the fallback for runs that
/// aren't wrapped in any `<colour>` tag and as the placeholder for every
/// `<dcN>` palette lookup. See [`dialog_markup::parse`] for the rationale
/// — once the dialog widget's CEGUI `TextColours` palette is wired
/// through, the per-`dcN` lookup will replace this placeholder.
const DEFAULT_TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

pub struct DialogBox {
    ui: Rc<UiManager>,
    avatar: Option<ImageSetImage>,
    avatar_position: AvatarPosition,
    height_factor: f32,
    /// Parsed coloured runs of the current line. Rendered by
    /// [`DialogBoxPresenter`] via a per-character layout loop so each
    /// segment renders in its own colour while still wrapping inside
    /// the dialog window.
    segments: Vec<Segment>,
    /// Tag-stripped form of the current line, returned by
    /// [`Self::text`] and consumed by the agent snapshot so external
    /// observers see clean text without having to parse markup.
    visible_text: String,
    /// `true` while a `talk()` continuation is actively driving this
    /// dialog box (i.e. text is being shown and we're waiting for the
    /// player to advance). Set by [`Self::set_text`] and cleared by
    /// [`Self::close`] from the talk continuation tail. External
    /// observers (the agent server, debug overlays) read it via
    /// [`Self::is_active`] to decide whether a dialog is on-screen.
    active: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AvatarPosition {
    Left,
    Right,
}

impl DialogBox {
    pub fn new(ui: Rc<UiManager>) -> Self {
        Self {
            ui,
            avatar: None,
            avatar_position: AvatarPosition::Left,
            height_factor: 0.2,
            segments: Vec::new(),
            visible_text: String::new(),
            active: false,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        let with_break = insert_speaker_break(text);
        self.segments = dialog_markup::parse(&with_break, DEFAULT_TEXT_COLOR);
        self.visible_text = dialog_markup::strip(&with_break);
        self.active = true;
    }

    /// Mark the dialog as dismissed and clear the rendered text + any
    /// avatar portrait. Called by the `talk()` continuation tail so the
    /// "is a dialog on-screen" signal flips back to `false` and the
    /// last-shown line stops leaking into the agent state snapshot.
    pub fn close(&mut self) {
        self.active = false;
        self.segments.clear();
        self.visible_text.clear();
        self.avatar = None;
        self.avatar_position = AvatarPosition::Left;
    }

    /// `true` while a `talk()` continuation is driving this dialog
    /// (text set + waiting for advance input). Used by the agent
    /// server's `DialogSnapshot.open` field.
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_avatar(&mut self, avatar: Option<ImageSetImage>, position: AvatarPosition) {
        self.avatar = avatar;
        self.avatar_position = position;
    }

    /// Currently displayed dialog text, **with PAL4 markup tags
    /// stripped** (so e.g. `<colour>` / `<dcN>` runs collapse to their
    /// visible text). Empty when no `talk()` has run yet or when the
    /// last `talk()` continuation has dismissed the box via
    /// [`Self::close`]. Used by the agent snapshot to feed
    /// `/v1/state.dialog.text` a clean, automation-friendly string.
    pub fn text(&self) -> &str {
        &self.visible_text
    }

    /// `true` while an avatar portrait is attached to the dialog.
    /// Independent of [`Self::is_active`] because a `talk()` can run
    /// without a portrait (narration); use `is_active` for "is a
    /// dialog on-screen" and `has_avatar` for "should the avatar
    /// slot render".
    #[allow(dead_code)]
    pub fn has_avatar(&self) -> bool {
        self.avatar.is_some()
    }

    pub fn avatar_position(&self) -> AvatarPosition {
        self.avatar_position
    }
}

/// Insert a newline after the first speaker-name colon (`：`) so the
/// speaker's name appears on its own line above the dialog body, as the
/// original CEGUI presentation did. The transform is idempotent: if the
/// raw payload already has `：\n`, we don't double-space.
fn insert_speaker_break(text: &str) -> String {
    if let Some(idx) = text.find('：') {
        let after = idx + '：'.len_utf8();
        let already_has_break = text[after..].chars().next() == Some('\n');
        if !already_has_break {
            let mut out = String::with_capacity(text.len() + 1);
            out.push_str(&text[..after]);
            out.push('\n');
            out.push_str(&text[after..]);
            return out;
        }
    }
    text.to_string()
}

pub struct DialogBoxPresenter;

impl DialogBoxPresenter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&self, dialog_box: &DialogBox, _delta_sec: f32) {
        let ui = dialog_box.ui.ui();

        let [window_width, window_height] = ui.io().display_size;
        let (dialog_x, dialog_width) = {
            if window_width / window_height > 4. / 3. {
                let dialog_width = window_height / 3. * 4.;
                let dialog_x = (window_width - dialog_width) / 2.;
                (dialog_x, dialog_width)
            } else {
                (0., window_width)
            }
        };

        let dialog_height = window_height * dialog_box.height_factor;
        let dialog_y = window_height * (1. - dialog_box.height_factor);

        let avatar_size = if let Some(avatar) = &dialog_box.avatar {
            let height = dialog_height * 1.8;
            let width = height * avatar.width as f32 / avatar.height as f32;
            [width, height]
        } else {
            [0., 0.]
        };

        let text_margins = if dialog_box.avatar_position == AvatarPosition::Left {
            (avatar_size[0] + 10., 10.)
        } else {
            (10., avatar_size[0] + 10.)
        };

        let wrap_width = (dialog_width - text_margins.0 - text_margins.1).max(1.0);

        basic_dlg_box(ui, "dlg_box")
            .draw_background(true)
            .position([dialog_x, dialog_y], Condition::Always)
            .size([dialog_width, dialog_height], Condition::Always)
            .build(|| {
                render_segments(ui, &dialog_box.segments, text_margins.0, wrap_width);
            });

        if let Some(avatar) = &dialog_box.avatar {
            let avatar_position = if dialog_box.avatar_position == AvatarPosition::Left {
                [dialog_x, window_height - avatar_size[1]]
            } else {
                [
                    dialog_x + dialog_width - avatar_size[0],
                    window_height - avatar_size[1],
                ]
            };

            let mut uv0 = [
                avatar.x as f32 / avatar.sprite.width() as f32,
                avatar.y as f32 / avatar.sprite.height() as f32,
            ];
            let mut uv1 = [
                (avatar.x + avatar.width) as f32 / avatar.sprite.width() as f32,
                (avatar.y + avatar.height) as f32 / avatar.sprite.height() as f32,
            ];

            if dialog_box.avatar_position == AvatarPosition::Right {
                std::mem::swap(&mut uv0[0], &mut uv1[0]);
            }

            let _tok = ui.push_style_var(imgui::StyleVar::WindowPadding([0., 0.]));
            basic_dlg_box(ui, "avatar_box")
                .draw_background(false)
                .position(avatar_position, Condition::Always)
                .size(avatar_size, Condition::Always)
                .build(|| {
                    imgui::Image::new(avatar.sprite.imgui_texture_id(), avatar_size)
                        .uv0(uv0)
                        .uv1(uv1)
                        .build(ui);
                });
        }
    }
}

/// Render coloured segments using a per-character layout loop:
///
/// * Each character is measured with `calc_text_size`; we break to a
///   new line whenever the next character would exceed `wrap_width`,
///   or whenever the source text contains a literal `\n`.
/// * Coloured runs are emitted via `text_colored` so each segment
///   renders in its own RGBA. Adjacent characters are positioned by
///   setting the cursor explicitly, so item-spacing style settings
///   can't introduce gaps between adjacent glyphs.
/// * Per-character breaking is appropriate for PAL4's CJK-heavy
///   dialog: every CJK glyph is a valid break point, and ASCII runs
///   still wrap (without word boundaries — acceptable here).
fn render_segments(ui: &Ui, segments: &[Segment], margin_x: f32, wrap_width: f32) {
    let line_height = ui.text_line_height();
    let mut line_x = 0.0_f32;
    let mut line_y = 0.0_f32;

    let mut buf = [0u8; 4];
    for segment in segments {
        for ch in segment.text.chars() {
            if ch == '\n' {
                line_x = 0.0;
                line_y += line_height;
                continue;
            }
            let s: &str = ch.encode_utf8(&mut buf);
            let [w, _] = ui.calc_text_size(s);
            if line_x > 0.0 && line_x + w > wrap_width {
                line_x = 0.0;
                line_y += line_height;
            }
            ui.set_cursor_pos([margin_x + line_x, line_y]);
            ui.text_colored(segment.color, s);
            line_x += w;
        }
    }
}

fn basic_dlg_box<'a>(ui: &'a Ui, name: &'static str) -> imgui::Window<'a, 'a, &'static str> {
    ui.window(name)
        .collapsible(false)
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .focused(false)
        .no_decoration()
}

#[cfg(test)]
mod tests {
    use super::insert_speaker_break;

    #[test]
    fn inserts_newline_after_speaker_colon() {
        assert_eq!(insert_speaker_break("云天河：abc"), "云天河：\nabc");
    }

    #[test]
    fn idempotent_when_break_already_present() {
        assert_eq!(insert_speaker_break("云天河：\nabc"), "云天河：\nabc");
    }

    #[test]
    fn no_change_when_no_full_width_colon() {
        assert_eq!(insert_speaker_break("plain narration"), "plain narration");
    }

    #[test]
    fn only_first_colon_gets_a_break() {
        assert_eq!(
            insert_speaker_break("a：b：c"),
            "a：\nb：c",
            "only the speaker-name colon (first one) should split"
        );
    }
}
