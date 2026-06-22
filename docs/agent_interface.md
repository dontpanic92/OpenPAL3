# PAL3 / PAL4 Agent Interface

OpenPAL3's PAL4 and PAL3 binaries can run with an **embedded HTTP+JSON
server** that
exposes observability and control endpoints for an external automation
agent (an AI test driver, a Python script, a shell loop, your future MCP
adapter, etc.). The server is opt-in via the `--agent-port` flag and
loopback-bound by default — turn it off and the game runs exactly as
before.

The same flag is also supported on the PAL3 binary (`yaobow --pal3
--agent-port 8765`). The wire protocol and transport are shared; the
[PAL3 support matrix](#pal3-support) below lists which endpoints are
fully implemented, partially supported, or `not_implemented` against the
PAL3 binary.

## Boot

```bash
# Default: bind 127.0.0.1:8765 with no token.
yaobow --pal4 --agent-port 8765
yaobow --pal3 --agent-port 8765   # PAL3 — same flag set, same protocol.

# Explicit bind + bearer token (required for any non-loopback bind).
yaobow --pal4 --agent-port 8765 --agent-bind 127.0.0.1 --agent-token hunter2
```

The listener logs `agent_server: listening on http://127.0.0.1:8765 (PAL4)`
(or `(PAL3)`) once it's ready to accept requests.

## Endpoints

All payloads are `application/json`. Responses with shape
`{"type": "error", "data": { "kind": …, "message": … }}` carry the
appropriate HTTP status:

| HTTP status | `kind`            | Meaning                                              |
| ----------- | ----------------- | ---------------------------------------------------- |
| 400         | `bad_request`     | malformed payload / unknown key name                 |
| 401         | `unauthorized`    | missing or wrong `Authorization: Bearer …`           |
| 409         | `conflict`        | valid but rejected by game state (e.g. step while running) |
| 501         | `not_implemented` | feature gated on follow-up work (e.g. screenshot)    |
| 500         | `internal`        | anything else                                        |

### Observe

| Method | Path                                | Description |
| ------ | ----------------------------------- | ----------- |
| `GET`  | `/v1/state`                         | Full snapshot: scene/block, leader pos, party HP/MP, money, dialog (text + open + avatar + `choices[]`), `inventory[]`, fps, pause flag, `script_running`, `movie_playing`, current script function. |
| `GET`  | `/v1/log/tail?after_seq=N&n=M`      | Ring-buffered log records since `after_seq`. The `dropped` flag warns when records were evicted before the caller polled. |
| `GET`  | `/v1/screenshot`                    | **Binary `image/png`** of the most recently presented swapchain frame (includes UI). Response carries `X-Screenshot-Width` / `X-Screenshot-Height` headers. Returns **501** when no frame has been presented yet, when the swapchain format is unsupported, or in headless builds without a presentable surface. |
| `GET`  | `/v1/scene/triggers`                | EVF event triggers for the currently loaded block: `{name, function, center, half_size, shape}`. `shape` is `"box"` (8 vertices), `"plane"` (4 vertices), or `"other"` — `"other"` triggers are skipped by the live engine but still surfaced here for inspection. |
| `GET`  | `/v1/scene/objects`                 | GOB objects + NPCs for the current block. Each object carries `{name, kind, position, visible, research_function}`; each NPC carries `{name, position, visible}`. `position` reflects live world-space (post script teleports), not load-time values. |
| `GET`  | `/v1/script/globals?start=N&limit=M`| Window over the AngelScript shared-globals array (story-plot flags). Response is `{len, start, globals}`. `len` is the full underlying array size; clients diff `globals[]` between actions to detect plot progression. |
| `GET`  | `/v1/script/trace/drain?after_seq=N&n=M` | Drain buffered VM execution-trace events with `seq > after_seq`. Capped at `n` per call (default 1024). Response is `{next_seq, dropped, capturing, events}`; see the **Trace** section below for the event reference. Streamed via repeated drains using the returned `next_seq` cursor. |

### Control

| Method | Path                                | Body                                                  |
| ------ | ----------------------------------- | ----------------------------------------------------- |
| `POST` | `/v1/input/key`                     | `{"key":"F","action":"tap"\|"down"\|"up"}`            |
| `POST` | `/v1/input/axis`                    | `{"axis":"LeftStickX","value":-1.0}`                  |
| `POST` | `/v1/player/teleport`               | `{"player":0,"pos":[x,y,z]}`                          |
| `POST` | `/v1/dialog/advance`                | _(empty body)_ — synthesises a `Space` tap            |
| `POST` | `/v1/dialog/choose`                 | `{"index":N}` — buffer a 1-based choice index for the next `giSelectDialogGetLastSelect` / `giCommonDialogGetLastSelect` call. See `/v1/state.dialog.choices` for the available items. |
| `POST` | `/v1/world_map/choose`              | `{"scene":"M02","block":"1"}` — buffer a destination for the next `giShowWorldMap` continuation tick. While the world map is open `/v1/state.world_map_open` is `true` and the surrounding script is suspended in a `Yield`. The buffered choice is one-shot and only consumed when the map prompt actually fires, so it's safe to pre-buffer the catalog-predicted destination before posting `/v1/scene/fire_trigger`. |
| `POST` | `/v1/scene/fire_trigger`            | `{"name":"ev01"}` (legacy) **or** `{"name":"ev01", "wait_until_idle":true, "collect_trace":true, "timeout_ms":5000}`. With `wait_until_idle` set the dispatcher defers the response until the VM becomes idle for two consecutive frames (or `timeout_ms` elapses); the reply then carries `{settled, waited_frames, trace_seq_start, trace_seq_end, current_script_fn}` so the caller can drain just this fire's trace events without races. **409** while a script is already running; **400** when the name is unknown or has no bound function. |
| `POST` | `/v1/object/interact`               | `{"name":"npc_lingsha"}` — fires a GOB entry's `research_function` (its "Examine" handler). **400** with `{"kind":"bad_request"}` when the entry has no examine handler. |

`action: "tap"` emits one frame of `pressed + released + is_down` and
naturally goes back to `up` next frame. `down` / `up` are sticky.

### Time

| Method | Path                                | Body                                                  |
| ------ | ----------------------------------- | ----------------------------------------------------- |
| `POST` | `/v1/time/pause`                    | _(empty body)_ — freeze the simulation                |
| `POST` | `/v1/time/resume`                   | _(empty body)_ — drop pending step budget, resume     |
| `POST` | `/v1/time/step`                     | `{"frames":60,"dt":0.0167}` (`dt` optional)           |
| `POST` | `/v1/time/fast_forward`             | `{"on":true}` — skips scripted `giWait`, dialog waits, and movie playback. While enabled, scripted movement/rotation tweens are also accelerated so wait-for-motion continuations (`player_end_move`, `npc_end_move`, `player_set_dir { sync = 1 }`, …) complete in a single frame. |

`step` is only honoured when the simulation is paused. The director
runs one fixed-step frame per real frame of pending budget; long-
running scripted waits (`giWait`) still consume their own simulated
time, so issue `fast_forward` if you want a hard skip.

### Persistence

| Method | Path                                | Body                          |
| ------ | ----------------------------------- | ----------------------------- |
| `POST` | `/v1/save`                          | `{"slot":1}` — slot file under `<save_dir>/OpenPAL4/Save/<slot>.json` |
| `POST` | `/v1/load`                          | `{"slot":1}` — **single** load endpoint; auto-routes by mode |

`/v1/load` is mode-aware: when a playthrough is active it restores the
slot **in-place**; when issued at the start menu it **boots a fresh
director** from the slot. There is intentionally no separate
`/v1/menu/load` route (see [Changes](#changes)).

### Lifecycle

| Method | Path                                | Body                          |
| ------ | ----------------------------------- | ----------------------------- |
| `POST` | `/v1/menu/new_game`                 | _(empty body)_ — start a fresh playthrough (works at the menu or as a restart from story) |
| `POST` | `/v1/menu/exit`                     | _(empty body)_ — quit the application |

### Trace

| Method | Path                                | Body                                                  |
| ------ | ----------------------------------- | ----------------------------------------------------- |
| `POST` | `/v1/script/trace/start`            | `{}` (use defaults) or `{"reset":true, "capacity":65536}`. Arms the VM-side `TraceSink`; subsequent VM activity is recorded into a bounded ring buffer for [`/v1/script/trace/drain`](#observe) to read. |
| `POST` | `/v1/script/trace/stop`             | _(empty body)_ — stop recording. Buffered events remain drainable until evicted by the next `start { reset }`. |

The drain endpoint returns events in the following shape (one item
per `events[]`):

```jsonc
{ "seq": 123,
  "kind": {
    "type": "branch" | "call_sys" | "global_read" | "global_write"
          | "fn_enter" | "fn_exit" | "suspend",
    // type-specific fields...
  }
}
```

Notable variants for plot-progression analysis:

- `branch` — `{fn_name, pc, branch ("jz" | "jnz" | "js_jgez" | ...), operand, offset, taken}`. The `taken` outcome paired with the predicate that fed it (the preceding `global_read` / `call_sys`) is how the planner identifies "which gate failed" on an unproductive fire.
- `call_sys` — `{fn_name, pc, sysfn_index, sysfn_name, sp_before, sp_after, r1_after}`. `r1_after` carries the legacy `gi*` ABI's return value, so `giHasItem` etc. are observable here.
- `global_read` / `global_write` — `{fn_name, pc, scope ("shared" | "module"), slot, value}`. Shared-globals match the array exposed via `/v1/script/globals`.

### Script eval

| Method | Path                                | Notes                                                 |
| ------ | ----------------------------------- | ----------------------------------------------------- |
| `POST` | `/v1/script/eval`                   | **501 — intentional.** Plot advancement is trigger-driven; see [`docs/pal4_plot_catalog.md`](pal4_plot_catalog.md#plot-advancement-not-set-the-flag). The stub stays for future read-only inspector use cases. |

## Changes

* **`/v1/menu/load` removed (breaking).** Loading is now a single,
  mode-aware `POST /v1/load`: it restores **in-place** when a playthrough
  is active and **boots a fresh director from the slot** at the start
  menu. Callers that previously posted `/v1/menu/load {slot}` should post
  `/v1/load {slot}` instead. `/v1/menu/new_game` and `/v1/menu/exit` are
  unchanged.

## Examples

### `curl`

```bash
$ curl -s http://127.0.0.1:8765/v1/state | jq
{
  "type": "state",
  "data": {
    "frame": 4823,
    "scene": "q01",
    "block": "q01_01",
    "leader": 0,
    "leader_pos": [12.34, 0.0, -45.6],
    "party": [
      { "slot": 0, "level": 1, "hp": 0, "max_hp": 0, "mp": 0, "max_mp": 0, "in_team": true },
      …
    ],
    "money": 0,
    "quest_percentage": 0,
    "dialog": { "open": false, "text": "", "avatar": "left", "choices": [] },
    "inventory": [],
    "fast_forward": false,
    "paused": false,
    "current_script_fn": "q01_01_main",
    "script_running": true,
    "movie_playing": false,
    "world_map_open": false,
    "fps": 59.7,
    "dt": 0.01672
  }
}

$ curl -s -X POST http://127.0.0.1:8765/v1/input/key \
       -d '{"key":"F","action":"tap"}'
{"type":"ok"}

$ curl -s -X POST http://127.0.0.1:8765/v1/time/pause -d '{}'
{"type":"ok"}

$ curl -s -X POST http://127.0.0.1:8765/v1/time/step \
       -d '{"frames":30,"dt":0.01667}'
{"type":"ok"}

# Binary screenshot — pipe directly to disk, no JSON decoding needed.
$ curl -s http://127.0.0.1:8765/v1/screenshot -o screen.png \
       -D - | grep -i 'x-screenshot\|content-type'
content-type: image/png
x-screenshot-width: 1920
x-screenshot-height: 1080
$ file screen.png
screen.png: PNG image data, 1920 x 1080, 8-bit/color RGBA, non-interlaced
```

### Python driver

```python
import json
import time
import urllib.request

BASE = "http://127.0.0.1:8765"

def post(path, body=None):
    req = urllib.request.Request(
        f"{BASE}{path}", method="POST",
        data=json.dumps(body or {}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        return json.loads(r.read())

def get(path):
    with urllib.request.urlopen(f"{BASE}{path}", timeout=5) as r:
        return json.loads(r.read())

# Pause, advance 60 fixed-step frames, snapshot.
post("/v1/time/pause")
post("/v1/time/step", {"frames": 60, "dt": 1/60})
print(get("/v1/state"))
post("/v1/time/resume")

# Tail the log since the last cursor we saw.
cursor = 0
while True:
    log = get(f"/v1/log/tail?after_seq={cursor}&n=100")["data"]
    cursor = log["next_seq"]
    for rec in log["records"]:
        print(rec["level"], rec["target"], rec["msg"])
    time.sleep(0.5)
```

### Pushing the plot from a Python driver

The observability + direct-fire endpoints are designed so an automation
driver can advance the game without solving navigation. The recipe is:

1. Read `/v1/state` to learn the current `scene` / `block`.
2. Pair that with the **static plot catalog** (see
   `docs/pal4_plot_catalog.md`) to pick the next trigger to fire.
3. `POST /v1/scene/fire_trigger` (or `/v1/object/interact`) to invoke
   the handler directly — no teleport / pathfinding needed.
4. Diff `/v1/script/globals` before vs after the call. If nothing
   moved, the catalog's `fns[..].reads` for the fired function tells
   you which globals gate it — that's the prerequisite plot flag the
   agent needs to satisfy elsewhere.

```python
def fire_next_trigger():
    state = get("/v1/state")["data"]
    scene, block = state["scene"], state["block"]
    triggers = get("/v1/scene/triggers")["data"]["triggers"]
    if not triggers:
        return False
    pre = get("/v1/script/globals")["data"]["globals"]
    name = triggers[0]["name"]  # in practice: pick from the catalog
    post("/v1/scene/fire_trigger", {"name": name})
    # Let the engine run a few frames so the handler can settle.
    time.sleep(0.5)
    post_globals = get("/v1/script/globals")["data"]["globals"]
    moved = [(i, a, b) for i, (a, b) in enumerate(zip(pre, post_globals)) if a != b]
    print(f"fired {name}; globals changed: {moved}")
    return bool(moved)
```

> **Reachability is *out of scope* for the agent surface.** We assume
> "if the catalog lists a trigger in the current block, the agent may
> fire it directly". Real prerequisites (closed bridges, story flags)
> show up as "fired but no globals moved" — the agent's job is to
> consult the static catalog to discover the gating handler, not to
> solve navigation. A future patch may add path-following on top of
> `/v1/player/teleport` + synthetic input for cases that genuinely
> need it.

## Known limitations / RE signals

OpenPAL3 is an active **reverse-engineering** project. Many file
formats, AngelScript opcodes, and per-game scripted side-effects are
still being mapped out from the original binaries. To keep that
mapping honest, the agent surface treats **process death as a
discovery signal, not a bug to suppress**:

* Script / asset *load* failures (`.csb`, `.gob`, `.npc`, `.dff`, …)
  intentionally panic. Canonical examples:
  `yaobow/shared/src/openpal4/asset_loader.rs::load_script_module`
  (`ScriptModule::read_from_buffer(...).unwrap()`) and
  `yaobow/shared/src/openpal4/app_context.rs::load_scene`
  (`Pal4Scene::load(...).unwrap()`).
* Script *parse* and *VM execute* failures (unknown opcodes,
  unimplemented `gi*` sysfns, stack underflow) likewise panic via
  `unimplemented!()` / `.unwrap()` rather than logging-and-continuing.

When an agent run trips one of these, that's a successful discovery
of unknown content. Do **not** swallow the error behind a logged
`anyhow::Result` — file a bug with the panicking input so the format
or sysfn can be added properly. The agent server itself is robust to
the game thread dying (the OS reaps the listener with it); restart
yaobow and resume the run from a save slot.

## Security

* The default bind is `127.0.0.1`; non-loopback binds **require** a
  bearer token (`AgentServerConfig::with_token`).
* No filesystem endpoints, no arbitrary code eval. `/v1/script/eval`
  is intentionally 501 — plot advancement is trigger-driven (see
  [`docs/pal4_plot_catalog.md`](pal4_plot_catalog.md#plot-advancement-not-set-the-flag)),
  and there is no `/v1/script/globals/set` endpoint either.
* The server does not respond to non-`/v1/...` URLs; unknown routes
  return `400 bad_request`.

## Implementation map

```
yaobow/agent_server/          # transport-agnostic crate (no engine deps)
├── src/protocol.rs           # AgentCommand / AgentResponse + JSON layout
├── src/queue.rs              # producer (Sync) + consumer (game-thread)
├── src/log_sink.rs           # bounded ring-buffer log::Log adapter
├── src/transport.rs          # tiny_http listener + routing
├── src/session.rs            # AgentSession trait + NullAgentSession stub
└── tests/                    # round-trip + e2e tests

radiance/radiance/src/input/synthetic.rs
                              # SyntheticInputBridge: OR-merge synthetic
                              # key / axis state with the real engine

yaobow/shared/src/openpal4/
├── agent.rs                  # Pal4AgentBridge (queue + bridge + cells)
└── director.rs               # drain loop + pause/step + dispatcher

yaobow/yaobow/src/openpal4/application.rs
                              # boot wiring (--agent-port → AgentServer::start)

yaobow/yaobow/src/main.rs     # CLI parsing of --agent-port / --bind / --token
```

The `agent_server` crate has no dependency on radiance or the game
crates — it's reusable for PAL3 / PAL5 by writing additional
per-game bridges that drain the same queue.

## Roadmap

* `headless-toggle`: still gated behind a Vulkan init refactor. Plain
  windowed mode covers every other endpoint today, **including
  `/v1/screenshot`** which transparently returns 501 in headless
  builds (no presentable surface to capture).
* MCP wrapper: trivial follow-up — it's just a client of these HTTP
  endpoints.

## PAL3 support

The PAL3 binary (`yaobow --pal3 --agent-port N`) speaks the same wire
protocol, but PAL3 has a smaller modeled gameplay surface than PAL4
(no battle system, no inventory/HP/MP, no world-map prompt, no GOB
`Examine` callback, and SCE bytecode instead of AngelScript). The
unsupported endpoints return `{"type":"error","data":{"kind":
"not_implemented","message":…}}` so external drivers can probe and
fall back gracefully.

### Snapshot field semantics (`/v1/state`)

PAL3 maps the shared `StateSnapshot` shape to its own engine state.
Differences from PAL4:

| Field                | PAL3 value                                                  |
| -------------------- | ----------------------------------------------------------- |
| `scene`              | `PersistentState::scene_name()` (e.g. `"q01"`, `"m22"`)     |
| `block`              | `PersistentState::sub_scene_name()` (PAL3's "sub-scene")    |
| `leader`             | `GlobalState::role_controlled()`                            |
| `leader_pos`         | Live transform of the resolved role entity                  |
| `party`              | Always `[]` — PAL3 has no battle system in the OSS impl     |
| `money`              | Always `0` (not modeled)                                    |
| `quest_percentage`   | Always `0` (not modeled)                                    |
| `inventory`          | Always `[]` (not modeled)                                   |
| `dialog`             | Always default — PAL3's SCE dialog state is not yet exposed |
| `world_map_open`     | Always `false` (PAL3 has no world map)                      |
| `script_running`     | `true` when `!adv_input_enabled` or the SCE proc stack is non-empty |
| `current_script_fn`  | Name of the proc on top of the SCE call stack, when running |
| `movie_playing`      | Always `false` for now (no SceVm hook yet)                  |
| `frame` / `fps` / `dt` / `paused` / `fast_forward` | Same as PAL4 (driven by the shared bridge) |

### Endpoint matrix

| Endpoint                              | PAL3 status        | Notes |
| ------------------------------------- | ------------------ | ----- |
| `GET  /v1/state`                      | **Supported**      | See snapshot semantics above |
| `GET  /v1/log/tail`                   | **Supported**      | Transport-layer, game-agnostic |
| `GET  /v1/screenshot`                 | **Supported**      | Reads back the last presented swapchain frame |
| `POST /v1/input/key`, `/v1/input/axis` | **Supported**     | Routed through the synthetic-input bridge |
| `POST /v1/time/pause` / `resume` / `step` | **Supported** | `AdventureDirector::update` honors `bridge.effective_dt` |
| `POST /v1/time/fast_forward`          | **Supported**      | `SceVm` reads the flag: dialog/movie waits are skipped and timed SCE tweens (camera/role/fade/quake) collapse to their final state in one frame |
| `POST /v1/player/teleport`            | **Supported**      | Teleports `GlobalState::role_controlled` and mirrors to `PersistentState` |
| `POST /v1/dialog/advance`             | **Supported**      | Synthesises a `Space` tap |
| `POST /v1/dialog/choose`              | **not_implemented**| Deferred; needs an injection point in the SCE dialog system. (Under fast-forward, PAL3 auto-picks the first option so runs don't stall.) |
| `POST /v1/save`                       | **Supported**      | `PersistentState::save` |
| `POST /v1/load`                       | **Supported**      | Rebuilds the `AdventureDirector` from the slot (PAL3 has no in-place restore, so the menu/in-game auto-route resolves to the same fresh boot) |
| `GET  /v1/script/globals`             | **Supported**      | Dense window over `PersistentState::get_global(i16)` |
| `POST /v1/menu/new_game` / `/v1/menu/exit` | **Supported** | Routed through `Pal3Service`; `--pal3 --agent-port` must have been the launch flag so the asset path is known |
| `POST /v1/script/eval`                | **not_implemented**| PAL3 has no AngelScript VM |
| `POST /v1/script/trace/*`             | **not_implemented**| No `SceVm` trace adapter yet |
| `GET  /v1/scene/triggers` / `objects` | **not_implemented**| Deferred — PAL3 enumerates triggers as SCE proc entries, not EVF |
| `POST /v1/scene/fire_trigger`         | **not_implemented**| Deferred — will route to `SceVm::call_proc_by_name` |
| `POST /v1/object/interact`            | **not_implemented**| PAL3 has no GOB `research_function` analog |
| `POST /v1/world_map/choose`           | **not_implemented**| PAL3 has no world-map prompt |
| `GET  /v1/perf`                       | **Supported**      | Shared with PAL4 (`radiance::perf` snapshot) |

### Differences from PAL4 you should know about

* **Mode-control**: PAL3 has no separate start-menu director that the
  agent dispatcher must respect — the start menu is a scripted overlay
  that hands off to `AdventureDirector` on click. `EnterNewGame` and
  `EnterLoadGame` install a fresh adventure director directly on the
  scene manager, skipping the menu.
* **Save slots**: PAL3 uses slots 1..=4 (matching the legacy quick-save
  layout). PAL4's slot range is wider — agents that wrap both should
  probe `IPal3Service::save_slot_count` (or trust the documented `4`).
* **`script_running` semantics**: PAL4 derives this from the
  AngelScript VM call stack. PAL3 ORs the SCE proc stack with the
  inverse of `adv_input_enabled`, so dialogues and movies (which
  disable adv input) read as `script_running = true` even when the
  underlying VM is in a `Yield` waiting for input.
