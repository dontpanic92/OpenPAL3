use std::cell::RefCell;
use std::collections::VecDeque;

use crosscom::ComRc;

use crate::comdef::services::{ICommandBus, ICommandBusImpl};

pub struct CommandBus {
    queue: RefCell<VecDeque<(i32, i32)>>,
    handler: Option<Box<dyn Fn(i32, i32) -> i32>>,
}

ComObject_CommandBus!(super::CommandBus);

impl CommandBus {
    pub fn create(handler: Option<Box<dyn Fn(i32, i32) -> i32>>) -> ComRc<ICommandBus> {
        ComRc::from_object(Self {
            queue: RefCell::new(VecDeque::new()),
            handler,
        })
    }

    pub fn drain(&self) -> Vec<(i32, i32)> {
        self.queue.borrow_mut().drain(..).collect()
    }
}

impl ICommandBusImpl for CommandBus {
    fn dispatch(&self, cmd_kind: i32, arg: i32) -> i32 {
        self.queue.borrow_mut().push_back((cmd_kind, arg));
        self.handler
            .as_ref()
            .map(|handler| handler(cmd_kind, arg))
            .unwrap_or(0)
    }
}
