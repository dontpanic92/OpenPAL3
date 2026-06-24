//! [`AgentSession`] trait + a `NullAgentSession` test stub.
//!
//! Concrete sessions live in their per-game crates (PAL4 in
//! `shared::openpal4::agent`) and call back into the game's
//! `AppContext` / `Director` to execute commands. The session runs on
//! the game thread; the transport thread never touches game state
//! directly.

use crate::protocol::{AgentCommand, AgentResponse, DialogSnapshot, StateSnapshot};

/// Per-game agent adapter. Implementations are typically not `Send`
/// (they hold references to engine objects), which is fine: the
/// command queue is consumed on the game thread that owns them.
pub trait AgentSession {
    /// Execute one command. Return value is wired straight back to the
    /// HTTP client. Implementations should never panic on bad input —
    /// surface those as [`AgentResponse::Error`].
    fn execute(&mut self, command: AgentCommand) -> AgentResponse;

    /// Build a fresh snapshot. Called either by the dispatch path for
    /// `GetState` or by hosts that want to publish a periodic state
    /// dump to the log sink.
    fn snapshot(&self) -> StateSnapshot;
}

/// Trivial session used by tests and as a placeholder before the
/// per-game adapter is wired. Every command returns `Ok` (or an empty
/// snapshot for `GetState`); good enough to exercise the transport
/// layer end-to-end.
///
/// Tests that need to exercise the screenshot path can call
/// [`Self::set_screenshot`] to inject a fixed RGBA frame that
/// [`AgentCommand::Screenshot`] then returns.
pub struct NullAgentSession {
    /// Monotonic frame counter for the snapshot.
    frame: u64,
    /// Optional canned screenshot — when set, served from
    /// [`AgentCommand::Screenshot`]. When `None`, the screenshot
    /// command returns an empty `ScreenshotResponse` (so the
    /// transport surfaces a 501 to the client).
    canned_screenshot: Option<(u32, u32, Vec<u8>)>,
}

impl NullAgentSession {
    pub fn new() -> Self {
        Self {
            frame: 0,
            canned_screenshot: None,
        }
    }

    /// Stash a canned RGBA frame to be returned from the next
    /// `Screenshot` command. Sized `width*height*4` bytes; callers
    /// are responsible for matching that or the transport will reject
    /// the payload with a 500.
    pub fn set_screenshot(&mut self, width: u32, height: u32, rgba: Vec<u8>) {
        self.canned_screenshot = Some((width, height, rgba));
    }
}

impl Default for NullAgentSession {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSession for NullAgentSession {
    fn execute(&mut self, command: AgentCommand) -> AgentResponse {
        self.frame = self.frame.saturating_add(1);
        match command {
            AgentCommand::GetState => AgentResponse::State(self.snapshot()),
            AgentCommand::LogTail(_) => AgentResponse::Log(crate::protocol::LogTailResponse {
                next_seq: 0,
                dropped: false,
                records: Vec::new(),
            }),
            AgentCommand::Screenshot => match self.canned_screenshot.clone() {
                Some((width, height, rgba)) => {
                    AgentResponse::Screenshot(crate::protocol::ScreenshotResponse {
                        width,
                        height,
                        encoded: true,
                        rgba,
                    })
                }
                None => AgentResponse::Screenshot(crate::protocol::ScreenshotResponse::default()),
            },
            AgentCommand::ScriptEval(p) => {
                AgentResponse::Script(crate::protocol::ScriptEvalResponse {
                    function: p.function,
                    result: None,
                })
            }
            _ => AgentResponse::Ok,
        }
    }

    fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            frame: self.frame,
            scene: String::new(),
            block: String::new(),
            leader: 0,
            leader_pos: [0.0; 3],
            party: Vec::new(),
            money: 0,
            quest_percentage: 0,
            dialog: DialogSnapshot::default(),
            fast_forward: false,
            paused: false,
            current_script_fn: None,
            script_running: false,
            movie_playing: false,
            fps: 0.0,
            dt: 0.0,
            inventory: Vec::new(),
            world_map_open: false,
            debug_camera: false,
            camera_eye: [0.0; 3],
            camera_target: [0.0; 3],
        }
    }
}
