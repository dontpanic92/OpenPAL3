use std::io::{BufRead, Cursor};

use shared::fs::pkg::pkg_archive::PkgHeader;

pub mod application;
pub mod scripting;

pub fn run_openpal4() {
    /*let mut app = OpenPal4Application::create("OpenPAL4");
    app.initialize();
    app.run();*/

    let mut line = String::new();
    let stdin = std::io::stdin();
    stdin.lock().read_line(&mut line).unwrap();

    let data = std::fs::read("F:\\Pal5Q\\Music.pkg").unwrap();
    let mut cursor = Cursor::new(data);
    let header = PkgHeader::read(&mut cursor).unwrap();
    cursor.set_position(header.entries_start as u64);
    println!("start: {}", header.entries_start);

    //let entries = PkgEntries::read(&mut cursor, "L#Z^zyjq" /*"Y%H^uz6i"*/).unwrap();
    //println!("{}", serde_json::to_string(&entries).unwrap());

    /*let data = std::fs::read("F:\\PAL4\\gamedata\\PALActor\\101\\101.dff").unwrap();
    let chunks = read_dff(&data).unwrap();
    println!("{}", serde_json::to_string(&chunks).unwrap());*/

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
