"""Decoded VM execution trace events.

Mirrors the agent-server `TraceEventKindPayload` enum. The decoder
takes the JSON shape from `/v1/script/trace/drain` and projects each
event into a typed dataclass; consumers (the planner) walk a
`list[TraceEvent]` instead of poking at dicts.
"""

from __future__ import annotations

import dataclasses
from typing import Iterable, Iterator, List, Optional


# ---- event payloads -----------------------------------------------------

@dataclasses.dataclass
class FnEnter:
    name: str
    function_index: int
    depth: int


@dataclasses.dataclass
class FnExit:
    name: str
    depth: int


@dataclasses.dataclass
class Branch:
    """Conditional jump. `taken` is the runtime outcome.

    `branch` is one of `"jz"`, `"jnz"`, `"js_jgez"`, `"jns_jlz"`,
    `"jp_jlez"`, `"jnp_jgz"`. See the VM-side enum in
    `yaobow/shared/src/scripting/angelscript/trace.rs` for the
    semantics of each.
    """

    fn_name: str
    pc: int
    branch: str
    operand: int
    offset: int
    taken: bool


@dataclasses.dataclass
class CallSys:
    fn_name: str
    pc: int
    sysfn_index: int
    sysfn_name: str
    sp_before: int
    sp_after: int
    r1_after: int


@dataclasses.dataclass
class GlobalRead:
    fn_name: str
    pc: int
    scope: str  # "shared" | "module"
    slot: int
    value: int


@dataclasses.dataclass
class GlobalWrite:
    fn_name: str
    pc: int
    scope: str
    slot: int
    value: int


@dataclasses.dataclass
class Suspend:
    fn_name: str
    pc: int


# Discriminated-union shorthand.
TraceKind = (
    FnEnter
    | FnExit
    | Branch
    | CallSys
    | GlobalRead
    | GlobalWrite
    | Suspend
)


@dataclasses.dataclass
class TraceEvent:
    seq: int
    kind: TraceKind


# ---- decoder ------------------------------------------------------------

def decode_event(raw: dict) -> TraceEvent:
    """Project one JSON event into a typed `TraceEvent`."""
    seq = int(raw["seq"])
    k = raw["kind"]
    t = k["type"]
    if t == "fn_enter":
        kind = FnEnter(
            name=k["name"],
            function_index=int(k["function_index"]),
            depth=int(k["depth"]),
        )
    elif t == "fn_exit":
        kind = FnExit(name=k["name"], depth=int(k["depth"]))
    elif t == "branch":
        kind = Branch(
            fn_name=k["fn_name"],
            pc=int(k["pc"]),
            branch=k["branch"],
            operand=int(k["operand"]),
            offset=int(k["offset"]),
            taken=bool(k["taken"]),
        )
    elif t == "call_sys":
        kind = CallSys(
            fn_name=k["fn_name"],
            pc=int(k["pc"]),
            sysfn_index=int(k["sysfn_index"]),
            sysfn_name=k["sysfn_name"],
            sp_before=int(k["sp_before"]),
            sp_after=int(k["sp_after"]),
            r1_after=int(k["r1_after"]),
        )
    elif t == "global_read":
        kind = GlobalRead(
            fn_name=k["fn_name"],
            pc=int(k["pc"]),
            scope=k["scope"],
            slot=int(k["slot"]),
            value=int(k["value"]),
        )
    elif t == "global_write":
        kind = GlobalWrite(
            fn_name=k["fn_name"],
            pc=int(k["pc"]),
            scope=k["scope"],
            slot=int(k["slot"]),
            value=int(k["value"]),
        )
    elif t == "suspend":
        kind = Suspend(fn_name=k["fn_name"], pc=int(k["pc"]))
    else:
        raise ValueError(f"unknown trace event type: {t!r}")
    return TraceEvent(seq=seq, kind=kind)


def decode_all(events: Iterable[dict]) -> List[TraceEvent]:
    return [decode_event(e) for e in events]


# ---- helpers for the planner --------------------------------------------

def callsys_calls(trace: Iterable[TraceEvent]) -> Iterator[CallSys]:
    """Iterate just the `CallSys` events in trace order."""
    for ev in trace:
        if isinstance(ev.kind, CallSys):
            yield ev.kind


def global_writes(trace: Iterable[TraceEvent]) -> Iterator[GlobalWrite]:
    for ev in trace:
        if isinstance(ev.kind, GlobalWrite):
            yield ev.kind


def global_reads(trace: Iterable[TraceEvent]) -> Iterator[GlobalRead]:
    for ev in trace:
        if isinstance(ev.kind, GlobalRead):
            yield ev.kind


def branches(trace: Iterable[TraceEvent]) -> Iterator[Branch]:
    for ev in trace:
        if isinstance(ev.kind, Branch):
            yield ev.kind


def first_call_returning(
    trace: Iterable[TraceEvent], sysfn_name: str
) -> Optional[CallSys]:
    """Locate the first `CallSys(sysfn_name)` event. Useful for
    spot-checking gates like `giHasItem(N)` in a fire's trace.
    """
    for c in callsys_calls(trace):
        if c.sysfn_name == sysfn_name:
            return c
    return None


def gating_callsys(
    trace: List[TraceEvent],
    sysfn_name: str,
) -> List[tuple[CallSys, Optional[Branch]]]:
    """Find every `CallSys(sysfn_name)` paired with the next branch
    in the same function.

    The pairing heuristic is: walk forward from the CallSys, return
    the first `Branch` event whose `fn_name` matches and whose `pc`
    is strictly greater. This identifies the "gate predicate" that
    consumed the sysfn's return value — useful for "this trigger
    failed because `giHasItem(202)` returned 0 and the jz didn't
    take" diagnoses.
    """
    out: List[tuple[CallSys, Optional[Branch]]] = []
    for i, ev in enumerate(trace):
        if not isinstance(ev.kind, CallSys):
            continue
        if ev.kind.sysfn_name != sysfn_name:
            continue
        partner: Optional[Branch] = None
        for j in range(i + 1, len(trace)):
            cand = trace[j].kind
            if isinstance(cand, Branch) and cand.fn_name == ev.kind.fn_name:
                partner = cand
                break
            # If we leave the function, give up.
            if isinstance(cand, FnExit) and cand.name == ev.kind.fn_name:
                break
        out.append((ev.kind, partner))
    return out
