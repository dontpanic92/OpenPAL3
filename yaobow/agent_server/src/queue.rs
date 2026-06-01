//! Cross-thread command queue used by the HTTP transport.
//!
//! The HTTP worker thread converts a request into an
//! [`AgentCommand`](crate::AgentCommand) and pushes an [`AgentEnvelope`]
//! into the [`AgentCommandQueue`]. It then blocks on the envelope's
//! single-use reply channel until the game thread drains the queue,
//! dispatches each command into the [`AgentSession`](crate::AgentSession),
//! and sends back the [`AgentResponse`](crate::AgentResponse).
//!
//! ## Split ownership
//!
//! `std::sync::mpsc::Receiver` is `Send` but not `Sync`, so the queue
//! cannot live inside an `Arc<...>` shared with the HTTP thread. The
//! producer (`AgentCommandQueue`) only holds the `Sender` and is
//! cheap to clone (just clones the inner mpsc handle). The consumer
//! side is exposed as a separate [`AgentCommandConsumer`] that the
//! game thread owns and drains.
//!
//! `AgentCommandQueue::new()` returns the matched pair.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::Duration;

use crate::protocol::{AgentCommand, AgentResponse};

/// One queued command plus its reply slot.
pub struct AgentEnvelope {
    /// The command to dispatch. Game-thread dispatchers `clone()` it
    /// out of the envelope before consuming `self` via [`Self::reply`].
    pub command: AgentCommand,
    reply: Sender<AgentResponse>,
}

impl AgentEnvelope {
    /// Build an envelope and the matching one-shot receiver. The HTTP
    /// worker keeps the receiver and `recv_timeout`s on it while the
    /// game-thread side calls [`Self::reply`].
    pub fn new(command: AgentCommand) -> (Self, Receiver<AgentResponse>) {
        let (tx, rx) = mpsc::channel();
        (Self { command, reply: tx }, rx)
    }

    /// Send the response back to the waiting HTTP worker. Logs (and
    /// drops) the response if the worker has already gone away (e.g.
    /// it timed out waiting). Safe to call exactly once; subsequent
    /// calls are no-ops because `Sender::send` consumes `self`.
    pub fn reply(self, response: AgentResponse) {
        if let Err(err) = self.reply.send(response) {
            log::debug!(
                "agent_server: reply receiver gone before response delivered: {:?}",
                err.0
            );
        }
    }
}

/// Producer half of the command queue. Cheap to clone — the inner
/// `Sender` is itself reference-counted.
#[derive(Clone)]
pub struct AgentCommandQueue {
    tx: Sender<AgentEnvelope>,
}

impl AgentCommandQueue {
    /// Build a fresh producer / consumer pair. The producer is given
    /// to the HTTP listener (and any other thread that needs to push
    /// commands); the consumer is owned by the game thread.
    pub fn new() -> (Self, AgentCommandConsumer) {
        let (tx, rx) = mpsc::channel();
        (Self { tx }, AgentCommandConsumer { rx })
    }

    /// Push an envelope onto the queue. Returns `Err(env)` (so the
    /// caller can reply with a transport error) when the consumer
    /// has been dropped — e.g. when the game thread is shutting down.
    pub fn push(&self, env: AgentEnvelope) -> Result<(), AgentEnvelope> {
        self.tx.send(env).map_err(|e| e.0)
    }

    /// Raw `Sender` clone — useful for places that already deal with
    /// the lower-level type. Prefer [`Self::push`] in new code.
    pub fn sender(&self) -> Sender<AgentEnvelope> {
        self.tx.clone()
    }
}

/// Consumer half of the command queue. Lives on the game thread and
/// is `!Sync` (mirroring the underlying `Receiver`).
pub struct AgentCommandConsumer {
    rx: Receiver<AgentEnvelope>,
}

impl AgentCommandConsumer {
    /// Non-blocking drain. Calls `f` on every pending envelope. The
    /// game thread invokes this once per frame.
    pub fn drain<F: FnMut(AgentEnvelope)>(&self, mut f: F) {
        loop {
            match self.rx.try_recv() {
                Ok(env) => f(env),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return,
            }
        }
    }

    /// Blocking drain with a timeout, used by integration tests that
    /// want to wait for at least one envelope without burning CPU.
    pub fn drain_with_timeout<F: FnMut(AgentEnvelope)>(
        &self,
        timeout: Duration,
        mut f: F,
    ) -> Result<(), RecvTimeoutError> {
        let first = self.rx.recv_timeout(timeout)?;
        f(first);
        self.drain(&mut f);
        Ok(())
    }
}
