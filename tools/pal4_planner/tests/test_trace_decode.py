"""Unit tests for `pal4_planner.trace`."""

from __future__ import annotations

import sys
from pathlib import Path

# Make the sibling package importable when pytest is invoked from
# the repo root.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from pal4_planner.trace import (
    Branch,
    CallSys,
    FnEnter,
    FnExit,
    GlobalRead,
    GlobalWrite,
    Suspend,
    TraceEvent,
    decode_all,
    decode_event,
)


def test_decode_round_trip_all_kinds():
    raws = [
        {
            "seq": 1,
            "kind": {"type": "fn_enter", "name": "f", "function_index": 4, "depth": 1},
        },
        {"seq": 2, "kind": {"type": "fn_exit", "name": "f", "depth": 1}},
        {
            "seq": 3,
            "kind": {
                "type": "branch",
                "fn_name": "f",
                "pc": 32,
                "branch": "jz",
                "operand": 0,
                "offset": 8,
                "taken": True,
            },
        },
        {
            "seq": 4,
            "kind": {
                "type": "call_sys",
                "fn_name": "f",
                "pc": 40,
                "sysfn_index": 99,
                "sysfn_name": "giHasItem",
                "sp_before": 1024,
                "sp_after": 1020,
                "r1_after": 0,
            },
        },
        {
            "seq": 5,
            "kind": {
                "type": "global_read",
                "fn_name": "f",
                "pc": 48,
                "scope": "shared",
                "slot": 0,
                "value": 10300,
            },
        },
        {
            "seq": 6,
            "kind": {
                "type": "global_write",
                "fn_name": "f",
                "pc": 56,
                "scope": "shared",
                "slot": 0,
                "value": 11400,
            },
        },
        {"seq": 7, "kind": {"type": "suspend", "fn_name": "f", "pc": 64}},
    ]
    events = decode_all(raws)
    assert [type(e.kind).__name__ for e in events] == [
        "FnEnter",
        "FnExit",
        "Branch",
        "CallSys",
        "GlobalRead",
        "GlobalWrite",
        "Suspend",
    ]
    assert events[3].kind.sysfn_name == "giHasItem"
    assert events[5].kind.value == 11400


def test_decode_unknown_kind_raises():
    import pytest

    with pytest.raises(ValueError):
        decode_event({"seq": 1, "kind": {"type": "unknown_op"}})
