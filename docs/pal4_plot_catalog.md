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

`--root` must be the directory containing `gamedata/`. The dumper
mounts the same vfs the live engine uses (`packfs::init_virtual_fs`)
and reads scripts through it, so **both layouts work without
extraction**:

* a Steam / Origin install (`gamedata/script.cpk` archive only — no
  `gamedata/script/` directory on disk), and
* an extracted install where `gamedata/script/<Scene>.csb` exists as
  loose files.

Use `--pretty` for human-readable diffs (much larger output). The
generator depends only on the file-format crates plus `packfs`, so it
runs on any host without a Vulkan / OpenAL stack.

When you need regenerate the plot dump, `generated/` folder is the
preferred destination. The folder is gitignored so the content won't
be checked in. 

## Schema

```jsonc
{
  "version": 1,
  "sysfn_count": <usize>,       // size of the openpal4 sysfn ordinal table
  "global_count": <usize>,      // size of script.csb's globals array
  "scenes": {
    "<scene>": {
      // Per-block playable data.
      "blocks": {
        "<block>": {
          "entry_fn": "<scene>_<block>_init" | null,
          "triggers": [{
            "name":      "ev01",
            "function":  "q01_01_to_q01_02",
            "center":    [x, y, z],
            "half_size": [hx, hy, hz],
            "shape":     "box" | "plane" | "other",
            // NEW — call-graph closure aggregated across `function`
            // and every fn it Call's transitively (BFS, capped at
            // CALL_CLOSURE_DEPTH=16 fns). See "Trigger / object
            // closure" below.
            "called_fns":  ["func2006", ...],
            "reads":       [<global slot>, ...],
            "writes":      [{"global": <slot>, "value": <i32> | null}, ...],
            "transitions": [["<scene>", "<block>"], ...]
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
            "research_function": "<gas fn>" | "",
            // NEW — same closure shape as triggers, computed from
            // `research_function` (empty when the entry has no
            // examine handler).
            "called_fns":  [...],
            "reads":       [...],
            "writes":      [...],
            "transitions": [...]
          }]
        }
      },

      // Scene-wide tables. Per-function summaries and the
      // `giArenaLoad` transitions list both live at scene scope
      // because PAL4 scripts mostly use generic `funcNNNN` names
      // that don't carry a block prefix; blocks reference these
      // by name through their EVF / GOB handler fields.
      "fns": {
        "<fn name>": {
          "reads":  [<shared-global slot>, ...],
          "writes": [{"global": <slot>, "value": <i32> | null}, ...],
          "sysfns": ["giArenaReady", "giTalk", ...],
          "calls":  ["funcNNNN", ...]   // intra-module Call targets
        }
      },
      "transitions": [{
        "to":     ["<scene>", "<block>"],
        "via_fn": "<fn that calls giArenaLoad>"
      }],

      // OPTIONAL — present when the scene's `.csb` exists but
      // either (a) carries no `*_init` fns (worldMap, M02 in some
      // builds) or (b) failed to parse. Agents following the
      // transitions list should treat a `note`-only scene as
      // "reachable but un-modelled" rather than missing.
      "note": "no *_init fns; reachable only via giArenaLoad …"
            | "parse failure: <error> — scene reachable via giArenaLoad …"
    }
  },

  // NEW — reverse index "to advance global[i] to value V, fire one
  // of these triggers". Built from every trigger / examine handler's
  // closure-aggregated `writes`. See "Plot advancement" below.
  "plot_index": {
    "<global slot>": [{
      "value":   <i32> | null,    // null when the writer's value is computed, not a literal
      "scene":   "<scene>",
      "block":   "<block>",
      "trigger": "<evf name>"  | null,
      "object":  "<gob name>"  | null,
      "fn":      "<fn that performs the Movga4>"
    }, ...]
  }
}
```

### How the values are recovered

| Field                       | Source                                                                 |
| --------------------------- | ---------------------------------------------------------------------- |
| `triggers`                  | EVF file (`gamedata/scenedata/<scene>/<block>/<block>.evf`)            |
| `npcs`                      | `…/<block>_4bit.npc` (NpcInfoFile)                                     |
| `objects`                   | `…/<block>_4bit.gob` (GobFile + `GobObjectType` tag table)             |
| `entry_fn`                  | `<scene>_<block>_init` if it exists in the per-scene `.csb`            |
| `transitions`               | Abstract walk of every fn in the scene's module; records `giArenaLoad(scene_str, block_str, …)` calls whose first two args are both string literals on the operand stack. |
| `fns[..].reads` / `writes`  | `Rdga4` / `Movga4` instructions with **negative** indices (= shared globals; the ones `/v1/script/globals` exposes). Positive indices reference module-local globals and are intentionally dropped — they are not part of the plot vector. The decode mirrors `ScriptVm::rdga4` / `movga4` in `yaobow/shared/src/scripting/angelscript/vm.rs`. |
| `fns[..].writes[].value`    | Top-of-operand-stack literal observed at the `Movga4` site: a recent `Set4 v` / `PushZero`. When the value is computed (returned from a sysfn, read from another global, etc.) the slot still records the write with `value: null`. |
| `fns[..].sysfns`            | Distinct `CallSys` indices resolved through `openpal4::scripting::create_context().functions()` (so the table stays in sync with the live game). |
| `fns[..].calls`             | Distinct intra-module `Call { function }` targets, resolved against `module.functions[function].name`. Seeds the per-trigger closure walker. |
| `triggers[..]` closure      | BFS from `function` over `fns[..].calls`, unioning `reads` / `writes` and any `transitions` whose `via_fn` was visited. Capped at `CALL_CLOSURE_DEPTH = 16` fns total. |
| `objects[..]` closure       | Same as triggers, seeded from `research_function`. Empty when the entry has no examine handler. |
| `plot_index`                | Inverted from every `triggers[..].writes` and `objects[..].writes` across the catalog, keyed by `global` slot, deduped on `(scene, block, trigger, object, fn, value)`. |

### Walker caveats

The abstract walker is intentionally tiny. It only tracks literal-int
and string-table-index pushes, and clears the operand stack on **any**
branch, arithmetic op, or load it can't prove safe. The trade-off:

- `transitions` is conservative — if the destination scene/block is
  built up dynamically (e.g. concatenated, read from a global), the
  transition is silently omitted rather than guessed.
- `reads` / `writes` are exhaustive over the function body because
  they don't depend on the operand stack.
- `writes[].value` is best-effort: present only when the value
  reaches `Movga4` as an unbroken `Set4 v` / `PushZero` push. Branches
  / function calls / arithmetic in between produce `value: null`.
- `triggers[..].transitions` / `triggers[..].writes` are unions over
  the BFS-reachable fn set, not a control-flow-aware "this trigger
  *will* fire this transition" claim. A fn whose body conditionally
  picks one of two `giArenaLoad`s will report both.
- `sysfns` / `calls` are *sets* in encounter order — useful for
  "does this fn ever play a movie / open a dialog?" probes; not a
  call trace.

The catalog explicitly does **not** model:

- Control-flow reachability inside a function.
- Cross-block `giArenaCome*` round-trips (returned via
  `giArenaComeFromHere`, which is always faked to `1` in the engine).
- AS object methods (`CallBnd`).

If you need any of those, the live agent endpoints can be used to
observe behaviour at runtime; the catalog is meant for *planning*, not
authoritative simulation.

## Plot advancement (NOT "set the flag")

**Do not** add a `POST /v1/script/globals` "set flag directly"
endpoint, and do not propose one. Directly writing into
`ScriptGlobalContext.vars` bypasses every scripted side-effect the
original game ties to the same flag transition — camera moves, party
swaps, item awards, music cues, NPC visibility, save-system
bookkeeping. The resulting state *looks* advanced to the agent but
will desync the moment the next genuine scripted scene reads the same
flag through the live game logic. Reverse-engineering catches none of
that drift.

The intended workflow is **strictly trigger-driven**:

1. Read `/v1/state` for the current `scene` / `block`.
2. Read `/v1/script/globals` for the current plot vector.
3. Pick the flag you need to advance (or the trigger you want to
   fire, then read its `writes` to see what flags it will set).
4. Look up `plot_index[<slot>]` for the list of `{scene, block,
   trigger, fn, value}` rows that *write* that flag. Filter by the
   `value` you want (when known).
5. If the chosen row's `scene` / `block` matches your current
   location, `POST /v1/scene/fire_trigger {"name": <trigger>}`
   (or `/v1/object/interact` when `object` is set). Otherwise chase
   a transition path: query the chosen scene's `transitions` for an
   edge from your current scene/block, and the scene/block-pair's
   own `plot_index` to find what trigger walks that edge.
6. If the fire returns 409 / has no observable effect, the gating
   prerequisite is one of the trigger's `reads`. Recurse: look each
   read slot up in `plot_index` to find *its* gating trigger, fire
   that first, then retry.

Concretely: in the session that drove the plot from Q01/Q01 to M01/1,
the `ev_M01_1_5` trigger appeared to do nothing. The catalog now
explains why — `func1001` writes `globals[0] = 11400`, but only
inside the branch the walker can't prove reachable. To advance, fire
the trigger whose `plot_index` entry actually sets `globals[0]` to
the next plot ID (e.g. `ev_M01_1_6` → `func2002` → `globals[0] :=
10300`). Engine-driven, scripted, no cheating.

## Agent loop sketch

```
state    <- GET /v1/state                      # current scene/block
triggers <- GET /v1/scene/triggers             # what can fire here
objects  <- GET /v1/scene/objects              # what can be examined
globals  <- GET /v1/script/globals             # current plot vector

# Pick the next plot-advancing trigger using the closure-aggregated
# writes on each trigger (NOT a "set globals" endpoint — see above).
candidates <- [
    t for t in catalog.scenes[scene].blocks[block].triggers
    if t.writes intersects (the flags you want to advance)
       and t.reads.subset_of(globals)         # gates satisfied
]

POST /v1/scene/fire_trigger {"name": candidates[0].name}
wait until /v1/state.script_running == false  # giWait honours fast_forward

diff(GET /v1/script/globals, globals)         # did the plot advance?
```

If nothing moved, walk the trigger's `reads`, look each slot up in
`plot_index`, and fire the gating trigger(s) first. Repeat.

