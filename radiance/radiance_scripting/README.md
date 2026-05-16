# radiance_scripting

Scripting layer for radiance. **One** script runtime for the
application lifetime, **one** director lifecycle interface used by both script-
and Rust-implemented directors, and **one** transition mechanism: every
director swap is a return value from `update`.

## Lifetime Rules

1. **`ScriptHost` is installed once on the radiance engine** (`ScriptHost::install(engine)`)
   and lives until the process exits. There is no second host, no swap, no per-screen runtime.
2. **Each user package loads exactly one script source** via `host.load_source(SRC)`.
   The package's main module must define a `pub fn init(host: box<IHostContext>) -> box<radiance.IDirector>`
   that returns the root director box. The box's underlying struct must conform
   to `radiance.IImmediateDirector` (which inherits from `radiance.IDirector`).
3. **The root director is reverse-wrapped via `wrap_im_director`** into a
   `ComRc<IImmediateDirector>`. The runtime-typed CCW factory in
   `crosscom-protosept` (see `proto_ccw.rs`) hands out a vtable backed by
   libffi closures that re-enter the interpreter on each method call.
   QI'ing back to `ComRc<IDirector>` returns the same CCW (its
   `additional_query_uuids` list includes `IDirector::INTERFACE_ID`).
4. **Transitions are return values.** Every
   `update(dt) -> ?box<radiance.IDirector>` returns either `null` (stay)
   or a bare box for the next director. The CCW's libffi thunk recursively
   `wrap_proto_unknown`s the returned box, so the engine receives a fresh
   `ComRc<IDirector>` for the next director.
5. **The engine pumps `render_im` separately** via
   [`ImmediateDirectorPump`](../radiance/src/radiance/immediate_pump.rs).
   `radiance_scripting::ImguiImmediateDirectorPump` is the production
   implementation: it parks `ImguiFrameState` around each per-frame
   `render_im(ui, dt)` call inside the engine's imgui scope.
6. **`deactivate` fires on final ComRc release** through the CCW's
   release-method hook (`ProtoSpec::release_method = Some("deactivate")`).
7. **Hot reload is not supported.** p7's `load_module` is append-only, and
   `ScriptHost::reload` (which would discard and rebuild interpreter state)
   cannot run inside a script call without panicking on `RefCell` reentry.
   Source changes require an application restart.

## Surfacing Rust-Implemented Directors

A Rust `ComObject` that implements `IDirector` can be returned to a script as
the next director by:

1. Interning it on the host: `let id = host.intern(rust_director);`.
2. Pushing it as a foreign box: `let b = host.foreign_box("radiance.comdef.IDirector", id)?;`.
3. Returning it from a host-service method (e.g. `IAppService::open_game`).

The receiving script wraps it in a local `HostDirectorIm`-style adapter
that implements `radiance.IImmediateDirector` and forwards `activate` /
`update` to the wrapped Rust `IDirector`, with `render_im` a no-op.
The canonical adapter shape lives in `yaobow_editor/scripts/editor_consts.p7`.

## File Map

| Path | Role |
| --- | --- |
| `src/runtime.rs` | `ScriptHost`, `ScriptDirectorHandle`, `RuntimeServices` |
| `src/proxies/imgui_pump.rs` | `ImguiImmediateDirectorPump` (production pump) |
| `src/proxies/wrap_director.rs` | `wrap_director` (plain `IDirector`) convenience |
| `src/proxies/wrap_im_director.rs` | `wrap_im_director` (`IImmediateDirector`) convenience |
| `src/services/` | `HostContext`, `GameRegistry`, `InputService`, `AudioService`, `TextureService`, `VfsService`, `ImguiUiHost`, `RecordingUiHost`, `TextureResolver` |
| `tests/runtime_smoke.rs` | `ScriptHost` lifecycle round-trips |
| `tests/services_smoke.rs` | Typed host-service contracts |
| `tests/ui_host_smoke.rs` | `IUiHost` recording + dispatcher plumbing |
| `tests/wrap_director_smoke.rs` | `wrap_director` activate/update/deactivate |
| `tests/imgui_pump_smoke_v2.rs` | `wrap_im_director` + pump dispatch |
| `tests/proto_ccw_director.rs` | runtime-typed CCW for `radiance.IDirector` |
| `tests/script_handle_lifetime.rs` | Captured `ComRc<IAction>` across script calls |

## What's Deliberately Not Here

- **No `CommandBus` / `ICommandBus` / `CommandRouter`.** Side-channel routing of
  host actions is replaced by typed methods on host services. To trigger a
  Rust action that produces a next director, add a method to the relevant
  `I*Service` returning `IDirector?` and have the script call it directly.
- **No reset / generation counters in the public API.** The interpreter
  state is append-only; if you need to discard rooted handles, drop the
  director ComObjects that own them (their `Drop` unroots).
- **No top-level free-function lifecycle.** Every screen is a struct
  implementing `radiance.IImmediateDirector` (and, for transition return
  shapes, also `radiance.IDirector`). Free functions are only entry
  points (`init`) and helpers.
- **No retained `UiNode` tree.** UI is immediate-mode: scripts call
  `IUiHost` methods directly from `render_im`. SAM coercion turns p7
  closures into `IAction` callbacks for pairing widgets (windows, tables,
  tab bars).
