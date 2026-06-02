# pal4_planner

A concolic-style automation driver for PAL4 (`yaobow --pal4`). Connects
to the embedded agent server, fires plot triggers, captures the
AngelScript execution trace, and uses it to identify *which gate*
failed when a trigger didn't make progress.

> **Plot advancement is strictly trigger-driven.** The planner never
> writes script globals directly — it only fires triggers, interacts
> with objects, and chains prerequisite fires to satisfy gates. See
> [`docs/pal4_plot_catalog.md`](../../docs/pal4_plot_catalog.md#plot-advancement-not-set-the-flag)
> for the full rationale (camera moves, party swaps, item awards,
> music cues, and save bookkeeping are all tied to flag transitions
> in the original game logic; bypassing them desyncs the state).

## Prerequisites

The planner reads the static plot catalog from
`generated/pal4_plot.json`. `generated/` is **gitignored**, so the
file does not ship with the repo — generate it once from your local
PAL4 install:

```bash
cargo run -p pal4_plot_dump -- \
    --root /path/to/PAL4 \
    --out generated/pal4_plot.json
```

The dumper depends only on the file-format crates plus `packfs`, so
it runs on any host without a Vulkan / OpenAL stack. See
[`docs/pal4_plot_catalog.md`](../../docs/pal4_plot_catalog.md) for
the catalog schema and regeneration notes (`--root` accepts both
Steam / Origin installs with `gamedata/script.cpk` only and extracted
installs with loose `gamedata/script/*.csb`).

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

Python 3.10+. Only stdlib (no third-party deps), so a fresh install
needs nothing beyond a Python interpreter. The 3.10 floor comes from
`trace.py` using PEP 604 union syntax (`X | Y`) at module scope (not
just in annotations), which `from __future__ import annotations` does
not postpone.

## Reserved save slot

The planner uses save slot **99** as scratch for the goal-seek loop
(`Client.save(99)` / `Client.load(99)`). Pick a different value via
`--save-slot` if it collides with your usage; PAL4's own UI/hotkeys
only enumerate slots 1–8 (see
`yaobow/shared/src/openpal4/director.rs::poll_save_load_hotkeys`), so
99 is normally out of the way. The save file is a plain JSON under
`<yaobow save dir>/<app>/Save/99.json` and can be deleted by hand if
the planner crashes mid-loop and leaves an unwanted slot behind.

## Testing

```bash
python -m pytest tools/pal4_planner/tests
```

Live smoke (requires a running yaobow):

```bash
RUN_LIVE=1 python -m pytest tools/pal4_planner/tests/test_smoke_live.py
```
