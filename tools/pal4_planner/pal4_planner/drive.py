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
from .planner import ExplorePlanner, GoalSeekPlanner, drift_summary
from .trace import TraceEvent, decode_all


def default_db_path() -> Path:
    base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(base) / "pal4_planner.sqlite"


def default_catalog_path() -> Path:
    here = Path(__file__).resolve()
    repo_root = here.parents[3]
    return repo_root / "generated" / "pal4_plot.json"


def _wait_for_idle_scene(client: Client, timeout_sec: float) -> bool:
    """Poll `/v1/state` until both `(scene, block)` are non-empty AND
    `script_running == False` AND `movie_playing == False`, or until
    `timeout_sec` elapses. The intro movie + `<Scene>_<Block>_init`
    script typically takes ~10–15 s after a cold `--pal4` launch,
    even with `fast_forward=True`. Returns `True` if the engine
    settled in time.
    """
    deadline = time.monotonic() + max(0.0, timeout_sec)
    poll_interval = 0.5
    last_log = 0.0
    while True:
        try:
            state = client.state()
        except AgentError as e:
            # Agent server not up yet — keep polling until deadline.
            if time.monotonic() >= deadline:
                print(f"[boot] agent server unreachable: {e}")
                return False
            time.sleep(poll_interval)
            continue
        scene = state.get("scene") or ""
        block = state.get("block") or ""
        script_running = bool(state.get("script_running", False))
        movie_playing = bool(state.get("movie_playing", False))
        if scene and block and not script_running and not movie_playing:
            return True
        now = time.monotonic()
        if now - last_log >= 2.0:
            print(
                f"[boot] waiting: scene={scene!r} block={block!r} "
                f"script_running={script_running} movie_playing={movie_playing}"
            )
            last_log = now
        if now >= deadline:
            return False
        time.sleep(poll_interval)


def _build_common(args: argparse.Namespace) -> tuple[Catalog, Graph, Client]:
    catalog_path = Path(args.catalog or default_catalog_path())
    db_path = Path(args.db or default_db_path())
    catalog = Catalog(catalog_path)
    if args.reset and db_path.exists():
        db_path.unlink()
    graph = Graph(db_path)
    # urllib timeout must comfortably exceed the server's reply
    # timeout — the server returns its own HTTP 500 on its
    # `reply_timeout` (default 5 s, configurable up to 60 s via
    # `--agent-reply-timeout-secs`); we never want urllib to time
    # out first because the recovery path in
    # `Client.fire_trigger_sync` keys off the server's structured
    # error response.
    http_timeout = max(getattr(args, "http_timeout_sec", 0.0), 90.0)
    client = Client(base_url=args.base_url, token=args.token, timeout=http_timeout)
    return catalog, graph, client


def _latest_blocking_gate(
    graph: Graph,
    scene: str,
    block: str,
    catalog: Optional[Catalog] = None,
    current_globals: Optional[List[int]] = None,
) -> Optional[tuple[int, Optional[int]]]:
    """Inspect the most recent fires in `(scene, block)` and return
    the `(slot, required_value)` of the freshest global gate that
    looks blocking AND has at least one writer the planner can
    actually fire.

    "Blocking" here means a gate whose
    `inferred_required_value` (populated by
    `graph.derive_gates_from_trace` from
    `Catalog.cmp_literal`) differs from the observed value AND
    differs from the *current* live globals (so a previous goal-seek
    win doesn't keep re-selecting the same satisfied gate). When
    `catalog` is provided we additionally filter to gates where
    `Catalog.plot_index_for(slot, value)` is non-empty — otherwise
    the planner would chase an unreachable target.

    When no value-specific gate has writers, fall back to
    "any global slot the recent fires read" and return
    `(slot, None)` so `GoalSeekPlanner.push_toward` enumerates all
    writers for that slot regardless of value (filtered to those
    with `value` known so we can prove the write lands).

    Returns `None` when no usable gate is recorded — the caller
    should then stop, since there's no candidate writer to chase.
    """
    rows = graph.conn.execute(
        "SELECT g.slot_or_sysfn, g.observed_value, g.taken, "
        "       g.inferred_required_value "
        "FROM gates g JOIN fires f ON g.fire_id = f.id "
        "WHERE f.scene = ? AND f.block = ? AND g.kind = 'global' "
        "ORDER BY f.id DESC, g.rowid DESC LIMIT 64",
        (scene, block),
    ).fetchall()

    def already_satisfied(slot: int, req: int) -> bool:
        if current_globals is None or slot >= len(current_globals):
            return False
        return current_globals[slot] == req

    # Prefer (slot, value) gates with at least one fireable writer
    # that we haven't already satisfied.
    for slot_str, observed, _taken, required in rows:
        if required is None or required == observed:
            continue
        try:
            slot = int(slot_str)
            req = int(required)
        except (TypeError, ValueError):
            continue
        if already_satisfied(slot, req):
            continue
        if catalog is None or catalog.plot_index_for(slot, req):
            return (slot, req)
    # Fall back to any slot with at least one writer in the index.
    for slot_str, _observed, _taken, _required in rows:
        try:
            slot = int(slot_str)
        except (TypeError, ValueError):
            continue
        if catalog is None or catalog.plot_index_for(slot):
            return (slot, None)
    return None


def cmd_explore(args: argparse.Namespace) -> int:
    catalog, graph, client = _build_common(args)
    client.fast_forward(True)
    client.trace_start(reset=True)
    planner = ExplorePlanner(catalog, graph)
    goal_seek = GoalSeekPlanner(
        catalog,
        graph,
        client,
        save_slot=args.save_slot,
        max_path_hops=args.max_path_hops,
    )

    if not _wait_for_idle_scene(client, args.boot_timeout_sec):
        print(
            f"[boot] timed out after {args.boot_timeout_sec}s waiting for "
            "the first idle scene; aborting (use --boot-timeout-sec to raise)"
        )
        return 1

    for step in range(args.steps):
        state = client.state()
        scene, block = state.get("scene", ""), state.get("block", "")
        if not scene or not block:
            print(f"[step {step}] no scene loaded; aborting")
            break
        live_triggers = client.scene_triggers()
        live_objects = client.scene_objects().get("objects", [])
        cur_globals = client.globals()
        cand = planner.pick(
            scene, block, live_triggers, live_objects, current_globals=cur_globals
        )
        if cand is None:
            if not args.goal_seek:
                print(
                    f"[step {step}] {scene}/{block}: nothing actionable; "
                    "stopping (re-run with --goal-seek to attempt "
                    "prerequisite-chain recovery)"
                )
                break
            gate = _latest_blocking_gate(
                graph, scene, block, catalog=catalog, current_globals=client.globals()
            )
            if gate is None:
                print(
                    f"[step {step}] {scene}/{block}: nothing actionable and "
                    "no recent unsatisfied global gate to chase; stopping"
                )
                break
            slot, value = gate
            print(
                f"[step {step}] {scene}/{block}: explore exhausted; "
                f"trying goal-seek for globals[{slot}]={value}"
            )
            result = goal_seek.push_toward(slot=slot, value=value)
            for note in result.notes:
                print(f"  {note}")
            if not result.success:
                print(
                    f"[step {step}] goal-seek failed: no plot-pushable writer "
                    f"for globals[{slot}]={value} from {scene}/{block}; stopping"
                )
                break
            print(
                f"[step {step}] goal-seek advanced globals[{slot}]: "
                f"{result.observed_before} -> {result.observed_after} "
                f"via {len(result.steps_taken)} step(s)"
            )
            # Globals have moved; the state-aware dedup in
            # `ExplorePlanner.pick(..., current_globals=...)`
            # automatically re-opens previously-fired triggers
            # whose recorded outcome no longer matches live globals.
            continue
        print(
            f"[step {step}] {scene}/{block} -> fire {cand.kind} "
            f"{cand.name!r} (score={cand.score})"
        )

        if cand.kind == "object":
            globals_before = client.globals()
            try:
                client.interact(cand.name)
            except AgentError as e:
                print(f"  interact failed: {e}")
                continue
            # /v1/object/interact is async — wait for the dispatched
            # research handler to settle so globals_after reflects
            # any writes it made. 120s for the long PAL4 examine
            # cutscenes. See `Client.wait_for_idle`.
            if not client.wait_for_idle(timeout_sec=120.0):
                print(f"  interact {cand.name!r}: engine did not idle in 120s")
            globals_after = client.globals()
            graph.record_interact(
                scene=scene,
                block=block,
                object_name=cand.name,
                globals_before=globals_before,
                globals_after=globals_after,
            )
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
            cmp_literal_lookup=lambda fn_name, pc, _s=scene: catalog.cmp_literal(
                _s, fn_name, pc
            ),
        )
        drift = drift_summary(
            cand.catalog_trigger,
            (globals_before, globals_after),
            transitioned_to,
            scene,
            block,
            catalog_block=catalog.block(scene, block),
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
        default=180_000,
        help="server-side per-fire timeout (ms); bounded by the engine's "
        "MAX_FIRE_TIMEOUT_MS (180s). Long PAL4 cutscenes (M01 magic "
        "tutorial, Q01 dialog scenes) routinely chain dozens of "
        "giPlayerDoAction/giPlayerEndAction pairs with 4-8s "
        "animations each — fast-forward only affects `giWait`, not "
        "animation playback. Note: this is independent of the HTTP "
        "transport's reply_timeout — start the server with "
        "--agent-reply-timeout-secs to raise the HTTP side past 5s.",
    )
    explore.add_argument(
        "--boot-timeout-sec",
        type=float,
        default=60.0,
        help="how long to wait for the first idle scene before aborting "
        "(intro movie + per-scene init can take ~10–15s on a cold start)",
    )
    explore.add_argument(
        "--goal-seek",
        action="store_true",
        help="when explore can't pick anything actionable, enumerate the "
        "catalog's plot_index writers for the most-recent failed gate and "
        "save/load/walk to them in turn. Per `docs/pal4_plot_catalog.md`, "
        "this is the only sanctioned plot-advancement path — never "
        "`set_global`.",
    )
    explore.add_argument(
        "--save-slot",
        type=int,
        default=99,
        help="scratch save slot used by --goal-seek (default 99; PAL4's "
        "UI hotkeys only enumerate slots 1–8 so 99 is normally safe)",
    )
    explore.add_argument(
        "--max-path-hops",
        type=int,
        default=6,
        help="BFS depth cap for Catalog.path_to when goal-seek walks to "
        "a remote writer; raise if your plot route legitimately needs more",
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
