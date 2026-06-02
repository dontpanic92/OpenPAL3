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


def test_planner_falls_back_to_object_interact():
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
                {"name": "1", "research_function": "func1012"},  # synthetic name skipped
                {"name": "MO002", "research_function": "func2002"},
            ],
        )
        assert pick is not None
        assert pick.kind == "object"
        assert pick.name == "MO002"
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
