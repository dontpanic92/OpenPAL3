#![allow(unused_variables)]
#![allow(dead_code)]

use application::run_title_selection;
use opengujian::run_opengujian;
use openpal3::run_openpal3;
use openpal4::run_openpal4;
use openpal5::run_openpal5;
use openswd5::run_openswd5;
use playground::run_test;
use shared::video::register_opengb_video_decoders;

mod application;
mod comdef;
mod opengujian;
mod openpal3;
mod openpal4;
mod openpal5;
mod openswd5;
mod playground;

pub fn main() {
    radiance::application::Application::set_panic_hook();
    init_logger();
    register_opengb_video_decoders();

    #[cfg(vita)]
    {
        run_openpal4();
    }

    #[cfg(not(vita))]
    {
        let args = std::env::args().collect::<Vec<String>>();
        if args.len() <= 1 {
            run_title_selection();
        } else {
            match args[1].as_str() {
                "--pal3" => {
                    run_openpal3();
                }
                "--pal4" => {
                    run_openpal4();
                }
                "--pal5" => {
                    run_openpal5();
                }
                "--pal5q" => {
                    run_openpal5();
                }
                "--swd5" => {
                    run_openswd5();
                }
                "--gujian" => {
                    run_opengujian();
                }
                "--test" => {
                    run_test();
                }
                &_ => {}
            }
        }
    }
}

fn init_logger() {
    #[cfg(any(windows, linux, macos, android))]
    {
        let logger = simple_logger::SimpleLogger::new();
        // workaround panic on Linux for 'Could not determine the UTC offset on this system'
        // see: https://github.com/borntyping/rust-simple_logger/issues/47
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
        let logger = logger.with_utc_timestamps();
        logger.init().unwrap();
    }

    #[cfg(vita)]
    {
        let logger = simplelog::WriteLogger::new(
            simplelog::LevelFilter::Error,
            simplelog::Config::default(),
            std::fs::File::create("ux0:data/yaobow.log").unwrap(),
        );

        simplelog::CombinedLogger::init(vec![logger]).unwrap();
    }
}

#[used]
#[export_name = "_newlib_heap_size_user"]
pub static _NEWLIB_HEAP_SIZE_USER: u32 = 216 * 1024 * 1024;
