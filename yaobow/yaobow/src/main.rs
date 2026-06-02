//! Thin binary entry point. All app logic lives in `yaobow_lib`.

use agent_server::AgentLogSink;
use log::{Level, LevelFilter, Log, Metadata, Record};
use shared::video::register_opengb_video_decoders;
use yaobow_lib::{
    run_opengujian, run_openpal3, run_openpal4, run_openpal4_with_agent, run_openpal5,
    run_openswd5, run_title_selection, Pal4AgentBootOptions,
};

pub fn main() {
    radiance::application::Application::set_panic_hook();

    #[cfg(vita)]
    {
        init_logger(None);
        register_opengb_video_decoders();
        run_openpal4();
    }

    #[cfg(not(vita))]
    {
        let args = std::env::args().collect::<Vec<String>>();
        let agent_opts: Option<Pal4AgentBootOptions> = if args.len() > 2 && args[1] == "--pal4" {
            parse_agent_args(&args[2..])
        } else {
            None
        };

        // Initialise the global logger *after* arg parsing so we can
        // tee into `AgentLogSink` when `--agent-port` is set. Doing it
        // before would race the application loader (which can't
        // re-register a logger) and leave `/v1/log/tail` empty.
        init_logger(agent_opts.is_some().then(|| AgentLogSink::new(4096)));
        register_opengb_video_decoders();

        if args.len() <= 1 {
            run_title_selection();
        } else {
            match args[1].as_str() {
                "--pal3" => run_openpal3(),
                "--pal4" => {
                    if agent_opts.is_some() {
                        run_openpal4_with_agent(agent_opts);
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

/// Parse the `--agent-port`, `--agent-bind`, `--agent-token`,
/// `--agent-reply-timeout-secs` flags out of the command-line tail.
/// Returns `None` when no `--agent-port` is present; otherwise an
/// [`Pal4AgentBootOptions`] ready for [`run_openpal4_with_agent`].
fn parse_agent_args(extra: &[String]) -> Option<Pal4AgentBootOptions> {
    let mut port: Option<u16> = None;
    let mut bind: Option<std::net::IpAddr> = None;
    let mut token: Option<String> = None;
    let mut reply_timeout: Option<std::time::Duration> = None;

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
            "--agent-reply-timeout-secs" => {
                reply_timeout = iter
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    // Cap at 60 s — beyond that something is wrong on
                    // the game side and we want to surface a 500
                    // rather than hold the HTTP connection forever.
                    .map(|secs| std::time::Duration::from_secs(secs.min(60)));
            }
            _ => {}
        }
    }

    let port = port?;
    let mut opts = Pal4AgentBootOptions::loopback(port);
    opts.bind_ip = bind;
    opts.token = token;
    opts.reply_timeout = reply_timeout;
    Some(opts)
}

fn init_logger(agent_sink: Option<AgentLogSink>) {
    #[cfg(any(windows, linux, macos, android))]
    {
        let logger = simple_logger::SimpleLogger::new();
        // workaround panic on Linux for 'Could not determine the UTC offset on this system'
        // see: https://github.com/borntyping/rust-simple_logger/issues/47
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "android"))]
        let logger = logger.with_utc_timestamps();

        if let Some(sink) = agent_sink {
            // Agent server enabled: install a tee that fans every
            // record into `AgentLogSink` (so `/v1/log/tail` works) and
            // also into `SimpleLogger` for the usual stdout output.
            let leaked = sink.leak();
            let tee = TeeLogger {
                agent: leaked,
                console: logger,
            };
            log::set_boxed_logger(Box::new(tee)).expect("install tee logger");
            log::set_max_level(LevelFilter::Trace);
        } else {
            logger.init().unwrap();
        }
    }

    #[cfg(vita)]
    {
        let _ = agent_sink;
        let logger = simplelog::WriteLogger::new(
            simplelog::LevelFilter::Error,
            simplelog::Config::default(),
            std::fs::File::create("ux0:data/yaobow.log").unwrap(),
        );

        simplelog::CombinedLogger::init(vec![logger]).unwrap();
    }
}

/// Two-way fan-out logger used when the agent server is enabled. Each
/// record is mirrored into the ring-buffered `AgentLogSink` (drained
/// by `/v1/log/tail`) **and** into `SimpleLogger` (which formats and
/// prints to stdout). `enabled`/`flush` short-circuit through both
/// backends so existing level filters keep working.
struct TeeLogger {
    agent: &'static AgentLogSink,
    console: simple_logger::SimpleLogger,
}

impl Log for TeeLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.agent.enabled(metadata) || self.console.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        // Always tap into the ring buffer (level filter is enforced
        // inside `AgentLogSink::log`), so the agent transport sees the
        // full firehose even when SimpleLogger has filtered it out.
        self.agent.log(record);
        if self.console.enabled(record.metadata()) {
            self.console.log(record);
        }
    }

    fn flush(&self) {
        self.agent.flush();
        self.console.flush();
    }
}

#[cfg(not(vita))]
#[allow(dead_code)]
fn _force_level_imports() {
    // Silences `unused_imports` when only one of the two cfg branches
    // above references `Level`/`LevelFilter`.
    let _ = Level::Info;
    let _ = LevelFilter::Trace;
}

#[used]
#[export_name = "_newlib_heap_size_user"]
pub static _NEWLIB_HEAP_SIZE_USER: u32 = 216 * 1024 * 1024;
