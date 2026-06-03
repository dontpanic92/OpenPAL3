"""Planner brain: pick the next trigger to fire, given current state.

Two strategies:

1. `ExplorePlanner` — single-step "fire the highest-scoring live
   trigger in the current block". Used for breadth-first coverage of
   a scene.
2. `GoalSeekPlanner` — multi-step "to land `globals[slot] = V`,
   enumerate the catalog's `plot_index[slot]` writers, walk to each
   in turn via `Catalog.path_to`, and re-check on every step". Used
   by `drive.cmd_explore` as the "nothing actionable" recovery when
   the explore loop exhausts the current block but a known gate is
   blocking progress.

Per `docs/pal4_plot_catalog.md#plot-advancement-not-set-the-flag`,
**plot advancement is strictly trigger-driven** — the planner never
writes globals directly. Goal-seek satisfies a gate by finding and
firing a *different* trigger whose static `writes` lands the
required value, then retries the original gated trigger.
"""

from __future__ import annotations

import dataclasses
import json
from typing import List, Optional, Tuple

from .catalog import Catalog, CatalogBlock, CatalogObject, CatalogTrigger, PlotWriter
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
        current_globals: Optional[List[int]] = None,
    ) -> Optional[Candidate]:
        already_fired = {
            rec.name
            for rec in self.graph.fires_in(scene, block, current_globals=current_globals)
            if rec.settled
        }
        # Catalog-aware scoring for live triggers + interactive objects.
        candidates: List[Candidate] = []
        cat_blk = self.catalog.block(scene, block)
        cat_triggers = {t.name: t for t in (cat_blk.triggers if cat_blk else [])}
        cat_objects = {o.name: o for o in (cat_blk.objects if cat_blk else [])}

        for t in live_triggers:
            name = t.get("name", "")
            if not name:
                continue
            if name in already_fired:
                continue
            ct = cat_triggers.get(name)
            # Wall filter: catalog says wall, or live engine has no fn bound.
            if ct is not None and ct.is_wall():
                continue
            if not t.get("function", ""):
                continue
            score = _score_trigger(
                ct, scene, block, current_globals=current_globals, catalog=self.catalog
            )
            candidates.append(
                Candidate(kind="trigger", name=name, score=score, catalog_trigger=ct)
            )

        # Score interactive objects alongside triggers (see B4).
        for o in live_objects:
            name = o.get("name", "")
            rf = o.get("research_function") or ""
            if not name or not rf:
                continue
            obj_key = f"obj:{name}"
            if obj_key in already_fired:
                continue
            co = cat_objects.get(name)
            score = _score_object(
                co, scene, block, current_globals=current_globals, catalog=self.catalog
            )
            candidates.append(
                Candidate(kind="object", name=name, score=score, catalog_trigger=None)
            )

        if not candidates:
            return None
        candidates.sort(key=lambda c: c.score, reverse=True)
        return candidates[0]


def _gate_satisfaction_bonus(
    fn: str,
    reads: List[int],
    current_globals: Optional[List[int]],
    catalog: Optional[Catalog],
    scene: str,
) -> int:
    """`1` when at least one of the trigger/object's `reads` slots
    currently holds a value the catalog recorded as a compare-RHS
    literal for `fn` — i.e., one of `fn`'s gates would *pass* from
    the current globals. `0` otherwise (gate unknown, no
    cmp_literals recorded, or no match).

    Plot triggers in PAL4 follow the pattern
    `if globals[slot] == V_required: …; globals[slot] = V_next`.
    A trigger whose `V_required` matches live state is almost
    always the right next plot step — we score it well above
    a transition-only or wrong-gate trigger so the planner picks
    the open gate first, before navigating away.
    """
    if not fn or not reads or current_globals is None or catalog is None:
        return 0
    fn_body = catalog.fns.get(scene.lower(), {}).get(fn)
    if fn_body is None:
        return 0
    cmp_lits = fn_body.get("cmp_literals") or {}
    if not cmp_lits:
        return 0
    live_values = {
        current_globals[slot]
        for slot in reads
        if 0 <= slot < len(current_globals)
    }
    for v in cmp_lits.values():
        if v is not None and v in live_values:
            return 1
    return 0


def _score_trigger(
    ct: Optional[CatalogTrigger],
    scene: str,
    block: str,
    current_globals: Optional[List[int]] = None,
    catalog: Optional[Catalog] = None,
) -> int:
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
    has_known_writes = 1 if any(w.value is not None for w in ct.writes) else 0
    plot_relevance = len(ct.reads) + sum(1 for w in ct.writes if w.value is not None)
    gate_bonus = _gate_satisfaction_bonus(
        ct.function, ct.reads, current_globals, catalog, scene
    )
    # Tiers, highest first:
    #   gate_bonus  (5000) — gate from cmp_literals matches live globals,
    #                        so the trigger body will actually run.
    #   has_writes  (1000) — trigger writes a plot flag; firing it can
    #                        advance globals even without a recognized
    #                        gate (e.g. unconditional writers).
    #   leave       (100)  — transitions out of the current block; useful
    #                        navigation but less likely to be a plot beat.
    #   plot/stay   (5)    — fine-grained tiebreakers.
    #   constant    (10)   — baseline so catalogued triggers outrank
    #                        the `ct is None` fallback (which scores 1).
    return (
        5000 * gate_bonus
        + 1000 * has_known_writes
        + 100 * leave
        + 5 * plot_relevance
        + 5 * stay
        + 10
    )


def _score_object(
    co: Optional["CatalogObject"],
    scene: str,
    block: str,
    current_globals: Optional[List[int]] = None,
    catalog: Optional[Catalog] = None,
) -> int:
    """Symmetric to `_score_trigger` for interactive GOB objects.

    Objects don't usually drive block transitions, so the scale is
    lower than triggers — but a gate-satisfied examine handler
    should still outrank a transition trigger from elsewhere in the
    block. See `progress_issues.md#B4`.
    """
    if co is None:
        return 1  # live but uncatalogued
    leave = sum(
        1
        for (s, b) in co.transitions
        if s.lower() != scene.lower() or b != block
    )
    has_known_writes = 1 if any(w.value is not None for w in co.writes) else 0
    plot_relevance = sum(1 for w in co.writes if w.value is not None) + len(co.reads)
    gate_bonus = _gate_satisfaction_bonus(
        co.research_function, co.reads, current_globals, catalog, scene
    )
    if leave == 0 and plot_relevance == 0 and gate_bonus == 0:
        return 2
    return (
        2500 * gate_bonus
        + 500 * has_known_writes
        + 100 * leave
        + 5 * plot_relevance
        + 5
    )


def drift_summary(
    catalog_trigger: Optional[CatalogTrigger],
    fired_globals: Tuple[List[int], List[int]],
    actually_transitioned_to: Optional[Tuple[str, str]],
    scene: str,
    block: str,
    catalog_block: Optional["CatalogBlock"] = None,
) -> List[str]:
    """Return human-readable notes about how the observed fire
    diverged from the catalog's prediction. Empty list = "no
    drift detected"; informational only, not a hard failure.

    When `catalog_block` is passed in and the block was synthesised
    from a `transitions` reference (`CatalogBlock.synthesized` is
    True), a missing trigger is reported as "block synthesised; no
    static trigger data" rather than the generic "trigger not in
    catalog" — the two cases have very different remediation paths
    and should not be conflated. See `progress_issues.md#B6`.
    """
    notes: List[str] = []
    if catalog_trigger is None:
        if catalog_block is not None and catalog_block.synthesized:
            notes.append(
                "block synthesised from transition reference; "
                "no static trigger data"
            )
        else:
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


# ---- Goal-seek planner --------------------------------------------------


@dataclasses.dataclass
class GoalSeekStep:
    """One step taken by `GoalSeekPlanner.push_toward`. Useful for
    logs and tests; not stored in the SQLite graph itself."""

    kind: str  # "fire" | "interact"
    name: str
    settled: bool
    error: Optional[str] = None


@dataclasses.dataclass
class GoalSeekResult:
    """Outcome of one `push_toward` call. `success=True` iff
    `globals[slot]` actually moved (and matches `target_value` when
    one was set). Even on failure, `steps_taken` is non-empty when
    *something* was tried — useful for the explore loop's logging."""

    success: bool
    writer: Optional[PlotWriter]
    steps_taken: List[GoalSeekStep]
    observed_before: Optional[int]
    observed_after: Optional[int]
    notes: List[str]


class GoalSeekPlanner:
    """Plot-driven prerequisite-chain runner.

    `push_toward(slot, value)` enumerates `Catalog.plot_index[slot]`
    writers, sorts them by reachability + previous success, and for
    each writer:

    1. Saves to `save_slot` (default 99) so failed attempts can be
       rolled back.
    2. Walks `Catalog.path_to(cur, target)` firing each
       trigger / interacting with each object in turn.
    3. Re-reads `client.globals()` and compares against the
       pre-attempt snapshot. If `globals[slot]` advanced (or
       matches `value` when set), return `success=True`.
    4. Otherwise loads the save slot back and tries the next
       writer.

    Per `docs/pal4_plot_catalog.md#plot-advancement-not-set-the-flag`,
    this is the *only* sanctioned plot-advancement path — direct
    `set_global` / `script.eval` shortcuts are explicitly
    out-of-scope. The 30 % of writers that have `value: None`
    (computed RHS) are still tried when `value` is `None`; they're
    excluded when a specific `value` is being sought because we
    can't prove they'd land it.
    """

    def __init__(
        self,
        catalog: Catalog,
        graph: Graph,
        client,  # types.Client (avoid import cycle)
        save_slot: int = 99,
        max_path_hops: int = 6,
    ):
        self.catalog = catalog
        self.graph = graph
        self.client = client
        self.save_slot = save_slot
        self.max_path_hops = max_path_hops

    def push_toward(
        self,
        slot: int,
        value: Optional[int] = None,
        max_writers: int = 8,
    ) -> GoalSeekResult:
        """Try to land `globals[slot] = value` (or just "any new
        value" when `value is None`) by firing prerequisite
        triggers. Returns once a writer succeeds or all candidates
        are exhausted.

        `max_writers` caps how many candidates we'll try per call —
        the catalog can have 90+ writers for a single slot and we
        don't want one stuck explore step to grind through them all.
        """
        observed_before_arr = self.client.globals()
        observed_before = (
            int(observed_before_arr[slot])
            if slot < len(observed_before_arr)
            else None
        )

        writers = self.catalog.plot_index_for(slot, value)
        # Drop any writer that already lands the observed value —
        # if globals[slot] is already V, the writer would be a no-op.
        if value is not None and observed_before == value:
            return GoalSeekResult(
                success=True,
                writer=None,
                steps_taken=[],
                observed_before=observed_before,
                observed_after=observed_before,
                notes=[f"globals[{slot}] already == {value}; nothing to do"],
            )

        # PAL4 plot flags are monotonically non-decreasing. If the
        # caller asks us to push toward a value we've already moved
        # past, every recorded writer for that exact value would
        # require the engine to "go back" — which the trigger system
        # cannot do. Refuse early so the explore loop can pick a
        # different (forward) target instead of grinding through
        # max_writers attempts that all immediately reload.
        if (
            value is not None
            and observed_before is not None
            and observed_before > value
        ):
            return GoalSeekResult(
                success=False,
                writer=None,
                steps_taken=[],
                observed_before=observed_before,
                observed_after=observed_before,
                notes=[
                    f"globals[{slot}] = {observed_before} is already past "
                    f"target {value}; refusing to push backward "
                    "(plot flags are monotonically non-decreasing). "
                    "Caller should pick a forward target instead."
                ],
            )

        # When the caller doesn't pin a specific value (fallback
        # path from `_latest_blocking_gate`), restrict to writers
        # whose recorded value is strictly forward of the live
        # global. value=None writers (computed RHS, ~30 % of the
        # index) survive — we can't statically prove their direction.
        if value is None and observed_before is not None:
            writers = [
                w for w in writers
                if w.value is None or w.value > observed_before
            ]

        # Read current state once for path planning.
        state = self.client.state()
        cur_scene = state.get("scene", "") or ""
        cur_block = state.get("block", "") or ""

        ranked = self._rank_writers(writers, cur_scene, cur_block)
        if not ranked:
            return GoalSeekResult(
                success=False,
                writer=None,
                steps_taken=[],
                observed_before=observed_before,
                observed_after=observed_before,
                notes=[
                    f"no plot_index writers for slot={slot} value={value!r}"
                ],
            )

        notes: List[str] = []
        steps_all: List[GoalSeekStep] = []
        attempts = 0
        for writer in ranked:
            if attempts >= max_writers:
                notes.append(
                    f"stopped after {attempts} attempts (max_writers); "
                    f"{len(ranked) - attempts} writers untried"
                )
                break
            attempts += 1

            # Plan the path before saving so we can skip writers
            # we can't reach in `max_path_hops`.
            path = self.catalog.path_to(
                cur_scene,
                cur_block,
                writer.scene,
                writer.block,
                max_hops=self.max_path_hops,
            )
            if path is None:
                notes.append(
                    f"skip {_writer_label(writer)}: no path within "
                    f"{self.max_path_hops} hops from {cur_scene}/{cur_block}"
                )
                continue

            # Snapshot before this attempt.
            try:
                self.client.save(self.save_slot)
            except Exception as e:
                notes.append(f"save(slot={self.save_slot}) failed: {e}")
                return GoalSeekResult(
                    success=False,
                    writer=writer,
                    steps_taken=steps_all,
                    observed_before=observed_before,
                    observed_after=None,
                    notes=notes,
                )

            attempt_steps: List[GoalSeekStep] = []
            attempt_failed = False
            for (action, name) in path:
                step = self._run_step(action, name)
                attempt_steps.append(step)
                if not step.settled:
                    attempt_failed = True
                    break

            # Finally fire/interact the writer itself.
            if not attempt_failed:
                final_action = "interact" if writer.object_name else "fire"
                final_name = writer.object_name or writer.trigger or ""
                if final_name:
                    step = self._run_step(final_action, final_name)
                    attempt_steps.append(step)
                    if not step.settled:
                        attempt_failed = True
                else:
                    notes.append(
                        f"writer {_writer_label(writer)} has neither "
                        "trigger nor object name; cannot fire"
                    )
                    attempt_failed = True

            steps_all.extend(attempt_steps)

            after_arr = self.client.globals()
            observed_after = (
                int(after_arr[slot]) if slot < len(after_arr) else None
            )
            if not attempt_failed and observed_after != observed_before and (
                value is None or observed_after == value
            ):
                return GoalSeekResult(
                    success=True,
                    writer=writer,
                    steps_taken=steps_all,
                    observed_before=observed_before,
                    observed_after=observed_after,
                    notes=notes,
                )

            # No progress — roll back and try the next writer.
            notes.append(
                f"{_writer_label(writer)} did not advance globals[{slot}] "
                f"({observed_before} → {observed_after}); reloading"
            )
            try:
                self.client.load(self.save_slot)
            except Exception as e:
                notes.append(
                    f"load(slot={self.save_slot}) failed after "
                    f"unsuccessful attempt: {e}; bailing"
                )
                return GoalSeekResult(
                    success=False,
                    writer=writer,
                    steps_taken=steps_all,
                    observed_before=observed_before,
                    observed_after=observed_after,
                    notes=notes,
                )
            # Re-anchor cur_scene/cur_block after load — load returns
            # us to the saved state, not the dead-end we just left.
            state = self.client.state()
            cur_scene = state.get("scene", "") or cur_scene
            cur_block = state.get("block", "") or cur_block

        # All writers tried; nothing worked. Final observed value.
        final_arr = self.client.globals()
        final_observed = (
            int(final_arr[slot]) if slot < len(final_arr) else observed_before
        )
        return GoalSeekResult(
            success=False,
            writer=None,
            steps_taken=steps_all,
            observed_before=observed_before,
            observed_after=final_observed,
            notes=notes,
        )

    def _rank_writers(
        self,
        writers: List[PlotWriter],
        cur_scene: str,
        cur_block: str,
    ) -> List[PlotWriter]:
        """Sort writers by:
          (a) reachable from `(cur_scene, cur_block)` (0 hops first,
              then shortest path);
          (b) prior success rate per `graph.fires_in` — writers we've
              seen settle in the past beat untested ones;
          (c) explicit-`value` writers beat `value: None` (computed)
              writers (handled here so the planner still considers
              the latter as a last resort).
        """
        def score(w: PlotWriter) -> Tuple[int, int, int]:
            path = self.catalog.path_to(
                cur_scene, cur_block, w.scene, w.block, max_hops=self.max_path_hops
            )
            reach = -1 if path is None else len(path)
            # Lower reach (closer) is better. We want lexical
            # ascending sort, so encode unreachable as a large
            # sentinel.
            reach_key = (
                10_000 if path is None else len(path)
            )
            prior = sum(
                1
                for rec in self.graph.fires_in(w.scene, w.block)
                if rec.name == (w.trigger or f"obj:{w.object_name or ''}")
                and rec.settled
            )
            value_known = 0 if w.value is not None else 1
            return (reach_key, -prior, value_known)

        return sorted(writers, key=score)

    def _run_step(self, action: str, name: str) -> GoalSeekStep:
        try:
            if action == "fire":
                result = self.client.fire_trigger_sync(name)
                return GoalSeekStep(
                    kind="fire", name=name, settled=result.settled
                )
            if action == "interact":
                self.client.interact(name)
                # /v1/object/interact is async — poll until the
                # engine idles so the next step (or globals()
                # check) sees the dispatched handler's writes.
                # 120s accommodates the long PAL4 examine cutscenes
                # (dialog + camera + giArenaLoad) we've observed.
                settled = self.client.wait_for_idle(timeout_sec=120.0)
                return GoalSeekStep(
                    kind="interact",
                    name=name,
                    settled=settled,
                    error=None if settled else "engine did not idle within 120s",
                )
            return GoalSeekStep(
                kind=action, name=name, settled=False, error="unknown action"
            )
        except Exception as e:  # AgentError or other
            return GoalSeekStep(
                kind=action, name=name, settled=False, error=str(e)
            )


def _writer_label(w: PlotWriter) -> str:
    target = w.trigger or w.object_name or f"<{w.fn}>"
    val = "?" if w.value is None else str(w.value)
    return f"{w.scene}/{w.block}#{target}={val}"
