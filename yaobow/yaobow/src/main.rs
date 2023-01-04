use std::{cell::RefCell, io::BufRead, rc::Rc};

use openpal4::{
    application::OpenPal4Application,
    scripting::{global_context::ScriptGlobalContext, module::ScriptModule, vm::ScriptVm},
};

mod openpal4;

pub fn main() {
    /*let mut app = OpenPal4Application::create("OpenPAL4");
    app.initialize();
    app.run();*/

    let mut line = String::new();
    let stdin = std::io::stdin();
    stdin.lock().read_line(&mut line).unwrap();

    let content = std::fs::read("F:\\PAL4\\gamedata\\script\\Q01.csb").unwrap();

    let module = ScriptModule::load_from_buffer(&content).unwrap();
    // println!("{}", serde_json::to_string(&module).unwrap());

    let context = Rc::new(RefCell::new(ScriptGlobalContext::new()));
    let mut vm = ScriptVm::new(context);
    let module = Rc::new(RefCell::new(module));
    vm.set_module(module);
    vm.set_function(0);
    vm.execute();
}
