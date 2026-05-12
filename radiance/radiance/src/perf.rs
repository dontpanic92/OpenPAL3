use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Disabled,
    Debug,
    Trace,
}

#[derive(Debug)]
struct Config {
    mode: Mode,
    interval_frames: u64,
}

#[derive(Debug)]
pub struct TimerGuard {
    name: &'static str,
    start: Option<Instant>,
}

#[derive(Debug)]
enum MetricStats {
    Timing(TimingStats),
    Counter(CounterStats),
    Gauge(GaugeStats),
}

#[derive(Debug)]
struct TimingStats {
    calls: u64,
    total_ns: u128,
    min_ns: u64,
    max_ns: u64,
}

#[derive(Default, Debug)]
struct CounterStats {
    total: u64,
    frame: u64,
}

#[derive(Default, Debug)]
struct GaugeStats {
    last: u64,
    max: u64,
}

#[derive(Default, Debug)]
struct Registry {
    frame: u64,
    metrics: HashMap<&'static str, MetricStats>,
}

impl Default for TimingStats {
    fn default() -> Self {
        Self {
            calls: 0,
            total_ns: 0,
            min_ns: u64::MAX,
            max_ns: 0,
        }
    }
}

impl TimingStats {
    fn record(&mut self, duration: Duration) {
        let ns = duration.as_nanos();
        let ns_u64 = ns.min(u64::MAX as u128) as u64;
        self.calls = self.calls.saturating_add(1);
        self.total_ns = self.total_ns.saturating_add(ns);
        self.min_ns = self.min_ns.min(ns_u64);
        self.max_ns = self.max_ns.max(ns_u64);
    }
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn enabled() -> bool {
    config().mode != Mode::Disabled
}

pub fn timer(name: &'static str) -> TimerGuard {
    TimerGuard {
        name,
        start: enabled().then(Instant::now),
    }
}

pub fn time<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
    let _timer = timer(name);
    f()
}

pub fn count(name: &'static str, amount: u64) {
    if enabled() {
        record_count(name, amount);
    }
}

pub fn gauge(name: &'static str, value: u64) {
    if enabled() {
        record_gauge(name, value);
    }
}

pub fn flush_frame() {
    let config = config();
    if config.mode == Mode::Disabled {
        return;
    }

    let mut line = String::new();
    let should_log = REGISTRY.with(|registry| {
        let Ok(mut registry) = registry.try_borrow_mut() else {
            return false;
        };
        registry.frame = registry.frame.saturating_add(1);
        if registry.frame % config.interval_frames != 0 {
            return false;
        }

        line = format!("perf frame={}", registry.frame);
        let mut names = registry.metrics.keys().copied().collect::<Vec<_>>();
        names.sort_unstable();
        for name in names {
            if let Some(metric) = registry.metrics.get(name) {
                line.push(' ');
                append_metric(&mut line, name, metric);
            }
        }

        for metric in registry.metrics.values_mut() {
            if let MetricStats::Counter(stats) = metric {
                stats.frame = 0;
            }
        }
        true
    });

    if should_log {
        match config.mode {
            Mode::Trace | Mode::Debug => log::debug!("{}", line),
            Mode::Disabled => {}
        }
    }
}

impl Drop for TimerGuard {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            record_timing(self.name, start.elapsed());
        }
    }
}

fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let mode = match std::env::var("OPENPAL3_PERF") {
            Ok(value) if is_truthy(&value) => mode_from_level_env(Mode::Debug),
            Ok(value) if value.eq_ignore_ascii_case("trace") => Mode::Trace,
            Ok(value) if value.eq_ignore_ascii_case("debug") => Mode::Debug,
            _ => Mode::Disabled,
        };
        let interval_frames = std::env::var("OPENPAL3_PERF_INTERVAL")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(match mode {
                Mode::Trace => 1,
                Mode::Debug => 120,
                Mode::Disabled => 120,
            });

        Config {
            mode,
            interval_frames,
        }
    })
}

fn mode_from_level_env(default: Mode) -> Mode {
    match std::env::var("OPENPAL3_PERF_LEVEL") {
        Ok(value) if value.eq_ignore_ascii_case("trace") => Mode::Trace,
        Ok(value) if value.eq_ignore_ascii_case("debug") => Mode::Debug,
        _ => default,
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn record_timing(name: &'static str, duration: Duration) {
    REGISTRY.with(|registry| {
        let Ok(mut registry) = registry.try_borrow_mut() else {
            return;
        };
        match registry.metrics.entry(name) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if let MetricStats::Timing(stats) = entry.get_mut() {
                    stats.record(duration);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let mut stats = TimingStats::default();
                stats.record(duration);
                entry.insert(MetricStats::Timing(stats));
            }
        }
    });
}

fn record_count(name: &'static str, amount: u64) {
    REGISTRY.with(|registry| {
        let Ok(mut registry) = registry.try_borrow_mut() else {
            return;
        };
        match registry.metrics.entry(name) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if let MetricStats::Counter(stats) = entry.get_mut() {
                    stats.total = stats.total.saturating_add(amount);
                    stats.frame = stats.frame.saturating_add(amount);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(MetricStats::Counter(CounterStats {
                    total: amount,
                    frame: amount,
                }));
            }
        }
    });
}

fn record_gauge(name: &'static str, value: u64) {
    REGISTRY.with(|registry| {
        let Ok(mut registry) = registry.try_borrow_mut() else {
            return;
        };
        match registry.metrics.entry(name) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if let MetricStats::Gauge(stats) = entry.get_mut() {
                    stats.last = value;
                    stats.max = stats.max.max(value);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(MetricStats::Gauge(GaugeStats {
                    last: value,
                    max: value,
                }));
            }
        }
    });
}

fn append_metric(line: &mut String, name: &str, metric: &MetricStats) {
    match metric {
        MetricStats::Timing(stats) => {
            let avg_ns = if stats.calls == 0 {
                0
            } else {
                (stats.total_ns / stats.calls as u128).min(u64::MAX as u128) as u64
            };
            line.push_str(&format!(
                "{} calls={} avg={} max={}",
                name,
                stats.calls,
                format_duration_ns(avg_ns),
                format_duration_ns(stats.max_ns)
            ));
        }
        MetricStats::Counter(stats) => {
            line.push_str(&format!(
                "{} frame={} total={}",
                name, stats.frame, stats.total
            ));
        }
        MetricStats::Gauge(stats) => {
            line.push_str(&format!("{} last={} max={}", name, stats.last, stats.max));
        }
    }
}

fn format_duration_ns(ns: u64) -> String {
    if ns >= 1_000_000 {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    } else if ns >= 1_000 {
        format!("{:.2}us", ns as f64 / 1_000.0)
    } else {
        format!("{}ns", ns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_registry() {
        REGISTRY.with(|registry| *registry.borrow_mut() = Registry::default());
    }

    #[test]
    fn disabled_api_does_not_panic() {
        let _timer = timer("perf.test.disabled_timer");
        count("perf.test.disabled_counter", 1);
        gauge("perf.test.disabled_gauge", 2);
        flush_frame();
    }

    #[test]
    fn counters_aggregate() {
        clear_registry();

        record_count("perf.test.counter", 2);
        record_count("perf.test.counter", 3);

        REGISTRY.with(|registry| {
            match registry
                .borrow()
                .metrics
                .get("perf.test.counter")
                .expect("counter metric should exist")
            {
                MetricStats::Counter(stats) => {
                    assert_eq!(stats.total, 5);
                    assert_eq!(stats.frame, 5);
                }
                other => panic!("unexpected metric kind: {:?}", other),
            }
        });
    }

    #[test]
    fn gauges_keep_last_and_max() {
        clear_registry();

        record_gauge("perf.test.gauge", 4);
        record_gauge("perf.test.gauge", 2);

        REGISTRY.with(|registry| {
            match registry
                .borrow()
                .metrics
                .get("perf.test.gauge")
                .expect("gauge metric should exist")
            {
                MetricStats::Gauge(stats) => {
                    assert_eq!(stats.last, 2);
                    assert_eq!(stats.max, 4);
                }
                other => panic!("unexpected metric kind: {:?}", other),
            }
        });
    }

    #[test]
    fn timer_guard_records_on_drop() {
        clear_registry();

        let guard = TimerGuard {
            name: "perf.test.timer",
            start: Some(Instant::now()),
        };
        drop(guard);

        REGISTRY.with(|registry| {
            match registry
                .borrow()
                .metrics
                .get("perf.test.timer")
                .expect("timer metric should exist")
            {
                MetricStats::Timing(stats) => assert_eq!(stats.calls, 1),
                other => panic!("unexpected metric kind: {:?}", other),
            }
        });
    }

    #[test]
    fn formats_duration_units() {
        assert_eq!(format_duration_ns(999), "999ns");
        assert_eq!(format_duration_ns(1_500), "1.50us");
        assert_eq!(format_duration_ns(1_500_000), "1.50ms");
    }
}
