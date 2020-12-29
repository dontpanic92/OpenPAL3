use std::io::{Cursor, Read};

use opengb::{application::OpenGbApplication, config::OpenGbConfig};

fn print_dir(entry: &opengb::cpk::CpkEntry, ident_level: usize) {
    let ident = "  ".repeat(ident_level);
    for child in entry.children() {
        println!("{}{}", ident, child.borrow().name());
        print_dir(&child.borrow(), ident_level + 1);
    }
}

fn main() {
    let config = OpenGbConfig::load("openpal3", "OPENPAL3");
    let mut app = OpenGbApplication::create(&config, "OpenPAL3");
    app.initialize();
    app.run();
}
