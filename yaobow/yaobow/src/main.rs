#![allow(unused_variables)]
#![allow(dead_code)]

use openpal3::run_openpal3;
use openpal4::run_openpal4;

mod comdef;
mod openpal3;
mod openpal4;

pub fn main() {
    init_logger();

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() <= 1 {
        run_openpal3();
    } else {
        match args[1].as_str() {
            "--pal3" => {
                run_openpal3();
            }
            "--pal4" => {
                run_openpal4();
            }
            "--pal5" => {
                run_openpal4();
            }
            "--pal5q" => {
                run_openpal4();
            }
            &_ => {}
        }
    }
}

fn init_logger() {
    let logger = simple_logger::SimpleLogger::new();
    // workaround panic on Linux for 'Could not determine the UTC offset on this system'
    // see: https://github.com/borntyping/rust-simple_logger/issues/47
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
    let logger = logger.with_utc_timestamps();
    logger.init().unwrap();
}
