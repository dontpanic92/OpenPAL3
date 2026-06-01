//! Wire protocol for the agent server.
//!
//! All commands and responses use serde's external-tag layout so the
//! JSON stays self-describing without depending on `#[serde(tag = …)]`
//! gymnastics. The transport layer maps each HTTP route directly to a
//! constructor and back; clients that prefer JSON-RPC over a single
//! `POST /v1/rpc` can send `{"command": <AgentCommand>}` and read back
//! the matching `AgentResponse`.
//!
//! ## Stability
//!
//! The enum is `#[non_exhaustive]` so new commands can land in minor
//! releases without breaking clients that match on `_`. Field names
//! are part of the public contract — rename via `#[serde(rename = …)]`
//! if the Rust identifier needs to change.

use serde::{Deserialize, Serialize};

/// Top-level agent command. One per request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentCommand {
    /// Read the current game state snapshot.
    GetState,
    /// Inject a key event into the synthetic input bridge.
    KeyInput(KeyInputParams),
    /// Set an axis state (e.g. gamepad stick) into the synthetic
    /// input bridge.
    AxisInput(AxisInputParams),
    /// Teleport a player slot to an absolute world position.
    TeleportPlayer(TeleportParams),
    /// Advance the currently open dialog box (equivalent to pressing
    /// the dialog-advance key).
    AdvanceDialog,
    /// Pause the game loop. While paused, [`StepTime`] is the only way
    /// to make progress.
    PauseTime,
    /// Resume the game loop at the platform's real frame rate.
    ResumeTime,
    /// Advance the game N fixed-step frames. Only valid when paused.
    StepTime(StepTimeParams),
    /// Toggle plot fast-forward (skips scripted waits).
    FastForward(FastForwardParams),
    /// Save the current game state to the given slot.
    SaveSlot(SlotParams),
    /// Load a previously saved game state from the given slot.
    LoadSlot(SlotParams),
    /// Read a slice of the ring-buffered log.
    LogTail(LogTailParams),
    /// Capture a PNG screenshot of the current framebuffer.
    Screenshot,
    /// Invoke a whitelisted `gi*` script function with literal args.
    ScriptEval(ScriptEvalParams),
}

/// Top-level agent response. Mirrors [`AgentCommand`] roughly but with
/// payload variants that carry returned data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentResponse {
    /// Generic acknowledgement with no payload.
    Ok,
    /// State snapshot reply.
    State(StateSnapshot),
    /// Log-tail reply.
    Log(LogTailResponse),
    /// Screenshot reply: PNG bytes encoded as base64.
    Screenshot(ScreenshotResponse),
    /// Result of [`AgentCommand::ScriptEval`].
    Script(ScriptEvalResponse),
    /// Operation failed.
    Error(AgentError),
}

/// Single key event with edge semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInputParams {
    /// Key name. Case-insensitive; e.g. `"F"`, `"Up"`, `"Space"`. The
    /// list of recognized names is `radiance::input::Key` minus
    /// `Unknown`.
    pub key: String,
    pub action: KeyAction,
}

/// What to do with the synthetic key for the next frame.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    /// Begin a hold: future frames see `is_down = true` until [`Up`].
    Down,
    /// End a hold: clears `is_down`.
    Up,
    /// One-frame tap: emits `pressed` for the next frame and `released`
    /// the frame after.
    Tap,
}

/// Axis push (e.g. left stick).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisInputParams {
    /// Axis name (e.g. `"LeftStickX"`).
    pub axis: String,
    /// Value in the canonical [-1.0, 1.0] range.
    pub value: f32,
}

/// Absolute teleport for the given player slot (`0..PLAYER_COUNT`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleportParams {
    pub player: i32,
    pub pos: [f32; 3],
}

/// Fixed-step tick request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTimeParams {
    /// Number of frames to advance.
    pub frames: u32,
    /// Per-frame delta in seconds. `None` defaults to 1/60.
    #[serde(default)]
    pub dt: Option<f32>,
}

/// `fast_forward` toggle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FastForwardParams {
    pub on: bool,
}

/// Slot index. Matches the existing `Pal4PersistentState::save` shape.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SlotParams {
    pub slot: i32,
}

/// Log-tail query.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LogTailParams {
    /// Return only records with `seq > after_seq`. Defaults to 0.
    #[serde(default)]
    pub after_seq: u64,
    /// Hard cap on returned records. Defaults to 256.
    #[serde(default)]
    pub n: Option<usize>,
}

/// Log-tail reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTailResponse {
    /// One past the last returned `seq` (== monotonic sink counter).
    /// Clients pass this back as the next `after_seq`.
    pub next_seq: u64,
    /// Set when the sink wrapped past unread records; clients should
    /// surface a "log records dropped" warning if true.
    pub dropped: bool,
    pub records: Vec<LogRecordPayload>,
}

/// JSON shape of a single buffered log record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecordPayload {
    pub seq: u64,
    /// ISO-8601 timestamp (UTC). Optional because we don't pull a
    /// `time` crate just for the agent server — the `transport` layer
    /// fills this in when available.
    #[serde(default)]
    pub ts: Option<String>,
    /// `"error" | "warn" | "info" | "debug" | "trace"`.
    pub level: String,
    pub target: String,
    pub msg: String,
}

/// Screenshot payload.
///
/// The raw RGBA bytes are carried through serde-skipped fields so the
/// dispatcher can hand them to the transport thread without paying for
/// JSON serialization of a large `Vec<u8>`. The transport detects this
/// variant and emits a binary `image/png` response directly.
///
/// JSON consumers (tests, generic clients) still see the metadata stub
/// — `width`, `height`, and an `encoded: false` marker indicating that
/// the bytes were stripped during serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenshotResponse {
    pub width: u32,
    pub height: u32,
    /// `true` when the payload would have carried image bytes (the
    /// transport will have already emitted them in a separate binary
    /// response). Always `false` over the wire because the bytes
    /// themselves are not part of the JSON.
    #[serde(default)]
    pub encoded: bool,
    /// Raw RGBA8 pixels in row-major order. Skipped during
    /// serialization — see the struct docs.
    #[serde(skip)]
    pub rgba: Vec<u8>,
}

/// Whitelisted script invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEvalParams {
    /// Function name, e.g. `"giAddMoney"`. Must match the
    /// session's allow-list.
    pub function: String,
    /// Literal positional arguments. Each value must be a JSON
    /// `number` or `string`; the session adapter converts to the
    /// AngelScript stack types.
    #[serde(default)]
    pub args: Vec<serde_json::Value>,
}

/// Script invocation reply: the function name and any returned value
/// (the legacy `gi*` ABI has at most one return).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEvalResponse {
    pub function: String,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
}

/// State snapshot returned by `/v1/state`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Monotonic frame counter. Increments once per game tick;
    /// useful for clients to detect whether a step actually advanced.
    pub frame: u64,
    pub scene: String,
    pub block: String,
    pub leader: i32,
    pub leader_pos: [f32; 3],
    pub party: Vec<PartyMember>,
    pub money: i32,
    pub quest_percentage: i32,
    pub dialog: DialogSnapshot,
    pub fast_forward: bool,
    pub paused: bool,
    /// `None` when no script is currently executing.
    pub current_script_fn: Option<String>,
    pub fps: f32,
    pub dt: f32,
}

/// Per-party-member subset of [`StateSnapshot`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartyMember {
    /// Slot index (0..PLAYER_COUNT).
    pub slot: usize,
    pub level: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub in_team: bool,
}

/// Current dialog state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DialogSnapshot {
    pub open: bool,
    pub text: String,
    /// `"left"`, `"right"`, or `""` when no avatar.
    pub avatar: String,
}

/// Error envelope. Used both as a JSON body and to choose the HTTP
/// status code in `transport`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentError {
    pub kind: AgentErrorKind,
    pub message: String,
}

/// Coarse error category.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorKind {
    /// Bad payload (parse error, unknown key name, …). Maps to HTTP 400.
    BadRequest,
    /// Valid command but rejected because of game state (e.g. step
    /// while running). Maps to HTTP 409.
    Conflict,
    /// Feature explicitly not implemented for the active session
    /// (e.g. screenshot in headless mode). Maps to HTTP 501.
    NotImplemented,
    /// Authorization rejected. Maps to HTTP 401.
    Unauthorized,
    /// Anything else. Maps to HTTP 500.
    Internal,
}

impl AgentError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            kind: AgentErrorKind::BadRequest,
            message: msg.into(),
        }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            kind: AgentErrorKind::Conflict,
            message: msg.into(),
        }
    }

    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self {
            kind: AgentErrorKind::NotImplemented,
            message: msg.into(),
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            kind: AgentErrorKind::Unauthorized,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            kind: AgentErrorKind::Internal,
            message: msg.into(),
        }
    }
}

impl AgentResponse {
    pub fn err(err: AgentError) -> Self {
        Self::Error(err)
    }
}
