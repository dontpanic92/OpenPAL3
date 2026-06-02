# PAL4 Planner

A concolic-style automation driver for PAL4 that lives in
`tools/pal4_planner/`. Built on top of the agent server's
[`docs/agent_interface.md`](agent_interface.md) endpoints — in
particular the new VM **execution-trace** stream
(`/v1/script/trace/*`) — to *push* the plot reliably instead of
guessing which trigger to fire next.

## Why concolic

Static analysis (the `tools/pal4_plot_dump`-generated
`generated/pal4_plot.json`) tells you *which* triggers exist and
which globals each *might* read or write. It cannot tell you whether
a specific fire will actually do anything from the current game
state — most plot-advancing branches are gated on `g[X] == V` or
`giHasItem(N)` checks the static walker doesn't model precisely.

The planner solves this by:

1. **Firing the trigger** through `/v1/scene/fire_trigger
   { wait_until_idle, collect_trace }`.
2. **Reading the live execution trace** the VM produced for that
   fire via `/v1/script/trace/drain`.
3. **Diffing observed vs catalog-predicted** `(reads, writes,
   transitions)`. The trace shows the *actual* branch outcomes and
   sysfn return values, so "trigger no-op" is always pinpointed to
   a specific failed predicate.
4. **Recording everything** to a SQLite DB so the next iteration
   knows "the last time we fired `ev_M01_1_5`, the gate was
   `global[0] == 11400` but observed `10300`".

The static catalog stays useful as a search-frontier hint (and is
itself enriched by the C1 + C4 fixes that synthesise missing-block
entries and tag wall/camera-helper volumes), but the **dynamic edge
graph** is the source of truth for plot-progression decisions.

## Data model

The SQLite DB (default `~/.cache/pal4_planner.sqlite`, override
with `--db`) has three tables:

| Table          | Row meaning                                                 |
| -------------- | ------------------------------------------------------------ |
| `fires`        | One row per executed trigger. Carries `(scene, block, name, fn, settled, waited_frames, transitioned_to_*, globals_before_json, globals_after_json)`. |
| `trace_events` | One row per VM trace event. `(fire_id, seq, kind, payload_json)`. |
| `gates`        | Inferred predicate observations: `(fire_id, kind, slot_or_sysfn, observed_value, taken)`. Populated from the trace by `derive_gates_from_trace`. |

Schema version is tracked in `schema_version(version)`. The planner
refuses to start against an older DB; pass `--reset` to drop +
recreate.

## Decision algorithm (current iteration: `explore`)

```text
for step in 0..max_steps:
    state = client.state()
    scene, block = state.scene, state.block
    live_triggers = client.scene_triggers()
    live_objects  = client.scene_objects().objects

    # Filter to triggers we haven't already fired in this block.
    candidates = [t for t in live_triggers
                  if t.function != ""
                     and (scene, block, t.name) not in fires]

    # Score: prefer transitions that leave the block; then
    # plot-relevance (writes / reads) per catalog; then known
    # triggers over uncatalogued ones.
    candidates.sort(key=score, reverse=True)

    if candidates:
        fire = client.fire_trigger_sync(candidates[0].name)
    else:
        # Fall back to GOB object research_functions, skipping
        # purely-numeric synthetic names (issue A4).
        fire = client.interact(first_research_object())
        continue

    events = client.trace_drain(after_seq=fire.trace_seq_start - 1)
    graph.record_fire(..., trace=events)
```

The next planner pass adds **goal-seek**: given a target
`(scene, block)`, BFS over the dynamic edge graph (the
`fires.transitioned_to_*` columns) to find a sequence of triggers
that reach it, falling back to `save / load` checkpointing when a
speculative fire dead-ends. The `gates` table makes
prerequisite-search practical: "this trigger requires `global[0]
== 11400`; which other trigger's `writes` produces that value?".

## CLI

```bash
# Build the graph from a fresh DB.
python -m pal4_planner.drive --reset explore --steps 80

# Dump the last fire's trace events.
python -m pal4_planner.drive inspect

# Inspect a specific fire by ID.
python -m pal4_planner.drive inspect --fire-id 12

# Re-print a recorded fire (useful when triaging a stuck run).
python -m pal4_planner.drive replay --fire-id 12
```

CLI also accepts `--base-url`, `--token`, `--db`, `--catalog`.

## Debugging a stuck run

1. **State snapshot**: `curl /v1/state | jq` shows the current
   `(scene, block, current_script_fn, script_running, movie_playing)`.
   `script_running=false` + `movie_playing=false` is the only state
   in which a planner step can proceed.
2. **Last fire**: `python -m pal4_planner.drive inspect` prints the
   most recent fire's trace. Look for:
   - `branch` events with `taken=false` immediately after a
     `global_read` / `call_sys` returning `0` — that's the failed
     gate.
   - No `global_write` events at all on a "plot trigger" — the fire
     was a no-op because the early-return branch took.
3. **Drift notes** are printed inline during `explore`:
   `drift_notes=['global[0] expected -> 11500 but observed 10201']`
   means the catalog promised a write that didn't happen, *because
   the gating branch was not taken*.
4. **Manual fire**:
   ```bash
   curl -s -X POST http://127.0.0.1:8765/v1/scene/fire_trigger \
     -d '{"name":"ev_M01_1_5","wait_until_idle":true,"collect_trace":true}' | jq
   ```
   then `curl -s "http://127.0.0.1:8765/v1/script/trace/drain?after_seq=N"`
   to read the trace yourself.

## Known limitations

- The current planner is single-step / explore-only. Goal-seek BFS
  + save/load backtracking are queued (see the matching todos in
  the session plan).
- Battles, mini-games, and `giMenu` choice trees are not yet
  modelled in the gate database — the planner correctly identifies
  these as "stuck" but cannot resolve them. `/v1/dialog/choose` is
  already wired for `giSelectDialogGetLastSelect` /
  `giCommonDialogGetLastSelect`, so menu-driven progression is
  possible from the agent surface; only the planner's choice
  exploration is missing.
- Object names that are purely numeric (e.g. `"1"`, `"2"`) are
  skipped by the fallback — they almost never correspond to
  plot-pushing entities and the planner avoids them to keep the
  exploration arm focused.

## Testing

```bash
python -m pytest tools/pal4_planner/tests
```

Live smoke (requires a running `yaobow --pal4 --agent-port 8765`):

```bash
RUN_LIVE=1 python -m pytest tools/pal4_planner/tests/test_smoke_live.py
```

The live smoke is gated by `RUN_LIVE` so CI runs the offline unit
suite by default.
