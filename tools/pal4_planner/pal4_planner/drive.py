"""CLI front-end for the PAL4 planner.

```
python -m pal4_planner.drive explore [--catalog ...] [--db ...] [--steps N]
python -m pal4_planner.drive inspect [--db ...] [--fire-id N]
python -m pal4_planner.drive replay --fire-id N [--db ...]
```
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import Optional

from .catalog import Catalog
from .client import AgentError, Client, FireResult
from .graph import Graph
from .planner import ExplorePlanner, drift_summary
from .trace import TraceEvent, decode_all


def default_db_path() -> Path:
    base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(base) / "pal4_planner.sqlite"


def default_catalog_path() -> Path:
    here = Path(__file__).resolve()
    repo_root = here.parents[3]
    return repo_root / "generated" / "pal4_plot.json"


def _build_common(args: argparse.Namespace) -> tuple[Catalog, Graph, Client]:
    catalog_path = Path(args.catalog or default_catalog_path())
    db_path = Path(args.db or default_db_path())
    catalog = Catalog(catalog_path)
    if args.reset and db_path.exists():
        db_path.unlink()
    graph = Graph(db_path)
    client = Client(base_url=args.base_url, token=args.token)
    return catalog, graph, client


def cmd_explore(args: argparse.Namespace) -> int:
    catalog, graph, client = _build_common(args)
    client.fast_forward(True)
    client.trace_start(reset=True)
    planner = ExplorePlanner(catalog, graph)

    for step in range(args.steps):
        state = client.state()
        scene, block = state.get("scene", ""), state.get("block", "")
        if not scene or not block:
            print(f"[step {step}] no scene loaded; aborting")
            break
        live_triggers = client.scene_triggers()
        live_objects = client.scene_objects().get("objects", [])
        cand = planner.pick(scene, block, live_triggers, live_objects)
        if cand is None:
            print(f"[step {step}] {scene}/{block}: nothing actionable; stopping")
            break
        print(
            f"[step {step}] {scene}/{block} -> fire {cand.kind} "
            f"{cand.name!r} (score={cand.score})"
        )

        if cand.kind == "object":
            try:
                client.interact(cand.name)
            except AgentError as e:
                print(f"  interact failed: {e}")
                continue
            time.sleep(0.2)
            continue

        globals_before = client.globals()
        try:
            result = client.fire_trigger_sync(cand.name, timeout_ms=args.fire_timeout_ms)
        except AgentError as e:
            print(f"  fire_trigger failed: {e}")
            continue

        new_state = client.state()
        transitioned_to: Optional[tuple[str, str]] = None
        if (
            new_state.get("scene") != scene
            or new_state.get("block") != block
        ):
            transitioned_to = (new_state.get("scene", ""), new_state.get("block", ""))
        globals_after = client.globals()

        # Drain just this fire's trace events.
        events: list[TraceEvent] = []
        if result.trace_seq_start is not None and result.trace_seq_end is not None:
            cursor = result.trace_seq_start - 1 if result.trace_seq_start > 0 else 0
            page = client.trace_drain(cursor, 8192)
            events = decode_all(page.get("events", []))

        fn = (cand.catalog_trigger.function if cand.catalog_trigger else "") or ""
        fire_id = graph.record_fire(
            scene=scene,
            block=block,
            name=cand.name,
            fn=fn,
            settled=result.settled,
            waited_frames=result.waited_frames,
            globals_before=globals_before,
            globals_after=globals_after,
            transitioned_to=transitioned_to,
            trace=events,
        )
        drift = drift_summary(
            cand.catalog_trigger,
            (globals_before, globals_after),
            transitioned_to,
            scene,
            block,
        )
        print(
            f"  fire_id={fire_id} settled={result.settled} "
            f"waited_frames={result.waited_frames} "
            f"transition={transitioned_to} drift_notes={drift}"
        )

    return 0


def cmd_inspect(args: argparse.Namespace) -> int:
    _, graph, _ = _build_common(args)
    fire_id = args.fire_id
    if fire_id is None:
        rec = graph.last_fire()
        if rec is None:
            print("no fires recorded yet")
            return 1
        fire_id = rec.id
    events = graph.trace_for_fire(fire_id)
    print(json.dumps({"fire_id": fire_id, "events": events}, indent=2))
    return 0


def cmd_replay(args: argparse.Namespace) -> int:
    _, graph, _ = _build_common(args)
    if args.fire_id is None:
        print("--fire-id is required for replay")
        return 2
    events = graph.trace_for_fire(args.fire_id)
    print(f"fire {args.fire_id}: {len(events)} events")
    for ev in events:
        print(f"  seq={ev['seq']} kind={ev['kind']} {ev['payload']}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="pal4_planner",
        description="Concolic-style PAL4 plot driver.",
    )
    p.add_argument(
        "--base-url",
        default="http://127.0.0.1:8765",
        help="agent server base URL (default: %(default)s)",
    )
    p.add_argument("--token", default=None, help="bearer token (if the server requires one)")
    p.add_argument("--db", default=None, help="SQLite DB path (default: ~/.cache/pal4_planner.sqlite)")
    p.add_argument(
        "--catalog",
        default=None,
        help="path to pal4_plot.json (default: <repo>/generated/pal4_plot.json)",
    )
    p.add_argument(
        "--reset",
        action="store_true",
        help="delete the DB before starting",
    )

    sub = p.add_subparsers(dest="cmd", required=True)
    explore = sub.add_parser("explore", help="fire triggers and build the graph DB")
    explore.add_argument("--steps", type=int, default=80)
    explore.add_argument(
        "--fire-timeout-ms",
        type=int,
        default=10_000,
        help="server-side fire-and-wait timeout (ms)",
    )
    explore.set_defaults(func=cmd_explore)

    inspect = sub.add_parser("inspect", help="dump the trace for a fire")
    inspect.add_argument("--fire-id", type=int, default=None)
    inspect.set_defaults(func=cmd_inspect)

    replay = sub.add_parser("replay", help="dump a previously recorded fire")
    replay.add_argument("--fire-id", type=int, required=True)
    replay.set_defaults(func=cmd_replay)

    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
