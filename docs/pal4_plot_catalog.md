# PAL4 Plot Catalog (`pal4_plot.json`)

The `tools/pal4_plot_dump` binary generates a static JSON catalog of
every scene/block in PAL4 — its triggers, interactive objects, NPCs,
the block-to-block transitions it can take, and a per-function summary
of which AngelScript globals each handler reads / writes plus which
`gi*` sysfns it calls.

Pair this catalog with the live `/v1/scene/triggers`,
`/v1/scene/objects`, and `/v1/script/globals` endpoints (see
`docs/agent_interface.md`) to drive PAL4 from an external automation
agent.

## Regenerating

```bash
cargo run -p pal4_plot_dump -- \
    --root /path/to/PAL4Game \
    --out generated/pal4_plot.json
```

`--root` must be the directory containing `gamedata/`. Use `--pretty`
for human-readable diffs (much larger output). The generator depends
only on the file-format crates plus `packfs`, so it runs on any host
without a Vulkan / OpenAL stack.

When you need regenerate the plot dump, `generated/` folder is the
preferred destination. The folder is gitignored so the content won't
be checked in. 

## Schema

```jsonc
{
  "version": 1,
  "sysfn_count": <usize>,    // size of the openpal4 sysfn ordinal table
  "global_count": <usize>,   // size of script.csb's globals array
  "scenes": {
    "<scene>": {
      "blocks": {
        "<block>": {
          "entry_fn": "<scene>_<block>_init" | null,
          "triggers": [{
            "name":      "ev01",
            "function":  "q01_01_to_q01_02",
            "center":    [x, y, z],
            "half_size": [hx, hy, hz],
            "shape":     "box" | "plane" | "other"
          }],
          "npcs": [{
            "name":            "<logical npc name>",
            "position":        [x, y, z],
            "default_visible": true
          }],
          "objects": [{
            "name":              "<logical gob name>",
            "kind":              "generic"|"sound"|"machine"|"action"|"get_item"|"effect"|"marker"|"unknown",
            "position":          [x, y, z],
            "research_function": "<gas fn>" | ""
          }],
          "transitions": [{
            "to":     ["<scene>", "<block>"],
            "via_fn": "<fn that calls giArenaLoad>"
          }],
          "fns": {
            "<fn name>": {
              "reads":  [<global indices>],
              "writes": [<global indices>],
              "sysfns": ["giArenaReady", "giTalk", ...]
            }
          }
        }
      }
    }
  }
}
```

### How the values are recovered

| Field                       | Source                                                                 |
| --------------------------- | ---------------------------------------------------------------------- |
| `triggers`                  | EVF file (`gamedata/scenedata/<scene>/<block>/<block>.evf`)            |
| `npcs`                      | `…/<block>_4bit.npc` (NpcInfoFile)                                     |
| `objects`                   | `…/<block>_4bit.gob` (GobFile + `GobObjectType` tag table)             |
| `entry_fn`                  | `<scene>_<block>_init` if it exists in `script.csb`                     |
| `transitions`               | Abstract walk of every `<scene>_<block>_*` fn; records `giArenaLoad(scene_str, block_str, …)` calls whose first two args are both string literals on the operand stack. |
| `fns[..].reads` / `writes`  | `Rdga4` / `Movga4` instructions in the disassembly.                    |
| `fns[..].sysfns`            | Distinct `CallSys` indices resolved through `openpal4::scripting::create_context().functions()` (so the table stays in sync with the live game). |

### Walker caveats

The abstract walker is intentionally tiny. It only tracks literal-int
and string-table-index pushes, and clears the operand stack on **any**
branch, call, arithmetic op, or load it can't prove safe. The trade-off:

- `transitions` is conservative — if the destination scene/block is
  built up dynamically (e.g. concatenated, read from a global), the
  transition is silently omitted rather than guessed.
- `reads` / `writes` are exhaustive over the function body because
  they don't depend on the operand stack.
- `sysfns` is a *set* in encounter order — useful for "does this fn
  ever play a movie / open a dialog?" probes; not a call trace.

The catalog explicitly does **not** model:

- Control-flow reachability inside a function.
- Cross-block `giArenaCome*` round-trips (returned via
  `giArenaComeFromHere`, which is always faked to `1` in the engine).
- AS object methods (`CallBnd`).

If you need any of those, the live agent endpoints can be used to
observe behaviour at runtime; the catalog is meant for *planning*, not
authoritative simulation.

## Agent loop sketch

```
state    <- GET /v1/state                      # current scene/block
triggers <- GET /v1/scene/triggers             # what can fire here
objects  <- GET /v1/scene/objects              # what can be examined
globals  <- GET /v1/script/globals             # current plot vector

next_fn  <- pick from catalog[scene][block].triggers
            (respecting fns[next_fn].reads vs globals)

POST /v1/scene/fire_trigger {"name": ...}      # invoke
sleep / step until /v1/state.script_running == false

diff(GET /v1/script/globals, globals)          # did the plot advance?
```

If nothing moved, walk `catalog[scene][block].fns[next_fn].reads`,
search the catalog for fns that `writes` those globals, and chase the
gating handler.
