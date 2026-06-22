//! Game-agnostic handlers for the *generic* agent-server command
//! subset (input, time/pause/step, screenshot, perf metrics).
//!
//! These operate purely on the shared [`AgentBridge`] and have no
//! per-game state, so every adapter (`openswd5::agent`,
//! `openpal5::agent`, …) can delegate the mode-agnostic arms here
//! instead of re-implementing them. PAL3/PAL4 predate this module and
//! keep their own inline copies; new games should prefer these.

use std::rc::Rc;

use agent_server::protocol::{
    AgentError, AgentResponse, AxisInputParams, KeyAction, KeyInputParams, ScreenshotResponse,
    StepTimeParams,
};
use radiance::input::{Axis, Key};

use crate::agent_common::AgentBridge;

/// `/v1/input/key` — inject a synthetic key down/up/tap.
pub fn handle_key_input(bridge: &Rc<AgentBridge>, params: KeyInputParams) -> AgentResponse {
    let Some(key) = Key::from_name(&params.key) else {
        return AgentResponse::err(AgentError::bad_request(format!(
            "unknown key name: {}",
            params.key
        )));
    };
    let synthetic = bridge.input_bridge.borrow();
    match params.action {
        KeyAction::Down => synthetic.press_down(key),
        KeyAction::Up => synthetic.release(key),
        KeyAction::Tap => synthetic.tap(key),
    }
    AgentResponse::Ok
}

/// `/v1/input/axis` — set a synthetic analog-axis value.
pub fn handle_axis_input(bridge: &Rc<AgentBridge>, params: AxisInputParams) -> AgentResponse {
    let Some(axis) = Axis::from_name(&params.axis) else {
        return AgentResponse::err(AgentError::bad_request(format!(
            "unknown axis name: {}",
            params.axis
        )));
    };
    bridge.input_bridge.borrow().set_axis(axis, params.value);
    AgentResponse::Ok
}

/// `/v1/time/step` — queue N fixed-step frames. Requires a prior
/// `/v1/time/pause`.
pub fn handle_step(bridge: &Rc<AgentBridge>, params: StepTimeParams) -> AgentResponse {
    if !bridge.paused.get() {
        return AgentResponse::err(AgentError::conflict(
            "must pause time before requesting fixed-step frames",
        ));
    }
    if params.frames == 0 {
        return AgentResponse::Ok;
    }
    bridge
        .requested_steps
        .set(bridge.requested_steps.get().saturating_add(params.frames));
    bridge.requested_dt.set(params.dt.unwrap_or(0.0).max(0.0));
    AgentResponse::Ok
}

/// `/v1/screenshot` — read back the last presented frame, if a
/// rendering engine handle has been wired onto the bridge.
pub fn handle_screenshot(bridge: &Rc<AgentBridge>) -> AgentResponse {
    let engine = match bridge.rendering_engine.borrow().clone() {
        Some(e) => e,
        None => return AgentResponse::Screenshot(ScreenshotResponse::default()),
    };
    match engine.borrow_mut().capture_last_frame() {
        Some(frame) => AgentResponse::Screenshot(ScreenshotResponse {
            width: frame.width,
            height: frame.height,
            encoded: true,
            rgba: frame.rgba,
        }),
        None => AgentResponse::Screenshot(ScreenshotResponse::default()),
    }
}

/// `/v1/perf` — snapshot the radiance perf registry.
pub fn handle_perf_metrics() -> AgentResponse {
    use agent_server::protocol::{PerfMetric, PerfMetricsResponse};

    let entries = radiance::perf::snapshot();
    let metrics = entries
        .into_iter()
        .map(|(name, snapshot)| match snapshot {
            radiance::perf::MetricSnapshot::Timing {
                calls,
                avg_ns,
                max_ns,
            } => PerfMetric::Timing {
                name: name.to_string(),
                calls,
                avg_ns,
                max_ns,
            },
            radiance::perf::MetricSnapshot::Counter { frame, total } => PerfMetric::Counter {
                name: name.to_string(),
                frame,
                total,
            },
            radiance::perf::MetricSnapshot::Gauge { last, max } => PerfMetric::Gauge {
                name: name.to_string(),
                last,
                max,
            },
        })
        .collect();
    AgentResponse::PerfMetrics(PerfMetricsResponse {
        enabled: radiance::perf::enabled(),
        metrics,
    })
}
