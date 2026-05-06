use crosscom::ComRc;
use radiance::comdef::IDirector;

use crate::ui_walker::LocalCommandQueue;

pub trait CommandRouter {
    /// Called for each command id enqueued by buttons in the rendered UI tree.
    /// Return Some(next_director) to short-circuit the rest of the queue and
    /// transition to that director on the next frame; return None to let the
    /// proxy keep draining commands and (if all return None) fall back to the
    /// script's `update` fn for routing.
    fn dispatch(&self, command_id: i32) -> Option<ComRc<IDirector>>;
}

/// Default router that ignores every command.
pub struct NullCommandRouter;

impl CommandRouter for NullCommandRouter {
    fn dispatch(&self, _command_id: i32) -> Option<ComRc<IDirector>> {
        None
    }
}

pub fn dispatch_commands(
    queue: &mut LocalCommandQueue,
    router: &dyn CommandRouter,
) -> Option<ComRc<IDirector>> {
    while let Some(command_id) = queue.queue.pop_front() {
        if let Some(next) = router.dispatch(command_id) {
            return Some(next);
        }
    }
    None
}
