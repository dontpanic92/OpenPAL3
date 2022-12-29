use std::io::BufRead;

use openpal4::{application::OpenPal4Application, scripting::module::ScriptModule};

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
    println!("{}", serde_json::to_string(&module).unwrap());
}
