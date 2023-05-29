use std::io::{BufRead, Cursor};

use fileformats::{
    rwbs::list_chunks,
    rwbs::{read_anm, read_bsp, read_dff},
};
use shared::{
    fs::pkg::pkg_archive::PkgHeader,
    loaders::anm::{load_anm, load_anm_action},
};

pub mod application;
pub mod scripting;

pub fn run_openpal4() {
    /*let mut app = OpenPal4Application::create("OpenPAL4");
    app.initialize();
    app.run();*/

    // let mut line = String::new();
    // let stdin = std::io::stdin();
    // stdin.lock().read_line(&mut line).unwrap();

    let data = std::fs::read("F:\\PAL4\\gamedata\\PALActor\\101\\C01.anm").unwrap();
    let chunks = read_anm(&data).unwrap();
    println!("{}", serde_json::to_string(&chunks).unwrap());

    let anm = load_anm_action(&chunks[0]);
    println!("{:?}", anm);

    /*let content = std::fs::read("F:\\PAL4\\gamedata\\script\\script.csb").unwrap();

    let module = ScriptModule::load_from_buffer(&content).unwrap();
    println!("{}", serde_json::to_string(&module).unwrap());

    let context = Rc::new(RefCell::new(ScriptGlobalContext::new()));
    let mut vm = ScriptVm::new(context);
    let module = Rc::new(RefCell::new(module));
    vm.set_module(module);
    vm.set_function(0);
    vm.execute();*/
}
