"""Unit tests for `Client.fire_trigger_sync` reply-timeout recovery.

Regression coverage for `progress_issues.md#B2`: the agent server's
HTTP transport has a hard-coded `reply_timeout` (default 5 s) that is
independent of `FireTriggerParams.timeout_ms`. When it trips, the
server returns HTTP 500 with `"game thread did not reply within Ns"`;
the planner used to treat that as a hard failure and race-fire the
next trigger while the engine was still running. The recovery path
now polls `/v1/state` for `script_running == false` and synthesises a
`FireResult` so the explore loop can keep walking.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from pal4_planner.client import AgentError, Client, FireResult


class _FakeClient(Client):
    """Replaces `_request` so tests don't touch the network."""

    def __init__(self, scripted_responses: list, state_sequence: list[dict]):
        super().__init__(base_url="http://127.0.0.1:0", timeout=0.01, retry_409=0)
        self._scripted = list(scripted_responses)
        self._states = list(state_sequence)

    def _request(self, method, path, body=None):
        if path == "/v1/state" and method == "GET":
            if not self._states:
                return {"type": "state", "data": {"script_running": False}}
            head = self._states.pop(0)
            return {"type": "state", "data": head}
        if not self._scripted:
            raise AssertionError(f"unexpected request: {method} {path}")
        head = self._scripted.pop(0)
        if isinstance(head, AgentError):
            raise head
        return head


def _fire_trigger_response(name: str = "ev_test") -> dict:
    return {
        "type": "fire_trigger",
        "data": {
            "name": name,
            "settled": True,
            "trace_seq_start": 100,
            "trace_seq_end": 142,
            "waited_frames": 6,
            "current_script_fn": None,
        },
    }


def test_fire_trigger_sync_returns_normally_when_server_replies():
    c = _FakeClient(
        scripted_responses=[_fire_trigger_response("ev_normal")],
        state_sequence=[],
    )
    r = c.fire_trigger_sync("ev_normal")
    assert r.name == "ev_normal"
    assert r.settled is True
    assert r.trace_seq_start == 100


def test_fire_trigger_sync_recovers_from_reply_timeout():
    """Server returns 500 'game thread did not reply within 5s'.
    The client must poll /v1/state, see script_running flip to False,
    and synthesise a settled FireResult."""
    timeout_err = AgentError(500, "internal", "game thread did not reply within 5s")
    c = _FakeClient(
        scripted_responses=[timeout_err],
        state_sequence=[
            {"script_running": True},
            {"script_running": True},
            {"script_running": False},  # engine settles on the third poll
        ],
    )
    r = c.fire_trigger_sync("ev_slow", wait_idle_timeout_sec=2.0)
    assert isinstance(r, FireResult)
    assert r.name == "ev_slow"
    assert r.settled is True
    # No trace window because the server's structured response was lost.
    assert r.trace_seq_start is None
    assert r.trace_seq_end is None


def test_fire_trigger_sync_re_raises_non_timeout_errors():
    not_timeout = AgentError(409, "conflict", "a script is already running")
    c = _FakeClient(scripted_responses=[not_timeout], state_sequence=[])
    try:
        c.fire_trigger_sync("ev_busy")
    except AgentError as e:
        assert e.kind == "conflict"
    else:
        raise AssertionError("expected AgentError")


def test_fire_trigger_sync_gives_up_if_engine_never_idles():
    """If the reply timeout fires AND the engine never idles within
    `wait_idle_timeout_sec`, the client must surface a clear error
    rather than hang or claim a phantom success."""
    timeout_err = AgentError(500, "internal", "game thread did not reply within 5s")
    # state always reports script_running=True
    c = _FakeClient(
        scripted_responses=[timeout_err],
        state_sequence=[{"script_running": True}] * 20,
    )
    try:
        c.fire_trigger_sync("ev_wedged", wait_idle_timeout_sec=0.1)
    except AgentError as e:
        assert "engine did not idle" in e.message
    else:
        raise AssertionError("expected AgentError")
