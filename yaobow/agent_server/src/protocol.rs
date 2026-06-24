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
    ///
    /// This is the single public load command (`POST /v1/load`). It
    /// auto-routes per the active mode: when a playthrough is live the
    /// game restores it **in-place**; when at the start menu the
    /// dispatcher boots a **fresh** director from the slot (equivalent to
    /// the internal [`Self::EnterLoadGame`] intent).
    LoadSlot(SlotParams),
    /// Read a slice of the ring-buffered log.
    LogTail(LogTailParams),
    /// Capture a PNG screenshot of the current framebuffer.
    Screenshot,
    /// Invoke a whitelisted `gi*` script function with literal args.
    ScriptEval(ScriptEvalParams),
    /// List the EVF event triggers for the currently loaded block,
    /// with their handler function names and world-space bounding
    /// boxes. Lets an automation driver answer "what scripted
    /// transitions exist right here?" without re-parsing EVF.
    GetSceneTriggers,
    /// List the GOB objects and NPCs for the currently loaded block,
    /// including the per-object `research_function` (the script fired
    /// by an "Examine" action). The complement of [`Self::GetSceneTriggers`]
    /// — between the two an agent has every plot-pushing surface in
    /// the current block.
    GetSceneObjects,
    /// Read a (windowed) slice of the AngelScript shared globals.
    /// Story-plot flags live here; diffing this between actions is
    /// the canonical "did the plot advance?" signal.
    GetScriptGlobals(ScriptGlobalsParams),
    /// Fire an EVF trigger by name as if the leader had walked into
    /// it: routes through the same `set_function_by_name2` path the
    /// engine uses on a real trigger collision.
    ///
    /// Accepts both the legacy `{name}` body and the richer
    /// [`FireTriggerParams`] shape with optional `wait_until_idle` /
    /// `collect_trace` / `timeout_ms` fields. The two are
    /// JSON-compatible thanks to `#[serde(default)]` on the new
    /// fields.
    FireSceneTrigger(FireTriggerParams),
    /// Fire a GOB entry's `research_function` as if the player had
    /// pressed "Examine" on it. Returns `bad_request {no_handler}`
    /// when the entry has no examine callback.
    InteractObject(NameParams),
    /// Start capturing the AngelScript VM execution trace into the
    /// agent-server ring buffer. Idempotent — re-calling while
    /// already capturing keeps the existing buffer.
    TraceStart(TraceStartParams),
    /// Stop capturing. Already-buffered events remain drainable via
    /// [`Self::TraceDrain`] until they are evicted or the buffer is
    /// reset by another [`Self::TraceStart`].
    TraceStop,
    /// Read a windowed slice of buffered trace events with
    /// `seq > after_seq`. Mirrors the [`LogTail`](Self::LogTail)
    /// pattern: the response carries a `next_seq` cursor and a
    /// `dropped` flag that warns when records were evicted before
    /// the caller polled.
    TraceDrain(TraceDrainParams),
    /// Buffer a choice index for the next
    /// `giSelectDialogGetLastSelect` /
    /// `giCommonDialogGetLastSelect` call. The buffered choice is
    /// consumed by the next dialog read; supply a fresh choice
    /// before every menu fire.
    ChooseDialog(DialogChooseParams),
    /// Buffer a `(scene, block)` destination for the next
    /// `giShowWorldMap` continuation tick. Surfaced via
    /// `/v1/state.world_map_open`; while open, the script is
    /// suspended in a `Yield` and will not idle until a choice is
    /// supplied. Consumed on the next continuation tick.
    ChooseWorldMap(WorldMapChooseParams),
    /// Snapshot every `radiance::perf` metric tracked on the game
    /// thread (timings, counters, gauges). Returned as a flat
    /// `Vec<{name, kind, …}>`; the agent then diffs successive
    /// snapshots to derive per-window rates. Always callable even
    /// when `OPENPAL3_PERF` is disabled at boot (returns an empty
    /// list in that case so callers don't have to special-case
    /// the disabled path).
    GetPerfMetrics,

    /// Start a fresh story playthrough (New Game) — replaces the active
    /// director with the PAL4 story director. Handled by the app-lifetime
    /// dispatcher (which owns the `SceneManager`); works from the start
    /// menu or as a restart from story.
    EnterNewGame,

    /// Internal "fresh-boot from save slot" intent. Like
    /// [`Self::EnterNewGame`] but boots directly into save `slot`.
    ///
    /// No longer has a dedicated HTTP route: `POST /v1/load`
    /// ([`Self::LoadSlot`]) maps onto this intent automatically when it
    /// is issued at the start menu. Retained as a variant so the
    /// per-game dispatchers can express the fresh-boot path internally.
    EnterLoadGame(SlotParams),

    /// Quit the application.
    ExitGame,

    /// Enable / disable the free-fly debug camera. While enabled the
    /// plot is frozen (the script VM and scripted camera stop
    /// advancing) so the scene can be inspected and the camera posed
    /// arbitrarily. Generic across games; currently dispatched only by
    /// PAL5.
    SetDebugCamera(DebugCameraParams),

    /// Place the scene camera at an absolute eye position looking at an
    /// absolute target point. Stable only while the debug camera is
    /// enabled (plot frozen); otherwise scripted camera commands may
    /// overwrite the pose on the next frame. Generic across games;
    /// currently dispatched only by PAL5.
    SetCamera(CameraPoseParams),
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
    /// Snapshot reply for [`AgentCommand::GetSceneTriggers`].
    SceneTriggers(SceneTriggersResponse),
    /// Snapshot reply for [`AgentCommand::GetSceneObjects`].
    SceneObjects(SceneObjectsResponse),
    /// Snapshot reply for [`AgentCommand::GetScriptGlobals`].
    ScriptGlobals(ScriptGlobalsResponse),
    /// Reply for [`AgentCommand::FireSceneTrigger`] when the caller
    /// asked for `wait_until_idle` / `collect_trace`. Legacy plain
    /// fire-and-return calls still resolve to [`Self::Ok`].
    FireTrigger(FireTriggerResponse),
    /// Snapshot reply for [`AgentCommand::TraceDrain`].
    TraceDrain(TraceDrainResponse),
    /// Snapshot reply for [`AgentCommand::GetPerfMetrics`].
    PerfMetrics(PerfMetricsResponse),
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

/// Toggle for the free-fly debug camera (plot freeze).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DebugCameraParams {
    pub enabled: bool,
}

/// Absolute camera pose: eye position + look-at target point, both in
/// world space.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraPoseParams {
    /// Camera eye position `[x, y, z]`.
    pub eye: [f32; 3],
    /// World-space point the camera looks at `[x, y, z]`.
    pub target: [f32; 3],
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
    /// `true` while the AngelScript VM has a call on its stack
    /// (typically a cutscene / event handler). Stays `true` across
    /// `Yield` waits where [`Self::current_script_fn`] may not be
    /// observable. External drivers should treat `script_running` as
    /// the authoritative "engine is busy with a scripted sequence"
    /// signal.
    #[serde(default)]
    pub script_running: bool,
    /// `true` while a `play_movie()` call is driving the video
    /// player. Set independently of `script_running` so agents can
    /// distinguish "movie cutscene" from "scripted dialogue".
    #[serde(default)]
    pub movie_playing: bool,
    pub fps: f32,
    pub dt: f32,
    /// Player inventory as `{id, count}` pairs, sorted by `id`
    /// for deterministic rendering. Empty `Vec` is the canonical
    /// "no items" state, not a missing field.
    #[serde(default)]
    pub inventory: Vec<InventoryEntry>,
    /// `true` while a `giShowWorldMap` continuation is waiting for
    /// a destination pick. When `true`, `script_running` will also
    /// be `true` (the VM is suspended in the Yield) — fire
    /// [`AgentCommand::ChooseWorldMap`] to unblock. While the map
    /// is open the engine accepts no other trigger fires until the
    /// continuation completes.
    #[serde(default)]
    pub world_map_open: bool,
    /// `true` while the free-fly debug camera is enabled (plot frozen).
    #[serde(default)]
    pub debug_camera: bool,
    /// Current camera eye position `[x, y, z]` in world space.
    #[serde(default)]
    pub camera_eye: [f32; 3],
    /// Current camera look-at target `[x, y, z]` in world space.
    #[serde(default)]
    pub camera_target: [f32; 3],
}

/// One inventory line item in [`StateSnapshot::inventory`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct InventoryEntry {
    /// Equipment / item ID — the same value the script-side
    /// `giAddEquip` / `giHasEquip` family of sysfns operate on.
    pub id: i32,
    /// Number of copies held. Always `> 0`; entries with `0` count
    /// are pruned at snapshot time.
    pub count: i32,
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
    /// Items queued for the next select-dialog (`giSelectDialogAddItem`).
    /// Empty `Vec` is the canonical "no choices available" state.
    /// The planner picks one via `POST /v1/dialog/choose {index}`.
    #[serde(default)]
    pub choices: Vec<String>,
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

/// Generic `{ "name": "…" }` payload used by direct-fire commands
/// (kept for backward compatibility / [`AgentCommand::InteractObject`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameParams {
    pub name: String,
}

/// Body shape accepted by `POST /v1/scene/fire_trigger`. Strict
/// superset of [`NameParams`]: a JSON `{ "name": "X" }` body parses
/// into this with the new fields defaulted, so older clients still
/// work unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireTriggerParams {
    pub name: String,
    /// When `true`, the director defers the response until the VM
    /// becomes idle (`script_running == false` for the
    /// `idle_settle_frames` window) or until `timeout_ms` elapses.
    /// Solves the gdiff race described in `generated/issues.md` A1
    /// — the planner can read globals / scene state in the reply
    /// frame and know the fire has fully settled.
    #[serde(default)]
    pub wait_until_idle: bool,
    /// When `true` (and `wait_until_idle` is set), the response also
    /// carries the trace-ring cursor range produced by the fire so
    /// callers can drain just this fire's events.
    #[serde(default)]
    pub collect_trace: bool,
    /// Maximum wait when `wait_until_idle` is set. Defaults to
    /// 5_000 ms; capped at 30_000 ms server-side.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

impl From<NameParams> for FireTriggerParams {
    fn from(p: NameParams) -> Self {
        Self {
            name: p.name,
            wait_until_idle: false,
            collect_trace: false,
            timeout_ms: None,
        }
    }
}

/// Reply for [`AgentCommand::FireSceneTrigger`] with
/// `wait_until_idle = true`. The legacy plain-fire path still
/// returns [`AgentResponse::Ok`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FireTriggerResponse {
    /// The name that was fired (echoed for client correlation).
    pub name: String,
    /// `true` when the VM became idle on its own within the
    /// timeout; `false` when the timeout elapsed and the fire is
    /// still in progress (the response is still safe to read — it
    /// just means the trace may grow further).
    pub settled: bool,
    /// Lowest trace-ring `seq` produced by this fire (the cursor
    /// the planner should pass as `after_seq` to
    /// `/v1/script/trace/drain` to read only this fire's events).
    /// `None` when `collect_trace` was not requested or no trace
    /// sink was capturing.
    #[serde(default)]
    pub trace_seq_start: Option<u64>,
    /// Highest trace-ring `seq` produced so far (exclusive upper
    /// bound). `None` under the same conditions as
    /// [`Self::trace_seq_start`].
    #[serde(default)]
    pub trace_seq_end: Option<u64>,
    /// Number of game-thread frames the dispatcher waited before
    /// returning. Useful for diagnosing long-running fires.
    pub waited_frames: u32,
    /// Name of the script function the VM is currently executing,
    /// or `None` if idle.
    #[serde(default)]
    pub current_script_fn: Option<String>,
}

/// Optional window over the global-variable array.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ScriptGlobalsParams {
    /// First index to include (default 0).
    #[serde(default)]
    pub start: usize,
    /// Hard cap on returned slots. `None` returns everything from
    /// `start` to the end.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// One EVF trigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerEntry {
    /// EVF-side trigger name (e.g. `"ev01"`).
    pub name: String,
    /// Script function fired on collision.
    pub function: String,
    /// World-space centroid (mean of the vertex centers).
    pub center: [f32; 3],
    /// World-space half-extents derived from the vertex AABB.
    pub half_size: [f32; 3],
    /// `"box"` for 8-vertex triggers, `"plane"` for 4-vertex, `"other"`
    /// for the few EVF entries the engine itself skips. Agents
    /// should generally ignore `"other"` triggers — the live engine
    /// doesn't build collision for them.
    pub shape: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneTriggersResponse {
    pub scene: String,
    pub block: String,
    pub triggers: Vec<TriggerEntry>,
}

/// One NPC entry (loaded from `*.npc`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcEntry {
    /// Logical NPC name (the key scripts pass to `giSetNpcVisible`).
    pub name: String,
    /// Live world-space position of the NPC entity. Reflects script
    /// teleports / animations rather than the load-time position.
    pub position: [f32; 3],
    /// `true` while the entity is visible (live, not just default).
    pub visible: bool,
}

/// One GOB object entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectEntry {
    /// Logical object name (the key scripts pass to
    /// `giSetObjectVisible`).
    pub name: String,
    /// Coarse object kind (`generic`/`action`/`get_item`/…) from
    /// `GobObjectType::name`, or `"unknown"`.
    pub kind: String,
    /// Live world-space position of the object entity.
    pub position: [f32; 3],
    /// `true` while the entity is currently visible.
    pub visible: bool,
    /// Script function called on "Examine", or empty string if the
    /// entry has no interaction handler.
    pub research_function: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneObjectsResponse {
    pub scene: String,
    pub block: String,
    pub npcs: Vec<NpcEntry>,
    pub objects: Vec<ObjectEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScriptGlobalsResponse {
    /// Total length of the underlying global array (independent of
    /// `start`/`limit` window). Lets clients detect a size change
    /// (e.g. after a module reload).
    pub len: usize,
    /// Index of the first returned slot.
    pub start: usize,
    /// Returned slots; `globals[i]` corresponds to global index
    /// `start + i`.
    pub globals: Vec<u32>,
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

// ---- trace types --------------------------------------------------------
//
// Mirrors the VM-side `TraceEvent` from
// `yaobow/shared/src/scripting/angelscript/trace.rs`. The agent_server
// crate intentionally redefines them here (rather than depending on
// `shared`) so the protocol stays free of game-specific code.

/// Parameters for [`AgentCommand::TraceStart`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TraceStartParams {
    /// Ring-buffer capacity (max retained events). `None` keeps the
    /// adapter's default (typically 65_536).
    #[serde(default)]
    pub capacity: Option<usize>,
    /// When `true`, reset the buffer + sequence counter at start.
    /// Default `true` — the common use case is "begin a fresh
    /// capture window".
    #[serde(default = "default_reset_on_start")]
    pub reset: bool,
}

fn default_reset_on_start() -> bool {
    true
}

/// Parameters for [`AgentCommand::TraceDrain`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TraceDrainParams {
    /// Return only events with `seq > after_seq`. Defaults to 0.
    #[serde(default)]
    pub after_seq: u64,
    /// Cap on returned events. Defaults to 1024 server-side.
    #[serde(default)]
    pub n: Option<usize>,
}

/// Response payload for [`AgentCommand::TraceDrain`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceDrainResponse {
    /// Cursor for the next call (highest `seq` issued + 1).
    pub next_seq: u64,
    /// `true` when the ring evicted records the caller hadn't
    /// observed yet — the planner should resync (e.g. by widening
    /// its `after_seq` window or restarting capture).
    pub dropped: bool,
    /// `true` while the underlying VM sink is actively recording.
    pub capturing: bool,
    pub events: Vec<TraceEventPayload>,
}

/// One trace event in transit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEventPayload {
    /// Monotonic sequence assigned by the VM at emission time.
    pub seq: u64,
    pub kind: TraceEventKindPayload,
}

/// Payload-side mirror of the VM `TraceEventKind`. Serde uses an
/// internally-tagged shape (`{"type": "...", ...}`) so JSON readers
/// can dispatch on `type` without external tags getting in the way
/// of typed clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEventKindPayload {
    FnEnter {
        name: String,
        function_index: u32,
        depth: u32,
    },
    FnExit {
        name: String,
        depth: u32,
    },
    Branch {
        fn_name: String,
        pc: u32,
        branch: TraceBranchKind,
        operand: i32,
        offset: i32,
        taken: bool,
    },
    CallSys {
        fn_name: String,
        pc: u32,
        sysfn_index: u32,
        sysfn_name: String,
        sp_before: u32,
        sp_after: u32,
        r1_after: u32,
    },
    GlobalRead {
        fn_name: String,
        pc: u32,
        scope: TraceGlobalScope,
        slot: u32,
        value: u32,
    },
    GlobalWrite {
        fn_name: String,
        pc: u32,
        scope: TraceGlobalScope,
        slot: u32,
        value: u32,
    },
    Suspend {
        fn_name: String,
        pc: u32,
    },
}

/// Wire form of the VM `BranchKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceBranchKind {
    Jz,
    Jnz,
    JsJgez,
    JnsJlz,
    JpJlez,
    JnpJgz,
}

/// Wire form of the VM `GlobalScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceGlobalScope {
    /// Shared "plot-flag" global (the array exposed via
    /// [`AgentCommand::GetScriptGlobals`]).
    Shared,
    /// Module-local global. PAL4 scripts almost never touch these
    /// — observed writes here are an RE signal worth surfacing.
    Module,
}

/// Parameters for [`AgentCommand::ChooseDialog`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DialogChooseParams {
    /// 1-based choice index, matching the legacy
    /// `giSelectDialogGetLastSelect` return convention. Pass `1` to
    /// pick the first item explicitly (also the implicit default).
    pub index: i32,
}

/// Parameters for [`AgentCommand::ChooseWorldMap`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMapChooseParams {
    /// Destination scene name (case-sensitive, e.g. `"M02"`,
    /// `"Q01"`). Mirrors the first argument to `giArenaLoad`.
    pub scene: String,
    /// Destination block name (e.g. `"1"`, `"N01"`). Mirrors the
    /// second argument to `giArenaLoad`.
    pub block: String,
}

/// Snapshot of `radiance::perf` registry, returned by
/// [`AgentCommand::GetPerfMetrics`].
///
/// `enabled = false` ⇒ the engine was launched without
/// `OPENPAL3_PERF=1` (or equivalent). All recorded `metrics` will
/// be empty in that case so callers can detect "instrumentation
/// inactive" vs "no metrics recorded yet".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfMetricsResponse {
    /// `true` if `radiance::perf::enabled()` is `true` on the game
    /// thread (i.e. the `OPENPAL3_PERF` env var was set at boot).
    pub enabled: bool,
    /// One entry per metric name currently tracked. Sorted
    /// alphabetically by name to make diffs trivial for callers.
    pub metrics: Vec<PerfMetric>,
}

/// One metric in [`PerfMetricsResponse::metrics`]. Discriminated by
/// `kind` so a single JSON envelope can carry timings, counters, and
/// gauges without a wrapper-per-kind explosion on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PerfMetric {
    /// `radiance::perf::time()` / `radiance::perf::timer()` output.
    /// `avg_ns` is `total_ns / calls`; `max_ns` is the worst single
    /// call observed since boot.
    Timing {
        name: String,
        calls: u64,
        avg_ns: u64,
        max_ns: u64,
    },
    /// `radiance::perf::count()` output. `frame` is the count
    /// accumulated since the last `perf::flush_frame()` (resets
    /// every `OPENPAL3_PERF_INTERVAL` frames when configured),
    /// `total` is the lifetime sum.
    Counter {
        name: String,
        frame: u64,
        total: u64,
    },
    /// `radiance::perf::gauge()` output. `last` is the most recent
    /// recorded value; `max` is the highest value seen since boot.
    Gauge { name: String, last: u64, max: u64 },
}
