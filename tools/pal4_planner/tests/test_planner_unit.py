"""Unit tests for `pal4_planner.planner` (mocked Client; no network)."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import json

from pal4_planner.catalog import Catalog
from pal4_planner.graph import Graph
from pal4_planner.planner import ExplorePlanner, drift_summary


def _build_catalog(tmp: Path) -> Catalog:
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
                                        "name": "ev_stay",
                                        "function": "func2006",
                                        "center": [0, 0, 0],
                                        "half_size": [1, 1, 1],
                                        "shape": "box",
                                        "kind": "trigger",
                                        "reads": [0],
                                        "writes": [],
                                        "transitions": [],
                                    },
                                    {
                                        "name": "ev_leave",
                                        "function": "func1001",
                                        "center": [0, 0, 0],
                                        "half_size": [1, 1, 1],
                                        "shape": "plane",
                                        "kind": "trigger",
                                        "reads": [0],
                                        "writes": [{"global": 0, "value": 11_400}],
                                        "transitions": [["Q01", "N01"]],
                                    },
                                    {
                                        "name": "wall01",
                                        "function": "",
                                        "center": [0, 0, 0],
                                        "half_size": [1, 1, 1],
                                        "shape": "other",
                                        "kind": "wall",
                                        "reads": [],
                                        "writes": [],
                                        "transitions": [],
                                    },
                                ],
                                "objects": [],
                            }
                        }
                    }
                }
            }
        ),
        encoding="utf-8",
    )
    return Catalog(p)


def test_planner_prefers_block_leaving_trigger():
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        planner = ExplorePlanner(catalog, g)

        live = [
            {"name": "ev_stay", "function": "func2006"},
            {"name": "ev_leave", "function": "func1001"},
            {"name": "wall01", "function": ""},
        ]
        pick = planner.pick("Q01", "Q01", live, [])
        assert pick is not None
        assert pick.name == "ev_leave"
        g.close()


def test_planner_skips_already_fired_triggers():
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        # Pretend ev_leave was already fired successfully.
        g.record_fire(
            scene="Q01",
            block="Q01",
            name="ev_leave",
            fn="func1001",
            settled=True,
            waited_frames=1,
            globals_before=[0],
            globals_after=[11_400],
            transitioned_to=("Q01", "N01"),
            trace=[],
        )
        planner = ExplorePlanner(catalog, g)
        live = [
            {"name": "ev_stay", "function": "func2006"},
            {"name": "ev_leave", "function": "func1001"},
        ]
        pick = planner.pick("Q01", "Q01", live, [])
        assert pick is not None
        assert pick.name == "ev_stay"
        g.close()


def test_planner_falls_back_to_object_interact_without_isdigit_skip():
    """Objects with purely-numeric names are NOT skipped — PAL4 uses
    numeric names (e.g. `"1"` / `"2"` in Q01/Q01) for plot-pushing
    GOB entries. The planner relies on catalog scoring instead of a
    name heuristic. See `progress_issues.md#B4`."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        planner = ExplorePlanner(catalog, g)
        pick = planner.pick(
            "Q01",
            "Q01",
            [],
            [
                {"name": "1", "research_function": "func1012"},
                {"name": "MO002", "research_function": "func2002"},
            ],
        )
        assert pick is not None
        assert pick.kind == "object"
        # Either is acceptable; both are equally-uncatalogued. The
        # important property is that "1" was *considered*, not silently
        # dropped: the test that lock-steps with `record_interact`
        # below proves dedup advances regardless of which we pick.
        assert pick.name in ("1", "MO002")
        g.close()


def test_planner_dedupes_interacted_objects_via_record_interact():
    """`Graph.record_interact` must update the planner's `already_fired`
    set so a second `pick()` with the same live objects returns a
    different candidate (or `None` once exhausted). Regression test
    for `progress_issues.md#B3`."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        g = Graph(tmp / "db.sqlite")
        planner = ExplorePlanner(catalog, g)
        live_objs = [
            {"name": "1", "research_function": "func1012"},
            {"name": "2", "research_function": "func1013"},
        ]
        first = planner.pick("Q01", "Q01", [], live_objs)
        assert first is not None and first.kind == "object"
        g.record_interact(scene="Q01", block="Q01", object_name=first.name)
        second = planner.pick("Q01", "Q01", [], live_objs)
        assert second is not None and second.kind == "object"
        assert second.name != first.name
        g.record_interact(scene="Q01", block="Q01", object_name=second.name)
        third = planner.pick("Q01", "Q01", [], live_objs)
        assert third is None
        g.close()


def test_planner_uses_catalog_kind_to_filter_walls():
    """When a trigger is catalogued as `kind == "wall"`, the planner
    must filter it even if it has a bound function name. Conversely,
    a trigger with `function == ""` but absent from the catalog
    (legacy fallback path) must still be filtered. And a third
    "boxed wall" case (catalogued as `kind == "trigger"` because the
    dumper's classifier requires `shape == "other"` to tag walls, but
    actually has `function == ""` — `wall01`/`wall02` in PAL4 Q01/Q01)
    must also be filtered by the live `function == ""` check.
    Regression test for `progress_issues.md#B5`."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
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
                                            "name": "ev_real",
                                            "function": "func1001",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                            "kind": "trigger",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [["Q01", "N01"]],
                                        },
                                        {
                                            "name": "ev_named_wall",
                                            "function": "func9999",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "other",
                                            "kind": "wall",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [],
                                        },
                                        {
                                            # wall01-style: catalogued as
                                            # "trigger" because shape != "other",
                                            # but live engine has no fn bound.
                                            "name": "wall01",
                                            "function": "",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                            "kind": "trigger",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [],
                                        },
                                    ],
                                    "objects": [],
                                }
                            }
                        }
                    }
                }
            ),
            encoding="utf-8",
        )
        catalog = Catalog(p)
        g = Graph(tmp / "db.sqlite")
        planner = ExplorePlanner(catalog, g)
        live = [
            {"name": "ev_named_wall", "function": "func9999"},
            {"name": "wall01", "function": ""},
            {"name": "ev_real", "function": "func1001"},
            {"name": "uncat_wall", "function": ""},  # legacy fallback path
        ]
        pick = planner.pick("Q01", "Q01", live, [])
        assert pick is not None
        assert pick.name == "ev_real"
        g.close()


def test_drift_summary_flags_missing_global_write():
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        trig = catalog.trigger("Q01", "Q01", "ev_leave")
        notes = drift_summary(trig, ([0, 0], [0, 0]), ("Q01", "N01"), "Q01", "Q01")
        assert any("global[0]" in n for n in notes)


def test_catalog_synthesises_missing_blocks():
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        blk = catalog.block("q01", "N01")
        assert blk is not None
        assert blk.synthesized is True
        assert blk.triggers == []


def test_drift_summary_distinguishes_synthesized_block_from_unknown_trigger():
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        catalog = _build_catalog(tmp)
        # Synthesised block (Q01/N01 only reachable via transition).
        synth_blk = catalog.block("q01", "N01")
        assert synth_blk is not None and synth_blk.synthesized
        notes = drift_summary(
            None,
            ([0], [0]),
            None,
            "Q01",
            "N01",
            catalog_block=synth_blk,
        )
        assert any("synthesised" in n for n in notes)
        assert not any(
            "trigger not in static catalog" in n for n in notes
        )

        # Real block, unknown trigger (catalog_trigger=None).
        real_blk = catalog.block("q01", "Q01")
        assert real_blk is not None and not real_blk.synthesized
        notes = drift_summary(
            None, ([0], [0]), None, "Q01", "Q01", catalog_block=real_blk
        )
        assert any("trigger not in static catalog" in n for n in notes)
        assert not any("synthesised" in n for n in notes)


def test_catalog_rejects_stale_pre_summary_schema():
    """A catalog produced by the pre-`#[serde(flatten)]` dumper has
    triggers without `kind`/`transitions`/`writes`/`reads`. The
    loader must hard-error (issue A2 / B7)."""
    from pal4_planner.catalog import StaleCatalogError

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
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
                                            "name": "ev_stale",
                                            "function": "func1001",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                        }
                                    ],
                                    "objects": [],
                                }
                            }
                        }
                    }
                }
            ),
            encoding="utf-8",
        )
        try:
            Catalog(p)
        except StaleCatalogError as e:
            assert "pal4_plot_dump" in str(e)
            assert "Regenerate" in str(e)
        else:
            raise AssertionError("expected StaleCatalogError")


def test_catalog_accepts_empty_scenes_as_noop():
    """An empty catalog (no scenes / no triggers) should not raise —
    the loader is being asked to no-op, and there is nothing whose
    scoring could silently degrade."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        p = tmp / "plot.json"
        p.write_text(json.dumps({"scenes": {}}), encoding="utf-8")
        Catalog(p)  # must not raise


def test_catalog_loads_plot_index_and_filters_by_value():
    """`Catalog.plot_index_for(slot, value=...)` returns the reverse
    index of which trigger / fn writes which value into which slot.
    Used by `GoalSeekPlanner` to find prerequisite fires that satisfy
    a gate. Catalog schema includes this top-level map; see
    `docs/pal4_plot_catalog.md#plot-advancement-not-set-the-flag`."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
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
                                            "name": "ev_seed",
                                            "function": "f1",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                            "kind": "trigger",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [],
                                        }
                                    ],
                                    "objects": [],
                                }
                            }
                        }
                    },
                    "plot_index": {
                        "0": [
                            {
                                "value": 11400,
                                "scene": "m01",
                                "block": "1",
                                "trigger": "ev_M01_1_5",
                                "fn": "func1001",
                            },
                            {
                                "value": 10600,
                                "scene": "m01",
                                "block": "1",
                                "trigger": "ev_M01_1_6",
                                "fn": "func2002",
                            },
                            {
                                "value": None,  # computed RHS
                                "scene": "m01",
                                "block": "1",
                                "trigger": None,
                                "object": "barrel",
                                "fn": "func9000",
                            },
                        ]
                    },
                }
            ),
            encoding="utf-8",
        )
        catalog = Catalog(p)
        all_slot_0 = catalog.plot_index_for(0)
        assert len(all_slot_0) == 3
        names = [w.trigger or w.object_name for w in all_slot_0]
        assert "ev_M01_1_5" in names and "ev_M01_1_6" in names
        assert "barrel" in names  # object writer present

        # value filter: only entries with the exact literal V.
        only_11400 = catalog.plot_index_for(0, 11400)
        assert len(only_11400) == 1
        assert only_11400[0].trigger == "ev_M01_1_5"
        # value filter excludes computed-RHS writers.
        only_none = catalog.plot_index_for(0, 0)
        assert only_none == []
        # Unknown slot: empty list.
        assert catalog.plot_index_for(999) == []


def test_catalog_path_to_finds_transition_via_trigger():
    """`Catalog.path_to` BFS over the transition graph and returns
    the trigger sequence to walk between blocks. Same-node returns
    `[]`; no path returns `None`."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
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
                                            "function": "f1",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                            "kind": "trigger",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [["M01", "1"]],
                                        }
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
                                            "name": "ev_m01_to_m02",
                                            "function": "f2",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                            "kind": "trigger",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [["M02", "1"]],
                                        }
                                    ],
                                    "objects": [],
                                }
                            }
                        },
                        "m02": {"blocks": {"1": {"triggers": [], "objects": []}}},
                        "x99": {"blocks": {"1": {"triggers": [], "objects": []}}},
                    }
                }
            ),
            encoding="utf-8",
        )
        catalog = Catalog(p)
        # Same node: empty path.
        assert catalog.path_to("Q01", "Q01", "Q01", "Q01") == []
        # One hop.
        one = catalog.path_to("Q01", "Q01", "M01", "1")
        assert one == [("fire", "ev_q01_to_m01")]
        # Two hops, case-insensitive on scene.
        two = catalog.path_to("q01", "Q01", "m02", "1")
        assert two == [
            ("fire", "ev_q01_to_m01"),
            ("fire", "ev_m01_to_m02"),
        ]
        # Unreachable target.
        assert catalog.path_to("Q01", "Q01", "X99", "1") is None
        # Depth cap respected.
        assert catalog.path_to("Q01", "Q01", "M02", "1", max_hops=1) is None


def test_catalog_path_to_uses_object_interactions():
    """Objects with `transitions` are valid edges; the path step
    is `("interact", name)`."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        p = tmp / "plot.json"
        p.write_text(
            json.dumps(
                {
                    "scenes": {
                        "q01": {
                            "blocks": {
                                "Q01": {
                                    "triggers": [],
                                    "objects": [
                                        {
                                            "name": "portal",
                                            "kind": "generic",
                                            "position": [0, 0, 0],
                                            "research_function": "f_portal",
                                            "reads": [],
                                            "writes": [],
                                            "transitions": [["N01", "1"]],
                                        }
                                    ],
                                }
                            }
                        },
                        "n01": {"blocks": {"1": {"triggers": [], "objects": []}}},
                    }
                }
            ),
            encoding="utf-8",
        )
        catalog = Catalog(p)
        path = catalog.path_to("Q01", "Q01", "N01", "1")
        assert path == [("interact", "portal")]


def test_catalog_cmp_literal_lookup():
    """`Catalog.cmp_literal(scene, fn, pc)` returns the per-fn RHS
    literal recorded by `pal4_plot_dump` (catalog schema v2)."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
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
                                            "name": "ev1",
                                            "function": "func2013",
                                            "center": [0, 0, 0],
                                            "half_size": [1, 1, 1],
                                            "shape": "box",
                                            "kind": "trigger",
                                            "reads": [0],
                                            "writes": [],
                                            "transitions": [],
                                        }
                                    ],
                                    "objects": [],
                                }
                            },
                            "fns": {
                                "func2013": {
                                    "reads": [0],
                                    "writes": [],
                                    "sysfns": [],
                                    "calls": [],
                                    "cmp_literals": {"34": 160500, "60": None},
                                }
                            },
                        }
                    }
                }
            ),
            encoding="utf-8",
        )
        catalog = Catalog(p)
        # Scene name is case-insensitive (lower-cased internally).
        assert catalog.cmp_literal("Q01", "func2013", 34) == 160500
        assert catalog.cmp_literal("q01", "func2013", 34) == 160500
        # `None` literal (computed RHS) is preserved.
        assert catalog.cmp_literal("q01", "func2013", 60) is None
        # Missing pc / fn / scene gracefully return None.
        assert catalog.cmp_literal("q01", "func2013", 999) is None
        assert catalog.cmp_literal("q01", "noSuchFn", 34) is None
        assert catalog.cmp_literal("noSuchScene", "func2013", 34) is None
