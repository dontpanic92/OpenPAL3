"""Unit tests for `pal4_planner.graph`."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from pal4_planner.graph import Graph, derive_gates_from_trace
from pal4_planner.trace import (
    Branch,
    CallSys,
    FnEnter,
    FnExit,
    GlobalRead,
    GlobalWrite,
    TraceEvent,
)


def _ev(seq: int, kind) -> TraceEvent:
    return TraceEvent(seq=seq, kind=kind)


def test_gate_inference_pairs_global_read_with_branch():
    events = [
        _ev(1, FnEnter(name="f", function_index=0, depth=1)),
        _ev(2, GlobalRead(fn_name="f", pc=8, scope="shared", slot=0, value=10_300)),
        _ev(
            3,
            Branch(
                fn_name="f",
                pc=16,
                branch="jz",
                operand=10_300,
                offset=8,
                taken=False,
            ),
        ),
        _ev(4, FnExit(name="f", depth=1)),
    ]
    gates = derive_gates_from_trace(events)
    assert len(gates) == 1
    g = gates[0]
    assert g.kind == "global"
    assert g.slot_or_sysfn == "0"
    assert g.observed_value == 10_300
    assert g.taken is False


def test_gate_inference_pairs_callsys_with_branch():
    events = [
        _ev(1, FnEnter(name="g", function_index=0, depth=1)),
        _ev(
            2,
            CallSys(
                fn_name="g",
                pc=4,
                sysfn_index=99,
                sysfn_name="giHasItem",
                sp_before=1024,
                sp_after=1020,
                r1_after=0,
            ),
        ),
        _ev(
            3,
            Branch(
                fn_name="g",
                pc=12,
                branch="jnz",
                operand=0,
                offset=8,
                taken=False,
            ),
        ),
    ]
    gates = derive_gates_from_trace(events)
    assert any(
        g.kind == "sysfn"
        and g.slot_or_sysfn == "giHasItem"
        and g.observed_value == 0
        and g.taken is False
        for g in gates
    )


def test_gate_inference_ignores_branch_in_different_function():
    events = [
        _ev(1, FnEnter(name="f", function_index=0, depth=1)),
        _ev(2, GlobalRead(fn_name="f", pc=8, scope="shared", slot=3, value=42)),
        _ev(3, FnExit(name="f", depth=1)),  # left the function
        _ev(4, FnEnter(name="g", function_index=1, depth=1)),
        _ev(
            5,
            Branch(
                fn_name="g",
                pc=4,
                branch="jz",
                operand=0,
                offset=8,
                taken=True,
            ),
        ),
    ]
    gates = derive_gates_from_trace(events)
    # GlobalRead in `f` should not be paired with the Branch in `g`.
    assert gates == []


def test_graph_records_fire_and_round_trips_trace():
    with tempfile.TemporaryDirectory() as td:
        g = Graph(Path(td) / "db.sqlite")
        events = [
            _ev(1, GlobalWrite(fn_name="f", pc=8, scope="shared", slot=0, value=10_600)),
        ]
        fire_id = g.record_fire(
            scene="Q01",
            block="Q01",
            name="ev_test",
            fn="func1001",
            settled=True,
            waited_frames=2,
            globals_before=[0, 0],
            globals_after=[10_600, 0],
            transitioned_to=("Q01", "N03"),
            trace=events,
        )
        assert fire_id > 0
        rec = g.last_fire()
        assert rec is not None
        assert (rec.scene, rec.block, rec.name) == ("Q01", "Q01", "ev_test")
        assert rec.transitioned_to_scene == "Q01"
        assert rec.transitioned_to_block == "N03"
        traced = g.trace_for_fire(fire_id)
        assert traced[0]["kind"] == "GlobalWrite"
        assert traced[0]["payload"]["value"] == 10_600
        g.close()
