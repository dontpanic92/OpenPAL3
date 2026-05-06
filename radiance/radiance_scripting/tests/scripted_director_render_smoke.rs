use std::cell::RefCell;

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance_scripting::command_router::dispatch_commands;
use radiance_scripting::{CommandRouter, LocalCommandQueue, NullCommandRouter};

struct DummyDirector;

radiance_scripting::ComObject_ScriptedDirector!(crate::DummyDirector);

impl IDirectorImpl for DummyDirector {
    fn activate(&self) {}

    fn update(&self, _delta_sec: f32) -> Option<ComRc<IDirector>> {
        None
    }
}

struct StubRouter {
    target_command: i32,
    next: ComRc<IDirector>,
    seen: RefCell<Vec<i32>>,
}

impl CommandRouter for StubRouter {
    fn dispatch(&self, command_id: i32) -> Option<ComRc<IDirector>> {
        self.seen.borrow_mut().push(command_id);
        (command_id == self.target_command).then(|| self.next.clone())
    }
}

#[test]
fn dispatch_commands_short_circuits_on_router_director() {
    let next = ComRc::<IDirector>::from_object(DummyDirector);
    let router = StubRouter {
        target_command: 7,
        next: next.clone(),
        seen: RefCell::new(Vec::new()),
    };
    let mut queue = LocalCommandQueue::default();
    queue.queue.push_back(3);
    queue.queue.push_back(7);
    queue.queue.push_back(9);

    let routed = dispatch_commands(&mut queue, &router).expect("router returns next director");

    assert_eq!(routed.ptr_value(), next.ptr_value());
    assert_eq!(*router.seen.borrow(), vec![3, 7]);
    assert_eq!(queue.queue.into_iter().collect::<Vec<_>>(), vec![9]);
}

#[test]
fn null_router_drains_all_commands_without_routing() {
    let mut queue = LocalCommandQueue::default();
    queue.queue.push_back(7);
    queue.queue.push_back(9);

    assert!(dispatch_commands(&mut queue, &NullCommandRouter).is_none());
    assert!(queue.queue.is_empty());
}
