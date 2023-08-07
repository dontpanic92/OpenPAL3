use std::{cell::RefCell, rc::Rc};

pub struct TaskManager {
    tasks: RefCell<Vec<Rc<RefCell<dyn Task>>>>,
    new_tasks: RefCell<Vec<Rc<RefCell<dyn Task>>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: RefCell::new(Vec::new()),
            new_tasks: RefCell::new(Vec::new()),
        }
    }

    pub fn run(&self, task: impl Task + 'static) -> Rc<TaskHandle> {
        let task = Rc::new(RefCell::new(task));
        self.new_tasks.borrow_mut().push(task.clone());

        Rc::new(TaskHandle::from_task(task))
    }

    pub fn run_generic(&self, update_func: impl FnMut(f32) -> bool + 'static) -> Rc<TaskHandle> {
        let task = GenericTask {
            update_func: Box::new(update_func),
            is_finished: false,
        };

        self.run(task)
    }

    pub fn update(&self, delta_sec: f32) {
        let mut tasks = self.tasks.borrow_mut().clone();
        tasks.append(&mut self.new_tasks.borrow_mut());
        self.new_tasks.borrow_mut().clear();

        tasks.retain(|task| !task.borrow().is_finished());

        for task in tasks.iter() {
            task.borrow_mut().update(delta_sec);
        }

        self.tasks.replace(tasks);
    }
}

pub trait Task {
    fn update(&mut self, delta_sec: f32);
    fn is_finished(&self) -> bool;
    fn stop(&mut self);
}

pub struct TaskHandle {
    task: Rc<RefCell<dyn Task>>,
}

impl TaskHandle {
    pub fn from_task(task: Rc<RefCell<dyn Task>>) -> Self {
        Self { task }
    }

    pub fn is_finished(&self) -> bool {
        self.task.borrow().is_finished()
    }

    pub fn stop(&self) {
        self.task.borrow_mut().stop();
    }
}

struct GenericTask {
    update_func: Box<dyn FnMut(f32) -> bool>,
    is_finished: bool,
}

impl Task for GenericTask {
    fn update(&mut self, delta_sec: f32) {
        if !self.is_finished {
            self.is_finished = (self.update_func)(delta_sec);
        }
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn stop(&mut self) {
        self.is_finished = true;
    }
}
