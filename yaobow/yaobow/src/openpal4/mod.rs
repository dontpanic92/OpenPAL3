use self::scripting::create_script_vm;

pub mod application;
pub mod scripting;

pub fn run_openpal4() {
    /*let mut app = OpenPal4Application::create("OpenPAL4");
    app.initialize();
    app.run();*/

    // let mut line = String::new();
    // let stdin = std::io::stdin();
    // stdin.lock().read_line(&mut line).unwrap();

    /*let data = std::fs::read("F:\\PAL4\\gamedata\\PALActor\\101\\C01.anm").unwrap();
    let chunks = read_anm(&data).unwrap();
    println!("{}", serde_json::to_string(&chunks).unwrap());

    let anm = load_anm_action(&chunks[0]);
    println!("{:?}", anm);*/

    let mut vm = create_script_vm();
    vm.execute();
}
