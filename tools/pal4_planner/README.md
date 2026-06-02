# pal4_planner

A concolic-style automation driver for PAL4 (`yaobow --pal4`). Connects
to the embedded agent server, fires plot triggers, captures the
AngelScript execution trace, and uses it to identify *which gate*
failed when a trigger didn't make progress.

## Quickstart

```bash
# 1. Start yaobow with the agent server.
yaobow --pal4 --agent-port 8765

# 2. From a separate shell, drive the plot.
cd tools/pal4_planner
python -m pal4_planner.drive explore --catalog ../../generated/pal4_plot.json
```

`explore` reads the static plot catalog, fires every trigger in each
block (preferring the ones whose static catalog entry exits the
block), captures the execution trace of each fire, and persists
everything to a SQLite DB at `~/.cache/pal4_planner.sqlite` (or
override with `--db <path>`).

Inspect a previous run:

```bash
python -m pal4_planner.drive inspect      # pretty-print last fire's trace
python -m pal4_planner.drive replay <id>  # replay a fire sequence
```

## Layout

```
pal4_planner/
├── client.py       HTTP client wrapping the agent endpoints
├── trace.py        TraceEvent dataclasses + accessors
├── catalog.py      Static-catalog loader with block-synthesis fallback
├── graph.py        SQLite-backed dynamic edge graph
├── planner.py      Drift detection + next-action selection
├── drive.py        CLI front-end
└── tests/          Unit tests (mocked Client)
```

## Requirements

Python 3.9+. Only stdlib (no third-party deps), so a fresh install
needs nothing beyond a Python interpreter.

## Testing

```bash
python -m pytest tools/pal4_planner/tests
```

Live smoke (requires a running yaobow):

```bash
RUN_LIVE=1 python -m pytest tools/pal4_planner/tests/test_smoke_live.py
```
