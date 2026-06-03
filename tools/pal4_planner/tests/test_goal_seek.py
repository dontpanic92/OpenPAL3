"""Unit tests for `GoalSeekPlanner` (mocked Client; no network).

Validates the trigger-driven prerequisite-chain workflow from
`docs/pal4_plot_catalog.md#plot-advancement-not-set-the-flag`:

- Saves before each attempt, loads on failure to back out drift.
- Walks the catalog's transition graph to reach remote writers.
- Reports the writer that actually advanced the gated global.
- Excludes value-None (computed-RHS) writers when a specific
  target value is being sought.
"""

from __future__ import annotations

import dataclasses
import json
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from pal4_planner.catalog import Catalog
from pal4_planner.client import FireResult
from pal4_planner.graph import Graph
from pal4_planner.planner import GoalSeekPlanner


# Sized to make hand-checked indices easy.
INITIAL_GLOBALS = [10_201, 0, 0, 0, 0]


class MockClient:
    """Records all calls. `globals` returns a mutable array; tests
    flip values via `programmed_writes` when a specific
    fire_trigger / interact lands.
    """

    def __init__(
        self,
        *,
        scene: str = "Q01",
        block: str = "Q01",
        initial_globals: Optional[List[int]] = None,
        # Map: trigger/object name -> dict of slot->value to apply on fire.
        programmed_writes: Optional[Dict[str, Dict[int, int]]] = None,
        # Trigger/object names whose fire/interact should raise to
        # simulate a failed step.
        failing_actions: Optional[List[str]] = None,
    ):
        self.scene = scene
        self.block = block
        self._globals = list(initial_globals or INITIAL_GLOBALS)
        self._writes = programmed_writes or {}
        self._failing = set(failing_actions or [])
        self.calls: List[Any] = []
        self._saved: List[List[int]] = []
        self._saved_scene: List[tuple[str, str]] = []

    def globals(self):
        return list(self._globals)

    def state(self):
        return {"scene": self.scene, "block": self.block, "script_running": False, "movie_playing": False}

    def wait_for_idle(self, timeout_sec: float = 30.0) -> bool:
        # Tests assume each fire/interact returns to idle synchronously
        # — the MockClient applies writes inline. Always return True.
        return True

    def save(self, slot: int):
        self.calls.append(("save", slot))
        self._saved.append(list(self._globals))
        self._saved_scene.append((self.scene, self.block))

    def load(self, slot: int):
        self.calls.append(("load", slot))
        if self._saved:
            self._globals = self._saved.pop()
            self.scene, self.block = self._saved_scene.pop()

    def fire_trigger_sync(self, name: str, **_) -> FireResult:
        self.calls.append(("fire", name))
        if name in self._failing:
            raise RuntimeError(f"fire {name} fails")
        # Apply programmed writes for this trigger.
        for slot, value in (self._writes.get(name) or {}).items():
            if slot < len(self._globals):
                self._globals[slot] = value
        return FireResult(
            name=name,
            settled=True,
            trace_seq_start=0,
            trace_seq_end=0,
            waited_frames=0,
            current_script_fn=None,
        )

    def interact(self, name: str):
        self.calls.append(("interact", name))
        if name in self._failing:
            raise RuntimeError(f"interact {name} fails")
        for slot, value in (self._writes.get(name) or {}).items():
            if slot < len(self._globals):
                self._globals[slot] = value


def _build_catalog(tmp: Path) -> Catalog:
    """Q01/Q01 starts. Two writers for slot 0:
       - ev_M01_1_5 (M01/1) writes value=11400 — reachable via ev_q01_to_m01.
       - ev_no_path (Z99/1) writes value=11400 — UNREACHABLE.
    Plus a same-block writer ev_local_writer (Q01/Q01) for value=10300.
    """
    p = tmp / "plot.json"
    p.write_text(
        json.dumps(
            {
                "scenes": {
                    "q01": {
                        "blocks": {
                            "Q01": {
                                "triggers": [
                                    {
                                        "name": "ev_q01_to_m01",
                                        "function": "f_q01_to_m01",
                                        "center": [0, 0, 0],
                                        "half_size": [1, 1, 1],
                                        "shape": "box",
                                        "kind": "trigger",
                                        "reads": [],
                                        "writes": [],
                                        "transitions": [["M01", "1"]],
                                    },
                                    {
                                        "name": "ev_local_writer",
                                        "function": "f_local",
                                        "center": [0, 0, 0],
                                        "half_size": [1, 1, 1],
                                        "shape": "box",
                                        "kind": "trigger",
                                        "reads": [],
                                        "writes": [{"global": 0, "value": 10_300}],
                                        "transitions": [],
                                    },
                                ],
                                "objects": [],
                            }
                        }
                    },
                    "m01": {
                        "blocks": {
                            "1": {
                                "triggers": [
                                    {
                                        "name": "ev_M01_1_5",
                                        "function": "func1001",
                                        "center": [0, 0, 0],
                                        "half_size": [1, 1, 1],
                                        "shape": "box",
                                        "kind": "trigger",
                                        "reads": [],
                                        "writes": [
                                            {"global": 0, "value": 11_400}
                                        ],
                                        "transitions": [],
                                    }
                                ],
                                "objects": [],
                            }
                        }
                    },
                    "z99": {"blocks": {"1": {"triggers": [], "objects": []}}},
                },
                "plot_index": {
                    "0": [
                        {
                            "value": 11_400,
                            "scene": "m01",
                            "block": "1",
                            "trigger": "ev_M01_1_5",
                            "fn": "func1001",
                        },
                        {
                            "value": 11_400,
                            "scene": "z99",
                            "block": "1",
                            "trigger": "ev_no_path",
                            "fn": "fX",
                        },
                        {
                            "value": 10_300,
                            "scene": "q01",
                            "block": "Q01",
                            "trigger": "ev_local_writer",
                            "fn": "f_local",
                        },
                        {
                            "value": None,  # computed-RHS, excluded when value=11400
                            "scene": "q01",
                            "block": "Q01",
                            "trigger": "ev_computed",
                            "fn": "f_x",
                        },
                    ]
                },
            }
        ),
        encoding="utf-8",
    )
    return Catalog(p)


def test_goal_seek_walks_path_and_succeeds():
    """ev_M01_1_5 in M01/1 lands g[0]=11400; reachable via
    ev_q01_to_m01. The planner must save, walk the path, fire
    the writer, and observe globals advance."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        client = MockClient(
            programmed_writes={
                "ev_q01_to_m01": {},  # just transitions, no writes
                "ev_M01_1_5": {0: 11_400},
            },
        )
        # Path-walking should also update the live scene. We simulate
        # that bookkeeping in `fire_trigger_sync` for path triggers:
        original_fire = client.fire_trigger_sync

        def fire_with_transition(name, **kwargs):
            r = original_fire(name, **kwargs)
            if name == "ev_q01_to_m01":
                client.scene, client.block = "M01", "1"
            return r

        client.fire_trigger_sync = fire_with_transition

        planner = GoalSeekPlanner(catalog, g, client, save_slot=99)
        result = planner.push_toward(slot=0, value=11_400)

        assert result.success is True, result.notes
        assert result.observed_before == 10_201
        assert result.observed_after == 11_400
        assert result.writer is not None
        assert result.writer.trigger == "ev_M01_1_5"

        # Should have saved before attempting.
        assert ("save", 99) in client.calls
        # Should have fired the path trigger AND the writer trigger.
        names = [c[1] for c in client.calls if c[0] == "fire"]
        assert names == ["ev_q01_to_m01", "ev_M01_1_5"]
        # Should NOT have loaded — success means we keep the state.
        assert ("load", 99) not in client.calls

        g.close()


def test_goal_seek_loads_when_writer_is_a_noop():
    """First-attempted writer ev_M01_1_5 is programmed to be a no-op
    (it 'fires' but doesn't change globals); the planner must load
    the save, drop back to the next writer. The next writer landing
    the same value=11400 is ev_no_path which is unreachable, so the
    overall result must fail cleanly."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        client = MockClient(
            # ev_M01_1_5 fires but doesn't actually move g[0].
            programmed_writes={},
        )
        original_fire = client.fire_trigger_sync

        def fire_with_transition(name, **kwargs):
            r = original_fire(name, **kwargs)
            if name == "ev_q01_to_m01":
                client.scene, client.block = "M01", "1"
            return r

        client.fire_trigger_sync = fire_with_transition

        planner = GoalSeekPlanner(catalog, g, client, save_slot=99)
        result = planner.push_toward(slot=0, value=11_400)

        assert result.success is False
        # Must have done at least one save → fire → load cycle.
        kinds = [c[0] for c in client.calls]
        assert "save" in kinds
        assert "load" in kinds
        # Should have explanatory notes for failures + unreachable
        # candidates.
        joined = " | ".join(result.notes)
        assert "did not advance globals[0]" in joined
        assert "no path" in joined  # ev_no_path got pruned by path_to
        g.close()


def test_goal_seek_skips_when_target_already_observed():
    """If globals[slot] already equals `value`, return success
    immediately without firing anything (no spurious save/load
    cycle)."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        initial = list(INITIAL_GLOBALS)
        initial[0] = 11_400
        client = MockClient(initial_globals=initial)
        planner = GoalSeekPlanner(catalog, g, client)
        result = planner.push_toward(slot=0, value=11_400)
        assert result.success is True
        assert result.writer is None
        assert client.calls == []  # no save/fire/load issued
        g.close()


def test_goal_seek_excludes_computed_rhs_when_value_set():
    """ev_computed has `value: null` in the catalog (computed RHS).
    When push_toward is called with a specific `value`, that writer
    must be excluded — we can't prove it would land the target.
    With `value=None`, it should be considered."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")

        # With value=11400, the only same-scene writers in plot_index
        # are ev_M01_1_5 (M01/1) and ev_no_path (Z99/1, unreachable);
        # ev_computed must NOT be enumerated.
        client = MockClient()
        planner = GoalSeekPlanner(catalog, g, client)
        for_value = catalog.plot_index_for(0, 11_400)
        triggers = {w.trigger for w in for_value}
        assert "ev_computed" not in triggers

        # Without a value filter, ev_computed appears.
        any_value = catalog.plot_index_for(0)
        triggers_all = {w.trigger for w in any_value}
        assert "ev_computed" in triggers_all

        g.close()


def test_goal_seek_refuses_backward_target():
    """If `globals[slot]` is already strictly greater than the
    requested `value`, `push_toward` must refuse rather than
    enumerate writers (PAL4 plot flags are monotonic; "going back"
    is meaningless and would waste max_writers attempts on writers
    that all immediately reload). Result has success=False with a
    clear note so the explore loop can fall back to a forward
    target."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        initial = list(INITIAL_GLOBALS)
        initial[0] = 11_400  # already past 10_201 etc.
        client = MockClient(initial_globals=initial)
        planner = GoalSeekPlanner(catalog, g, client)
        # Try to push back to a value below the live state.
        result = planner.push_toward(slot=0, value=10_201)
        assert result.success is False
        assert result.writer is None
        assert result.observed_before == 11_400
        assert result.observed_after == 11_400
        # No save/fire/load cycle should be issued.
        assert client.calls == []
        joined = " | ".join(result.notes)
        assert "already past" in joined
        g.close()


def test_goal_seek_value_none_filters_to_forward_writers():
    """When called with `value=None` (the fallback "advance the
    slot by any amount" mode), `push_toward` must skip writers
    landing values <= current — those would be backward or no-op
    moves. value=None writers (computed RHS) survive."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        initial = list(INITIAL_GLOBALS)
        initial[0] = 11_400  # at the catalog's highest concrete writer.
        client = MockClient(initial_globals=initial)
        # No programmed writes — every candidate will be a no-op so
        # `push_toward` will try until exhausted.
        planner = GoalSeekPlanner(catalog, g, client)
        result = planner.push_toward(slot=0, value=None)
        # The 11_400 and 10_300 writers are <= current and must be
        # filtered before any save/load cycle. The only survivor is
        # `ev_computed` (value=None, computed RHS).
        joined = " | ".join(result.notes)
        assert "ev_M01_1_5=11400" not in joined
        assert "ev_no_path=11400" not in joined
        assert "ev_local_writer=10300" not in joined
        # The computed-RHS writer is the only one that should be
        # tried (and it will fail — MockClient doesn't programme it
        # to actually move globals).
        names = [c[1] for c in client.calls if c[0] == "fire"]
        # ev_computed has no transitions/path, so it's just fired
        # directly in q01/Q01. The path planner returns [] meaning
        # "already at target block".
        assert names == ["ev_computed"], names
        g.close()
