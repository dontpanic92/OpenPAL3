//! Thin binary entry point. All app logic lives in `yaobow_lib`.

use shared::video::register_opengb_video_decoders;
use yaobow_lib::{
    run_opengujian, run_openpal3, run_openpal4, run_openpal4_with_agent, run_openpal5,
    run_openswd5, run_title_selection, Pal4AgentBootOptions,
};

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
                "--pal3" => run_openpal3(),
                "--pal4" => {
                    let agent = parse_agent_args(&args[2..]);
                    if agent.is_some() {
                        run_openpal4_with_agent(agent);
                    } else {
                        run_openpal4();
                    }
                }
                "--pal5" => run_openpal5(),
                "--pal5q" => run_openpal5(),
                "--swd5" => run_openswd5(),
                "--gujian" => run_opengujian(),
                "--test" => {}
                &_ => {}
            }
        }
    }
}

/// Parse the `--agent-port`, `--agent-bind`, `--agent-token` flags out
/// of the command-line tail. Returns `None` when no `--agent-port` is
/// present; otherwise an [`Pal4AgentBootOptions`] ready for
/// [`run_openpal4_with_agent`].
fn parse_agent_args(extra: &[String]) -> Option<Pal4AgentBootOptions> {
    let mut port: Option<u16> = None;
    let mut bind: Option<std::net::IpAddr> = None;
    let mut token: Option<String> = None;

    let mut iter = extra.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--agent-port" => {
                port = iter.next().and_then(|s| s.parse().ok());
            }
            "--agent-bind" => {
                bind = iter.next().and_then(|s| s.parse().ok());
            }
            "--agent-token" => {
                token = iter.next().cloned();
            }
            _ => {}
        }
    }

    let port = port?;
    let mut opts = Pal4AgentBootOptions::loopback(port);
    opts.bind_ip = bind;
    opts.token = token;
    Some(opts)
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
