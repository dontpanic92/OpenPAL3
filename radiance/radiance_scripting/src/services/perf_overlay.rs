//! Env-gated imgui overlay that dumps `radiance::perf` counters once
//! per frame, used to diagnose editor hot paths (notably the VFS tree
//! renderer in `resource_tree.p7`).
//!
//! Enabled by setting `YAOBOW_EDITOR_PERF_OVERLAY=1` *and*
//! `OPENPAL3_PERF=1` (the latter is required because `radiance::perf`
//! itself is a no-op when disabled, so there would be nothing to
//! display).
//!
//! The overlay computes per-frame counter deltas locally: it keeps the
//! previous frame's `total` for each counter and subtracts. This means
//! the underlying perf registry is unchanged — `radiance::perf` keeps
//! its monotonic counters and the periodic `flush_frame` log line if
//! callers wire it up.
//!
//! The overlay also displays its own per-frame frame-time
//! (`editor.frame_us`) so callers can correlate tree cost with whole-
//! frame cost.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use radiance::perf::{self, MetricSnapshot};

const ENV_FLAG: &str = "YAOBOW_EDITOR_PERF_OVERLAY";

pub struct PerfOverlay {
    enabled: bool,
    prev_counter_totals: RefCell<HashMap<&'static str, u64>>,
    last_frame_start: RefCell<Option<Instant>>,
    last_copy: RefCell<Option<Instant>>,
}

impl Default for PerfOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfOverlay {
    pub fn new() -> Self {
        let enabled = std::env::var(ENV_FLAG)
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        Self {
            enabled,
            prev_counter_totals: RefCell::new(HashMap::new()),
            last_frame_start: RefCell::new(None),
            last_copy: RefCell::new(None),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Render the overlay window. Must be called inside an active
    /// imgui frame. No-op when disabled, so callers can unconditionally
    /// invoke it.
    pub fn render(&self, ui: &imgui::Ui) {
        if !self.enabled {
            return;
        }

        // Record whole-frame wall time (the interval between
        // successive `render` calls). Crude but good enough for
        // diagnosing whether a counter scales with frame time.
        let now = Instant::now();
        let mut last = self.last_frame_start.borrow_mut();
        let frame_us = last
            .replace(now)
            .map(|prev| now.saturating_duration_since(prev).as_micros() as u64);

        let snapshot = perf::snapshot();
        let mut prev = self.prev_counter_totals.borrow_mut();

        // Build the full text once into a buffer. This is what both
        // the on-screen lines and the Copy button push to the
        // clipboard, so what the user sees is exactly what they get.
        let mut buf = String::with_capacity(64 * snapshot.len().max(1) + 64);
        buf.push_str("perf (yaobow_editor)\n");
        if let Some(us) = frame_us {
            buf.push_str(&format!("frame: {} us\n", us));
        }
        if snapshot.is_empty() {
            buf.push_str("(no metrics - is OPENPAL3_PERF=1 set?)\n");
        } else {
            for (name, metric) in &snapshot {
                match metric {
                    MetricSnapshot::Counter { total, .. } => {
                        let delta = total.saturating_sub(*prev.get(name).unwrap_or(&0));
                        prev.insert(name, *total);
                        if name.ends_with("_ns") {
                            buf.push_str(&format!(
                                "{}: {} (lifetime {})\n",
                                name,
                                format_ns(delta),
                                format_ns(*total)
                            ));
                        } else {
                            buf.push_str(&format!(
                                "{}: this_frame={} total={}\n",
                                name, delta, total
                            ));
                        }
                    }
                    MetricSnapshot::Timing {
                        calls,
                        avg_ns,
                        max_ns,
                    } => {
                        buf.push_str(&format!(
                            "{}: calls={} avg={} max={}\n",
                            name,
                            calls,
                            format_ns(*avg_ns),
                            format_ns(*max_ns)
                        ));
                    }
                    MetricSnapshot::Gauge { last, max } => {
                        buf.push_str(&format!("{}: last={} max={}\n", name, last, max));
                    }
                }
            }
        }

        // Render the overlay in a fixed-position transparent window
        // anchored to the top-right corner so it never overlaps the
        // resource tree pane.
        let display_size = ui.io().display_size;
        let win_w = 560.0_f32;
        let pos = [(display_size[0] - win_w - 8.0).max(8.0), 8.0];
        let last_copy = &self.last_copy;
        ui.window("##perf_overlay")
            .position(pos, imgui::Condition::Always)
            .size([win_w, 0.0], imgui::Condition::Always)
            .size_constraints([win_w, 0.0], [win_w, f32::MAX])
            .flags(
                imgui::WindowFlags::NO_DECORATION
                    | imgui::WindowFlags::NO_MOVE
                    | imgui::WindowFlags::NO_SAVED_SETTINGS
                    | imgui::WindowFlags::NO_FOCUS_ON_APPEARING
                    | imgui::WindowFlags::NO_NAV,
            )
            .bg_alpha(0.55)
            .build(|| {
                if ui.button("Copy") {
                    ui.set_clipboard_text(&buf);
                    *last_copy.borrow_mut() = Some(Instant::now());
                }
                ui.same_line();
                let copied_recently = last_copy
                    .borrow()
                    .map(|t| t.elapsed().as_secs_f32() < 1.5)
                    .unwrap_or(false);
                if copied_recently {
                    ui.text_colored([0.4, 1.0, 0.4, 1.0], "copied!");
                } else {
                    ui.text_disabled("(copies all lines below)");
                }
                ui.separator();
                // Stream the prebuilt buffer line-by-line so what's
                // on screen matches what got copied byte-for-byte.
                for line in buf.lines().skip(1) {
                    ui.text(line);
                }
            });
    }
}

fn format_ns(ns: u64) -> String {
    if ns >= 1_000_000 {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    } else if ns >= 1_000 {
        format!("{:.2}us", ns as f64 / 1_000.0)
    } else {
        format!("{}ns", ns)
    }
}
