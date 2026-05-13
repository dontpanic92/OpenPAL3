# radiance_scripting

Scripting layer for radiance. **One** script runtime for the
application lifetime, **one** director lifecycle interface used by both script-
and Rust-implemented directors, and **one** transition mechanism: every
director swap is a return value from `dispatch` or `update`.

## Lifetime Rules

1. **`ScriptHost` is installed once on the radiance engine** (`ScriptHost::install(engine)`)
   and lives until the process exits. There is no second host, no swap, no per-screen runtime.
2. **Each user package loads exactly one script source** via `host.load_source(SRC)`.
   The package's main module must define a `pub fn init(host: box<IHostContext>) -> box<director.Director>`
   that returns the root director.
3. **The root director is rooted via an opaque `ScriptDirectorHandle`.** The
   `ScriptedDirector` Rust ComObject (`ScriptedDirector::wrap` /
   `ScriptedDirector::with_ui`) drives its lifecycle and drops the GC root on
   `Drop`. `IDirector::activate` and `IDirector::update` forward to the
   wrapped script director's proto methods.
4. **Transitions are return values.** Every `dispatch(command_id) -> array<box<Director>>`
   and `update(dt) -> array<box<Director>>` returns zero or one wrapped
   director. The proxy unwraps the array, replaces itself with a new
   `ScriptedDirector` over the returned director, and the previous one drops.
5. **Hot reload is not supported.** p7's `load_module` is append-only, and
   `ScriptHost::reload` (which would discard and rebuild interpreter state)
   cannot run inside a script call without panicking on `RefCell` reentry.
   Source changes require an application restart.

## Surfacing Rust-Implemented Directors

A Rust `ComObject` that implements `IDirector` can be returned to a script as
the next director by:

1. Interning it on the host: `let id = host.intern(rust_director);`.
2. Pushing it as a foreign box: `let b = host.foreign_box("radiance.comdef.IDirector", id)?;`.
3. Returning it from a host-service method (e.g. `IAppService::open_game`).

The receiving script then wraps it in a local `HostDirector` adapter and
returns it from `dispatch` / `update`. The adapter **must** be declared in
the user's main script module (the one passed to `host.load_source`) — p7's
proto-struct method dispatch keys on module-local type ids, so a
`HostDirector` defined in `director.p7` (the shared bindings module) would
not dispatch from cross-module callers. The canonical adapter shape is in
the doc comment at the top of `bindings/director.p7`; the editor's
`welcome.p7` is the reference implementation.

## File Map

| Path | Role |
| --- | --- |
| `bindings/director.p7` | The `Director` proto + adapter pattern doc |
| `bindings/ui.p7` | `UiNode` constructors used in `render` |
| `src/runtime.rs` | `ScriptHost`, `ScriptDirectorHandle`, `RuntimeServices` |
| `src/proxies/scripted_director.rs` | `ScriptedDirector` ComObject |
| `src/services/` | `HostContext`, `GameRegistry`, `InputService`, `AudioService`, `TextureService`, `VfsService` |
| `src/ui_walker/` | Owned `UiNode` resolver + imgui walker |
| `tests/runtime_smoke.rs` | `ScriptHost` lifecycle round-trips |
| `tests/services_smoke.rs` | Typed host-service contracts |
| `tests/foreign_director_smoke.rs` | Rust→script director surfacing |

## What's Deliberately Not Here

- **No `CommandBus` / `ICommandBus` / `CommandRouter`.** Side-channel routing of
  host actions is replaced by typed methods on host services. To trigger a
  Rust action that produces a next director, add a method to the relevant
  `I*Service` returning `IDirector?` and have the script call it directly.
- **No reset / generation counters in the public API.** The interpreter
  state is append-only; if you need to discard rooted handles, drop the
  director ComObjects that own them (their `Drop` unroots).
- **No top-level free-function lifecycle.** Every screen is a struct
  implementing `director.Director`. Free functions are only entry points
  (`init`) and helpers.
