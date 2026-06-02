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
    reads: List[int] = dataclasses.field(default_factory=list)
    writes: List[CatalogWrite] = dataclasses.field(default_factory=list)
    transitions: List[Tuple[str, str]] = dataclasses.field(default_factory=list)

    def is_wall(self) -> bool:
        """Heuristic for collision/camera helper triggers — no
        function bound and the engine itself skips them. Resolves
        issue C4 client-side.
        """
        return not self.function and self.shape == "other"


@dataclasses.dataclass
class CatalogObject:
    name: str
    kind: str
    position: Tuple[float, float, float]
    research_function: str = ""
    reads: List[int] = dataclasses.field(default_factory=list)
    writes: List[CatalogWrite] = dataclasses.field(default_factory=list)

    def is_synthetic_name(self) -> bool:
        """`True` when the object's name is purely numeric — those
        rarely correspond to plot-pushing entities (see issue A4).
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


class Catalog:
    """Lookup-only view over `pal4_plot.json`."""

    def __init__(self, path: Path):
        self.path = Path(path)
        raw = json.loads(self.path.read_text(encoding="utf-8"))
        scenes_raw = raw.get("scenes", {})
        self.scenes: Dict[str, Dict[str, CatalogBlock]] = {}
        # First pass: ingest the explicit blocks.
        for scn_name, scn in scenes_raw.items():
            blocks: Dict[str, CatalogBlock] = {}
            for blk_name, blk in (scn.get("blocks") or {}).items():
                blocks[blk_name] = _decode_block(blk)
            self.scenes[scn_name.lower()] = blocks
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

    def block(self, scene: str, block: str) -> Optional[CatalogBlock]:
        return self.scenes.get(scene.lower(), {}).get(block)

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
    )


def _maybe_int(v) -> Optional[int]:
    if v is None:
        return None
    try:
        return int(v)
    except (TypeError, ValueError):
        return None
