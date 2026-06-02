"""SQLite-backed dynamic edge graph for the PAL4 planner.

Persists every fire's outcome — observed `(reads, writes,
transitions, gate predicates)` — across runs so the planner builds an
increasingly accurate picture of the plot graph. The static catalog
becomes a *search-frontier hint*; the dynamic graph is the ground
truth for "what does this trigger actually do?".

Schema is intentionally small:

```
schema_version(version INTEGER)
fires(
    id INTEGER PRIMARY KEY,
    started_at REAL,
    scene TEXT,
    block TEXT,
    name TEXT,
    fn TEXT,
    settled INTEGER,
    waited_frames INTEGER,
    transitioned_to_scene TEXT,
    transitioned_to_block TEXT,
    globals_before_json TEXT,
    globals_after_json TEXT
)
trace_events(
    fire_id INTEGER,
    seq INTEGER,
    kind TEXT,
    payload_json TEXT,
    PRIMARY KEY (fire_id, seq)
)
gates(
    fire_id INTEGER,
    kind TEXT,            -- "global" | "sysfn"
    slot_or_sysfn TEXT,
    observed_value INTEGER,
    taken INTEGER,
    inferred_required_value INTEGER  -- NULL when unknown
)
```

The planner uses `gates` to answer "what predicate gated this fire?"
and `fires` to track block-level coverage / shortest path to a goal.
"""

from __future__ import annotations

import contextlib
import dataclasses
import json
import sqlite3
import time
from pathlib import Path
from typing import Callable, Iterable, List, Optional

from .trace import (
    Branch,
    CallSys,
    GlobalRead,
    GlobalWrite,
    TraceEvent,
    branches as iter_branches,
    callsys_calls,
    global_reads,
    global_writes,
)


SCHEMA_VERSION = 1


@dataclasses.dataclass
class FireRecord:
    id: int
    scene: str
    block: str
    name: str
    fn: str
    settled: bool
    waited_frames: int
    transitioned_to_scene: Optional[str]
    transitioned_to_block: Optional[str]


@dataclasses.dataclass
class GateRecord:
    fire_id: int
    kind: str
    slot_or_sysfn: str
    observed_value: int
    taken: bool
    inferred_required_value: Optional[int]


class Graph:
    def __init__(self, path: Path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.conn = sqlite3.connect(str(self.path))
        self.conn.execute("PRAGMA foreign_keys = ON")
        self._init_schema()

    def _init_schema(self) -> None:
        with self._transaction() as cur:
            cur.execute(
                "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)"
            )
            row = cur.execute("SELECT version FROM schema_version").fetchone()
            if row is None:
                cur.execute(
                    "INSERT INTO schema_version (version) VALUES (?)",
                    (SCHEMA_VERSION,),
                )
            elif row[0] != SCHEMA_VERSION:
                raise RuntimeError(
                    f"pal4_planner DB at {self.path} has schema version "
                    f"{row[0]}; this build expects {SCHEMA_VERSION}. "
                    "Pass --reset to drop and recreate."
                )
            cur.executescript(
                """
                CREATE TABLE IF NOT EXISTS fires (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    started_at REAL NOT NULL,
                    scene TEXT NOT NULL,
                    block TEXT NOT NULL,
                    name TEXT NOT NULL,
                    fn TEXT NOT NULL,
                    settled INTEGER NOT NULL,
                    waited_frames INTEGER NOT NULL,
                    transitioned_to_scene TEXT,
                    transitioned_to_block TEXT,
                    globals_before_json TEXT,
                    globals_after_json TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_fires_scene_block
                    ON fires(scene, block);

                CREATE TABLE IF NOT EXISTS trace_events (
                    fire_id INTEGER NOT NULL,
                    seq INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    PRIMARY KEY (fire_id, seq),
                    FOREIGN KEY (fire_id) REFERENCES fires(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS gates (
                    fire_id INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    slot_or_sysfn TEXT NOT NULL,
                    observed_value INTEGER NOT NULL,
                    taken INTEGER NOT NULL,
                    inferred_required_value INTEGER,
                    FOREIGN KEY (fire_id) REFERENCES fires(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_gates_fire ON gates(fire_id);
                CREATE INDEX IF NOT EXISTS idx_gates_kind ON gates(kind, slot_or_sysfn);
                """
            )

    @contextlib.contextmanager
    def _transaction(self):
        cur = self.conn.cursor()
        try:
            yield cur
            self.conn.commit()
        except Exception:
            self.conn.rollback()
            raise
        finally:
            cur.close()

    # ---- writes ---------------------------------------------------------

    def record_fire(
        self,
        scene: str,
        block: str,
        name: str,
        fn: str,
        settled: bool,
        waited_frames: int,
        globals_before: List[int],
        globals_after: List[int],
        transitioned_to: Optional[tuple[str, str]],
        trace: Iterable[TraceEvent],
        cmp_literal_lookup: Optional[CmpLiteralLookup] = None,
    ) -> int:
        events = list(trace)
        gates = derive_gates_from_trace(
            events, cmp_literal_lookup=cmp_literal_lookup
        )
        with self._transaction() as cur:
            cur.execute(
                """
                INSERT INTO fires (
                    started_at, scene, block, name, fn,
                    settled, waited_frames,
                    transitioned_to_scene, transitioned_to_block,
                    globals_before_json, globals_after_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    time.time(),
                    scene,
                    block,
                    name,
                    fn,
                    int(settled),
                    waited_frames,
                    transitioned_to[0] if transitioned_to else None,
                    transitioned_to[1] if transitioned_to else None,
                    json.dumps(globals_before),
                    json.dumps(globals_after),
                ),
            )
            fire_id = cur.lastrowid
            for ev in events:
                cur.execute(
                    "INSERT INTO trace_events (fire_id, seq, kind, payload_json) "
                    "VALUES (?, ?, ?, ?)",
                    (
                        fire_id,
                        ev.seq,
                        type(ev.kind).__name__,
                        json.dumps(dataclasses.asdict(ev.kind)),
                    ),
                )
            for g in gates:
                cur.execute(
                    "INSERT INTO gates (fire_id, kind, slot_or_sysfn, "
                    "observed_value, taken, inferred_required_value) "
                    "VALUES (?, ?, ?, ?, ?, ?)",
                    (
                        fire_id,
                        g.kind,
                        g.slot_or_sysfn,
                        g.observed_value,
                        int(g.taken),
                        g.inferred_required_value,
                    ),
                )
            return int(fire_id)

    def record_interact(
        self,
        scene: str,
        block: str,
        object_name: str,
        globals_before: Optional[List[int]] = None,
        globals_after: Optional[List[int]] = None,
    ) -> int:
        """Persist an object-interact step so the planner's dedup set
        (`fires_in`) reflects it. The stored `name` is prefixed with
        `obj:` to match the key `ExplorePlanner.pick` uses for
        already-fired lookup (`obj_key = f"obj:{name}"`).

        We don't have an `interact` trace cursor on the agent server
        today, so `trace=[]` here; the planner's interest in this
        record is purely dedup + provenance. Fixes `progress_issues.md#B3`.
        """
        with self._transaction() as cur:
            cur.execute(
                """
                INSERT INTO fires (
                    started_at, scene, block, name, fn,
                    settled, waited_frames,
                    transitioned_to_scene, transitioned_to_block,
                    globals_before_json, globals_after_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    time.time(),
                    scene,
                    block,
                    f"obj:{object_name}",
                    "",  # no fn — interact dispatches via research_function dynamically
                    1,
                    0,
                    None,
                    None,
                    json.dumps(globals_before) if globals_before is not None else None,
                    json.dumps(globals_after) if globals_after is not None else None,
                ),
            )
            return int(cur.lastrowid)

    # ---- reads ----------------------------------------------------------

    def last_fire(self) -> Optional[FireRecord]:
        row = self.conn.execute(
            "SELECT id, scene, block, name, fn, settled, waited_frames, "
            "transitioned_to_scene, transitioned_to_block "
            "FROM fires ORDER BY id DESC LIMIT 1"
        ).fetchone()
        if row is None:
            return None
        return FireRecord(*row[:5], bool(row[5]), int(row[6]), row[7], row[8])

    def fires_in(
        self,
        scene: str,
        block: str,
        current_globals: Optional[List[int]] = None,
    ) -> List[FireRecord]:
        """Return previously-recorded fires in `(scene, block)`.

        When `current_globals` is `None`, returns every settled fire
        (used by goal-seek's "have we ever succeeded with this
        writer?" ranking).

        When `current_globals` is provided, returns only fires that
        were **pure no-ops in this state** — i.e. fires whose
        recorded `globals_after_json` equals `current_globals` AND
        which produced no transition out of the block. Those are
        the only fires worth deduping from the planner's
        perspective: a fire that *transitioned* under the current
        globals is interesting to re-do because the transition's
        downstream effect may now differ, and a fire whose
        `globals_after` differs from the live state is by
        definition stale (the world has moved on since).
        """
        if current_globals is None:
            rows = self.conn.execute(
                "SELECT id, scene, block, name, fn, settled, waited_frames, "
                "transitioned_to_scene, transitioned_to_block "
                "FROM fires WHERE scene = ? AND block = ? ORDER BY id",
                (scene, block),
            ).fetchall()
            return [FireRecord(*r[:5], bool(r[5]), int(r[6]), r[7], r[8]) for r in rows]
        # Filter on stored globals_after. A fire dedupes if its
        # observed effect on globals matches the current live state:
        # re-firing would just reproduce the same outcome. Triggers
        # whose `globals_after` no longer matches the live state
        # are by definition stale (the world has moved on) and
        # remain fireable. Transitioning triggers are deduped
        # under their observed-state condition too — re-navigating
        # to the same destination under the same globals lands us
        # in the same place we already explored, so it's a no-op
        # from the planner's perspective.
        wanted = json.dumps(list(current_globals))
        rows = self.conn.execute(
            "SELECT id, scene, block, name, fn, settled, waited_frames, "
            "transitioned_to_scene, transitioned_to_block "
            "FROM fires WHERE scene = ? AND block = ? "
            "AND globals_after_json = ? "
            "ORDER BY id",
            (scene, block, wanted),
        ).fetchall()
        return [FireRecord(*r[:5], bool(r[5]), int(r[6]), r[7], r[8]) for r in rows]

    def trace_for_fire(self, fire_id: int) -> List[dict]:
        rows = self.conn.execute(
            "SELECT seq, kind, payload_json FROM trace_events "
            "WHERE fire_id = ? ORDER BY seq",
            (fire_id,),
        ).fetchall()
        return [
            {"seq": seq, "kind": kind, "payload": json.loads(payload)}
            for seq, kind, payload in rows
        ]

    def reset(self) -> None:
        with self._transaction() as cur:
            cur.executescript(
                "DROP TABLE IF EXISTS trace_events; "
                "DROP TABLE IF EXISTS gates; "
                "DROP TABLE IF EXISTS fires; "
                "DROP TABLE IF EXISTS schema_version;"
            )
        self._init_schema()

    def close(self) -> None:
        self.conn.close()


# ---- gate inference -----------------------------------------------------

# Callable signature: `(fn_name, pc) -> Optional[int]`. Used by
# `derive_gates_from_trace` to look up the per-fn RHS literal
# `pal4_plot_dump` recovered. The trace already carries `(fn_name, pc)`
# on every Branch event, so the planner can wire this with a closure
# over `Catalog.cmp_literal(scene, fn, pc)` once it knows the scene of
# the fire — see `drive.cmd_explore`.
CmpLiteralLookup = Callable[[str, int], Optional[int]]


def derive_gates_from_trace(
    events: List[TraceEvent],
    cmp_literal_lookup: Optional[CmpLiteralLookup] = None,
) -> List[GateRecord]:
    """Extract `(predicate, observed value, branch outcome)` triples
    from a fire's trace.

    Heuristic: a `GlobalRead` (slot, value) followed by a `Branch`
    in the same function is treated as a "global gate". Similarly,
    a `CallSys` returning into `r1_after` followed by a `Branch` is
    a "sysfn gate" — most common case is `giHasItem(N)` returning 0
    and the planner needs to learn "to satisfy this fire, item N must
    be in the inventory".

    When `cmp_literal_lookup` is provided (typically a closure over
    `Catalog.cmp_literal(scene, fn, pc)`), the function populates
    `inferred_required_value` for global gates from the static
    walker's per-`(fn, pc)` `cmp_literals` map. The trace's
    `Branch.operand` is the *comparison result* (0 / 1 for Jz/Jnz),
    not the RHS literal, so the literal must come from the static
    catalog. For sysfn gates we still record `None` — the planner
    uses `slot_or_sysfn` as the key to search for a "writer" trigger.
    """
    gates: List[GateRecord] = []
    for i, ev in enumerate(events):
        kind = ev.kind
        if isinstance(kind, GlobalRead):
            partner = _find_next_branch(events, i + 1, kind.fn_name)
            if partner is not None:
                inferred = None
                if cmp_literal_lookup is not None:
                    inferred = cmp_literal_lookup(partner.fn_name, partner.pc)
                gates.append(
                    GateRecord(
                        fire_id=-1,  # filled in by record_fire
                        kind="global",
                        slot_or_sysfn=str(kind.slot),
                        observed_value=int(kind.value),
                        taken=partner.taken,
                        inferred_required_value=inferred,
                    )
                )
        elif isinstance(kind, CallSys):
            partner = _find_next_branch(events, i + 1, kind.fn_name)
            if partner is None:
                continue
            # Use r1_after as the "return value" — for legacy gi*
            # sysfns this is where the int return lands.
            gates.append(
                GateRecord(
                    fire_id=-1,
                    kind="sysfn",
                    slot_or_sysfn=kind.sysfn_name,
                    observed_value=int(kind.r1_after),
                    taken=partner.taken,
                    inferred_required_value=None,
                )
            )
    return gates


def _find_next_branch(
    events: List[TraceEvent], start: int, fn_name: str
) -> Optional[Branch]:
    """Walk forward looking for the next `Branch` in `fn_name`.
    Returns `None` if we leave the function before finding one.
    """
    for j in range(start, len(events)):
        k = events[j].kind
        if isinstance(k, Branch) and k.fn_name == fn_name:
            return k
        from .trace import FnExit  # local to avoid circular at top
        if isinstance(k, FnExit) and k.name == fn_name:
            return None
    return None
