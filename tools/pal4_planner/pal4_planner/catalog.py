"""Static plot catalog (`generated/pal4_plot.json`) reader.

The on-disk catalog is built by `tools/pal4_plot_dump` from PAL4's
`gamedata/script/*.csb` modules. This module wraps it with the
lookups the planner needs and synthesises stub block entries for any
`(scene, block)` referenced via `transitions` but missing from the
top-level `blocks` map (issue C1 in `generated/issues.md`).
"""

from __future__ import annotations

import dataclasses
import json
from pathlib import Path
from typing import Dict, List, Optional, Tuple


@dataclasses.dataclass(frozen=True)
class PlotWriter:
    """One row of the catalog's `plot_index[slot]` reverse index:
    "to land `globals[slot] = value`, fire one of these things".

    - `scene` / `block` always set.
    - Exactly one of `trigger` / `object` is non-`None` (the entry
      seeded the index from a trigger fn closure or an object's
      `research_function`, respectively). When **both** are `None`
      the writer is an init / scripted-cutscene fn with no
      direct EVF/GOB entry — the planner has to chase the
      transitions graph to reach it. Such entries are rare.
    - `value` is `None` when the static walker couldn't recover
      the literal V (computed RHS); the writer is still listed
      because firing it changes `globals[slot]` *somehow*.
    """

    slot: int
    value: Optional[int]
    scene: str
    block: str
    trigger: Optional[str]
    object_name: Optional[str]
    fn: str


@dataclasses.dataclass
class CatalogWrite:
    """One `{global, value}` write recorded by the static walker."""

    global_slot: int
    value: Optional[int]


@dataclasses.dataclass
class CatalogTrigger:
    name: str
    function: str
    center: Tuple[float, float, float]
    half_size: Tuple[float, float, float]
    shape: str
    # `kind` is the explicit tag emitted by `pal4_plot_dump`:
    # `"trigger"` for plot-meaningful event volumes, `"wall"` for
    # collision / camera helper volumes the live engine itself
    # skips. Empty string when missing (older catalog or
    # synthesised block) — callers should fall back to
    # `function == ""` in that case.
    kind: str = ""
    reads: List[int] = dataclasses.field(default_factory=list)
    writes: List[CatalogWrite] = dataclasses.field(default_factory=list)
    transitions: List[Tuple[str, str]] = dataclasses.field(default_factory=list)

    def is_wall(self) -> bool:
        """`True` when the catalog tagged this trigger as a wall /
        camera helper volume (or, when `kind` is missing on an older
        catalog, when there's no bound function — the legacy
        heuristic). The live engine skips these regardless."""
        if self.kind:
            return self.kind == "wall"
        return not self.function


@dataclasses.dataclass
class CatalogObject:
    name: str
    kind: str
    position: Tuple[float, float, float]
    research_function: str = ""
    reads: List[int] = dataclasses.field(default_factory=list)
    writes: List[CatalogWrite] = dataclasses.field(default_factory=list)
    transitions: List[Tuple[str, str]] = dataclasses.field(default_factory=list)

    def is_synthetic_name(self) -> bool:
        """`True` when the object's name is purely numeric.

        Note: PAL4 routinely uses numeric names for plot-pushing GOB
        entries (the "examine this barrel" / "open this chest" kind),
        so the planner does **not** skip on this flag — it's left
        here only for callers that genuinely want to filter
        decorative numerics out of analyses. See `progress_issues.md`
        issue B4 for the regression that motivated the change.
        """
        return self.name.isdigit()


@dataclasses.dataclass
class CatalogBlock:
    triggers: List[CatalogTrigger] = dataclasses.field(default_factory=list)
    objects: List[CatalogObject] = dataclasses.field(default_factory=list)
    synthesized: bool = False
    """`True` when this block was synthesised from a `transitions`
    reference instead of read from `blocks` directly — its triggers
    / objects lists are empty and the planner should fall back to
    `/v1/scene/triggers` live data.
    """


class StaleCatalogError(RuntimeError):
    """Raised by `Catalog.__init__` when the loaded JSON predates the
    per-trigger `TriggerSummary` flattening in `tools/pal4_plot_dump`.

    A stale catalog parses cleanly but every trigger gets an empty
    `(reads, writes, transitions)` tuple, which silently degrades the
    planner's scoring to "always 10" and disables drift detection.
    Better to fail loudly with a regeneration hint than to run with
    bogus data — see `progress_issues.md#A2`.
    """


class Catalog:
    """Lookup-only view over `pal4_plot.json`."""

    def __init__(self, path: Path):
        self.path = Path(path)
        raw = json.loads(self.path.read_text(encoding="utf-8"))
        _check_catalog_schema(raw, self.path)
        scenes_raw = raw.get("scenes", {})
        self.scenes: Dict[str, Dict[str, CatalogBlock]] = {}
        # Per-scene fn map: scene (lower) -> fn name -> {"cmp_literals": {pc: V|None}, ...}.
        # `pal4_plot.json` carries this directly under `scenes.<scn>.fns.<fn>`,
        # added in catalog schema v2 for the planner's gate-inference
        # path. We only mirror the fields the planner reads; extending
        # this dict is cheap.
        self.fns: Dict[str, Dict[str, Dict[str, Dict[int, Optional[int]]]]] = {}
        # `plot_index[slot]` is the catalog's reverse index — see
        # `docs/pal4_plot_catalog.md` and the dataclass docstring.
        # Keyed by integer slot for ergonomic access from the planner.
        self.plot_index: Dict[int, List[PlotWriter]] = {}
        for scn_name, scn in scenes_raw.items():
            scn_key = scn_name.lower()
            blocks: Dict[str, CatalogBlock] = {}
            for blk_name, blk in (scn.get("blocks") or {}).items():
                blocks[blk_name] = _decode_block(blk)
            self.scenes[scn_key] = blocks
            fns_raw = scn.get("fns") or {}
            scn_fns: Dict[str, Dict[str, Dict[int, Optional[int]]]] = {}
            for fn_name, fn_body in fns_raw.items():
                cmp_lits_raw = (fn_body or {}).get("cmp_literals") or {}
                cmp_lits: Dict[int, Optional[int]] = {}
                for pc_str, v in cmp_lits_raw.items():
                    try:
                        cmp_lits[int(pc_str)] = (
                            int(v) if v is not None else None
                        )
                    except (TypeError, ValueError):
                        continue
                # Carry the static walker's per-fn sysfn list through
                # so the planner can detect functions that prompt the
                # user for a world-map destination (`giShowWorldMap`)
                # and pre-buffer the choice before firing the trigger.
                sysfns = list((fn_body or {}).get("sysfns") or [])
                scn_fns[fn_name] = {
                    "cmp_literals": cmp_lits,
                    "sysfns": sysfns,
                }
            self.fns[scn_key] = scn_fns
        # Second pass: synthesise stub blocks for transitions that
        # reference (scene, block) pairs we don't have entries for.
        for scn_name, blocks in self.scenes.items():
            extra: Dict[str, CatalogBlock] = {}
            for blk in blocks.values():
                for trig in blk.triggers:
                    for dest in trig.transitions:
                        ds, db = dest[0].lower(), dest[1]
                        if ds not in self.scenes:
                            self.scenes.setdefault(ds, {})
                        target_scene = self.scenes[ds]
                        if db not in target_scene and (ds != scn_name or db not in extra):
                            extra.setdefault(db, CatalogBlock(synthesized=True))
            # Apply same-scene synthesised entries.
            for db, blk in extra.items():
                blocks.setdefault(db, blk)
        # Third pass: cross-scene synth (after first pass settled).
        for scn_name, blocks in list(self.scenes.items()):
            for blk in list(blocks.values()):
                for trig in blk.triggers:
                    for dest in trig.transitions:
                        ds, db = dest[0].lower(), dest[1]
                        target = self.scenes.setdefault(ds, {})
                        target.setdefault(db, CatalogBlock(synthesized=True))

        # Fourth pass: parse `plot_index` (catalog schema v1 and up).
        # Keys are stringified slot indices in JSON; ergonomic to
        # expose as int slots to the planner.
        for slot_str, entries in (raw.get("plot_index") or {}).items():
            try:
                slot = int(slot_str)
            except (TypeError, ValueError):
                continue
            decoded: List[PlotWriter] = []
            for e in entries or []:
                if not isinstance(e, dict):
                    continue
                decoded.append(
                    PlotWriter(
                        slot=slot,
                        value=_maybe_int(e.get("value")),
                        scene=str(e.get("scene", "")),
                        block=str(e.get("block", "")),
                        trigger=(e.get("trigger") if e.get("trigger") else None),
                        object_name=(e.get("object") if e.get("object") else None),
                        fn=str(e.get("fn", "")),
                    )
                )
            if decoded:
                self.plot_index[slot] = decoded

    def block(self, scene: str, block: str) -> Optional[CatalogBlock]:
        return self.scenes.get(scene.lower(), {}).get(block)

    def plot_index_for(
        self,
        slot: int,
        value: Optional[int] = None,
    ) -> List[PlotWriter]:
        """Return all catalog writers that mutate `globals[slot]`,
        optionally filtered to a specific `value`. Empty list when
        the slot has no recorded writers.

        Used by `GoalSeekPlanner` to enumerate prerequisite-fire
        candidates when a gated trigger no-ops because
        `globals[slot] != V_required` — see
        `docs/pal4_plot_catalog.md#plot-advancement-not-set-the-flag`.

        Filtering on `value` is best-effort: writers whose
        `value` is `None` (computed RHS) are excluded when `value`
        is set, because we can't prove they'd land the target.
        """
        rows = self.plot_index.get(int(slot), [])
        if value is None:
            return list(rows)
        return [r for r in rows if r.value == value]

    def path_to(
        self,
        cur_scene: str,
        cur_block: str,
        target_scene: str,
        target_block: str,
        max_hops: int = 6,
    ) -> Optional[List[Tuple[str, str]]]:
        """BFS over the catalog transition graph for a path from
        `(cur_scene, cur_block)` to `(target_scene, target_block)`.
        Returns a list of `(action_kind, trigger_or_object_name)`
        steps, each fireable from the block reached so far. Returns
        `None` when no path is found within `max_hops`; returns `[]`
        when already at the target (caller fires the writer
        directly).

        Used by `GoalSeekPlanner` to walk to a remote writer
        without resorting to a `set_global`/`script.eval` shortcut.
        See `docs/pal4_plot_catalog.md` step 5 ("chase a transition
        path").

        Action kinds: `"fire"` (call `client.fire_trigger_sync`),
        `"interact"` (call `client.interact`). Edges are recorded as
        the *trigger* `name`s (or object names) the catalog walker
        attributed each transition to; entries with no source
        trigger (synthesised blocks) contribute no edges.
        """
        cur = (cur_scene.lower(), cur_block)
        target = (target_scene.lower(), target_block)
        if cur == target:
            return []
        # BFS frontier; each entry holds the path of steps taken so
        # far (we record steps not nodes so the final result already
        # has trigger names).
        from collections import deque

        seen: set[Tuple[str, str]] = {cur}
        queue: deque[Tuple[Tuple[str, str], List[Tuple[str, str]]]] = deque(
            [(cur, [])]
        )
        while queue:
            (scn, blk), path = queue.popleft()
            if len(path) >= max_hops:
                continue
            block_obj = self.scenes.get(scn, {}).get(blk)
            if block_obj is None:
                continue
            # Trigger-driven edges.
            for trig in block_obj.triggers:
                if not trig.function or trig.is_wall():
                    continue
                for (ds, db) in trig.transitions:
                    next_node = (ds.lower(), db)
                    if next_node in seen:
                        continue
                    next_path = path + [("fire", trig.name)]
                    if next_node == target:
                        return next_path
                    seen.add(next_node)
                    queue.append((next_node, next_path))
            # Object-driven edges.
            for obj in block_obj.objects:
                if not obj.research_function:
                    continue
                for (ds, db) in obj.transitions:
                    next_node = (ds.lower(), db)
                    if next_node in seen:
                        continue
                    next_path = path + [("interact", obj.name)]
                    if next_node == target:
                        return next_path
                    seen.add(next_node)
                    queue.append((next_node, next_path))
        return None

    def cmp_literal(
        self, scene: str, fn: str, pc: int
    ) -> Optional[int]:
        """Return the `g[slot] cmp V` RHS literal the static walker
        recovered for the conditional jump at `(scene.fns[fn], pc)`,
        or `None` either when (a) no gate predicate was recognised at
        that PC, or (b) the predicate was recognised but the RHS was
        computed instead of literal. Used by
        `graph.derive_gates_from_trace` to populate
        `gates.inferred_required_value`. See
        `docs/pal4_plot_catalog.md` schema v2.

        Callers should treat "no entry" and "entry with value `None`"
        as the same outcome for ranking purposes — both mean "no
        usable required value is known".
        """
        scn_fns = self.fns.get(scene.lower())
        if scn_fns is None:
            return None
        fn_body = scn_fns.get(fn)
        if fn_body is None:
            return None
        return fn_body.get("cmp_literals", {}).get(int(pc))

    def fn_calls_sysfn(self, scene: str, fn: str, sysfn: str) -> bool:
        """`True` when the static walker recorded `sysfn` in the
        catalog's per-function sysfn list for `(scene, fn)`. Used to
        detect functions that block on a user prompt the planner
        knows how to satisfy (e.g. `giShowWorldMap`)."""
        scn_fns = self.fns.get(scene.lower())
        if scn_fns is None:
            return False
        fn_body = scn_fns.get(fn)
        if fn_body is None:
            return False
        return sysfn in (fn_body.get("sysfns") or [])

    def trigger(
        self, scene: str, block: str, name: str
    ) -> Optional[CatalogTrigger]:
        blk = self.block(scene, block)
        if blk is None:
            return None
        for t in blk.triggers:
            if t.name == name:
                return t
        return None

    def transitions_from(
        self, scene: str, block: str
    ) -> List[Tuple[str, str, str]]:
        """All recorded `(target_scene, target_block, via_trigger)`
        transitions out of this block. Used by the planner's
        BFS-style "next unvisited block" lookup.
        """
        blk = self.block(scene, block)
        out: List[Tuple[str, str, str]] = []
        if blk is None:
            return out
        for t in blk.triggers:
            for dest in t.transitions:
                out.append((dest[0], dest[1], t.name))
        return out


def _decode_block(raw: dict) -> CatalogBlock:
    return CatalogBlock(
        triggers=[_decode_trigger(t) for t in (raw.get("triggers") or [])],
        objects=[_decode_object(o) for o in (raw.get("objects") or [])],
    )


def _decode_trigger(raw: dict) -> CatalogTrigger:
    writes = []
    for w in raw.get("writes") or []:
        if isinstance(w, dict):
            writes.append(
                CatalogWrite(
                    global_slot=int(w.get("global", 0)),
                    value=_maybe_int(w.get("value")),
                )
            )
    return CatalogTrigger(
        name=raw.get("name", ""),
        function=raw.get("function", ""),
        center=tuple(raw.get("center", [0, 0, 0]))[:3],
        half_size=tuple(raw.get("half_size", [0, 0, 0]))[:3],
        shape=raw.get("shape", ""),
        kind=raw.get("kind", "") or "",
        reads=[int(x) for x in (raw.get("reads") or [])],
        writes=writes,
        transitions=[
            tuple(x)[:2]  # type: ignore[misc]
            for x in (raw.get("transitions") or [])
            if isinstance(x, (list, tuple)) and len(x) >= 2
        ],
    )


def _decode_object(raw: dict) -> CatalogObject:
    writes = []
    for w in raw.get("writes") or []:
        if isinstance(w, dict):
            writes.append(
                CatalogWrite(
                    global_slot=int(w.get("global", 0)),
                    value=_maybe_int(w.get("value")),
                )
            )
    return CatalogObject(
        name=raw.get("name", ""),
        kind=raw.get("kind", ""),
        position=tuple(raw.get("position", [0, 0, 0]))[:3],
        research_function=raw.get("research_function", "") or "",
        reads=[int(x) for x in (raw.get("reads") or [])],
        writes=writes,
        transitions=[
            tuple(x)[:2]  # type: ignore[misc]
            for x in (raw.get("transitions") or [])
            if isinstance(x, (list, tuple)) and len(x) >= 2
        ],
    )


def _maybe_int(v) -> Optional[int]:
    if v is None:
        return None
    try:
        return int(v)
    except (TypeError, ValueError):
        return None


# Keys that `tools/pal4_plot_dump` flattens onto every trigger /
# object via `#[serde(flatten)] TriggerSummary`. A catalog that lacks
# any of them on every trigger was produced by an older dumper and
# the planner can't use it; see `StaleCatalogError`.
_REQUIRED_TRIGGER_KEYS = ("kind", "transitions", "writes", "reads")


def _check_catalog_schema(raw: dict, path: Path) -> None:
    """Sanity-check the loaded catalog. Raises `StaleCatalogError`
    when no trigger in any block carries the flattened summary keys
    that `pal4_plot_dump` now emits — that's the signature of a
    catalog produced before the `#[serde(flatten)] TriggerSummary`
    change, and the planner's scoring depends on those fields.

    An empty catalog (no scenes / all blocks empty) passes — there is
    nothing to score and the loader is being asked to no-op.
    """
    scenes = raw.get("scenes") or {}
    seen_any_trigger = False
    for scn in scenes.values():
        for blk in (scn.get("blocks") or {}).values():
            for trig in blk.get("triggers") or []:
                seen_any_trigger = True
                if all(k in trig for k in _REQUIRED_TRIGGER_KEYS):
                    return  # at least one well-formed trigger; OK.
    if not seen_any_trigger:
        return
    raise StaleCatalogError(
        f"{path}: catalog has triggers but none carry the flattened "
        f"{_REQUIRED_TRIGGER_KEYS} keys produced by the current "
        "pal4_plot_dump. The planner's scoring would silently "
        "degrade to a constant. Regenerate with:\n"
        "  cargo run -p pal4_plot_dump -- --root <PAL4 root> "
        f"--out {path}"
    )
