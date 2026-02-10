use std::{cell::RefCell, rc::Rc};

use imgui::Context;

use crate::application::Platform;

pub struct ImguiPlatform {
    context: Rc<RefCell<Context>>,
}

impl ImguiPlatform {
    pub fn new(context: Rc<RefCell<Context>>, platform: &mut Platform) -> Rc<RefCell<Self>> {
        Self::set_display_size(&context);
        Rc::new(RefCell::new(Self { context }))
    }

    pub fn new_frame(&self) {}

    pub fn prepare_render(&self, ui: &imgui::Ui) {}

    fn set_display_size(context: &Rc<RefCell<Context>>) {
        let mut context = context.borrow_mut();
        context.io_mut().display_size = [960., 544.];
    }
}
