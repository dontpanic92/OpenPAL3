"""Planner brain: pick the next trigger to fire, given current state.

This is intentionally a *small* planner. It implements the "explore"
strategy from the design doc:

1. Read live triggers for the current block.
2. Filter to those with a function bound and not yet fired in this
   `(scene, block)` pair (per the graph DB).
3. Score each remaining candidate:
   - Catalog says it transitions *out* of the current block (highest).
   - Catalog says it has writes / reads (mid — it's plot-relevant).
   - Catalog has *any* entry for it (low — known trigger).
   - Otherwise: just a live trigger with no metadata.
4. Return the highest-scoring candidate, falling back to GOB
   `research_function` objects when no triggers are left.

The goal-seek BFS with save/load backtracking is intentionally out of
scope for this iteration; the next planner pass uses the dynamic
graph (`graph.gates`) to plan prerequisite fires that satisfy a
specific gate, and that's where save/load comes in.
"""

from __future__ import annotations

import dataclasses
from typing import List, Optional, Tuple

from .catalog import Catalog, CatalogTrigger
from .graph import Graph


@dataclasses.dataclass
class Candidate:
    """One actionable target for the planner."""

    kind: str  # "trigger" | "object"
    name: str
    score: int
    catalog_trigger: Optional[CatalogTrigger] = None


class ExplorePlanner:
    """Stateless picker. Each call queries the graph DB for "what
    have we already fired in this block?" — no in-memory state.
    """

    def __init__(self, catalog: Catalog, graph: Graph):
        self.catalog = catalog
        self.graph = graph

    def pick(
        self,
        scene: str,
        block: str,
        live_triggers: List[dict],
        live_objects: List[dict],
    ) -> Optional[Candidate]:
        already_fired = {
            rec.name for rec in self.graph.fires_in(scene, block) if rec.settled
        }
        # Catalog-aware scoring for live triggers.
        candidates: List[Candidate] = []
        cat_blk = self.catalog.block(scene, block)
        cat_triggers = {t.name: t for t in (cat_blk.triggers if cat_blk else [])}

        for t in live_triggers:
            name = t.get("name", "")
            fn = t.get("function", "")
            if not name or not fn:
                continue
            if name in already_fired:
                continue
            ct = cat_triggers.get(name)
            score = _score_trigger(ct, scene, block)
            candidates.append(
                Candidate(kind="trigger", name=name, score=score, catalog_trigger=ct)
            )

        if candidates:
            candidates.sort(key=lambda c: c.score, reverse=True)
            return candidates[0]

        # Object-interact fallback. Skip purely-numeric names and
        # objects without a research_function.
        for o in live_objects:
            name = o.get("name", "")
            rf = o.get("research_function") or ""
            if not name or not rf:
                continue
            if name.isdigit():
                continue
            obj_key = f"obj:{name}"
            if obj_key in already_fired:
                continue
            return Candidate(kind="object", name=name, score=0)

        return None


def _score_trigger(ct: Optional[CatalogTrigger], scene: str, block: str) -> int:
    if ct is None:
        return 1  # live but uncatalogued — fallback exploration value
    if ct.is_wall():
        return -1  # never pick wall/camera triggers
    leave = sum(
        1
        for (s, b) in ct.transitions
        if s.lower() != scene.lower() or b != block
    )
    stay = sum(
        1
        for (s, b) in ct.transitions
        if s.lower() == scene.lower() and b == block
    )
    plot_relevance = len(ct.reads) + sum(1 for w in ct.writes if w.value is not None)
    # Heavily prefer "leaves block"; medium for "has plot side effects";
    # small for "stays in block" (still useful to make NPCs visible).
    return 1000 * leave + 50 * plot_relevance + 5 * stay + 10


def drift_summary(
    catalog_trigger: Optional[CatalogTrigger],
    fired_globals: Tuple[List[int], List[int]],
    actually_transitioned_to: Optional[Tuple[str, str]],
    scene: str,
    block: str,
) -> List[str]:
    """Return human-readable notes about how the observed fire
    diverged from the catalog's prediction. Empty list = "no
    drift detected"; informational only, not a hard failure.
    """
    notes: List[str] = []
    if catalog_trigger is None:
        notes.append("trigger not in static catalog; recorded as exploration")
        return notes
    before, after = fired_globals
    # Globals the catalog promised to write
    for w in catalog_trigger.writes:
        if w.value is None:
            continue
        idx = w.global_slot
        if idx >= len(after):
            continue
        observed = after[idx]
        if observed != w.value:
            notes.append(
                f"global[{idx}] expected -> {w.value} but observed {observed} "
                f"(was {before[idx] if idx < len(before) else '?'})"
            )
    # Transitions
    expected = {
        (s.lower(), b)
        for (s, b) in catalog_trigger.transitions
        if (s.lower(), b) != (scene.lower(), block)
    }
    if expected and actually_transitioned_to is None:
        notes.append(
            f"expected one of {sorted(expected)!r} transitions; observed none"
        )
    elif (
        actually_transitioned_to is not None
        and expected
        and (actually_transitioned_to[0].lower(), actually_transitioned_to[1])
        not in expected
    ):
        notes.append(
            f"expected one of {sorted(expected)!r}; observed "
            f"{actually_transitioned_to!r}"
        )
    return notes
