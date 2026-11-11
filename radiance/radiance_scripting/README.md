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
   that returns the root director box. A screen director's underlying struct
   typically also conforms to `radiance.IUiLayer` (a sibling of
   `radiance.IDirector`, not a base) so the engine can draw it.
3. **The root director is reverse-wrapped via `wrap_director`** into a
   `ComRc<IDirector>`. The runtime-typed CCW factory in
   `crosscom-protosept` (see `proto_ccw.rs`) hands out a vtable backed by
   libffi closures that re-enter the interpreter on each method call.
   QI'ing to `ComRc<IUiLayer>` returns the same CCW (its
   `additional_query_uuids` list includes `IUiLayer::INTERFACE_ID` when the
   struct conforms to it).
4. **Transitions are return values.** Every
   `update(dt) -> ?box<radiance.IDirector>` returns either `null` (stay)
   or a bare box for the next director. The CCW's libffi thunk recursively
   `wrap_proto_unknown`s the returned box, so the engine receives a fresh
   `ComRc<IDirector>` for the next director.
5. **UI rendering is decoupled from directors.** The engine owns a UI-layer
   stack ([`UiLayerStack`](../radiance/src/radiance/ui_layer.rs)) and drives
   every registered `radiance.IUiLayer`'s `render(ui, dt)` once per frame in
   z-order band order, via the installed
   [`UiFrameRenderer`](../radiance/src/radiance/ui_frame.rs).
   `radiance_scripting::ImguiUiFrameRenderer` is the production implementation:
   it parks one `ImguiFrameState` around the whole layer pass inside the
   engine's imgui scope. The active director is auto-bridged into the `Scene`
   band when it also conforms to `IUiLayer`; standalone surfaces register via
   `UiManager::register_ui_layer` and hold the returned RAII handle.
6. **`deactivate` fires as a first-class `IDirector` method** declared in
   `radiance.idl`, invoked exactly once by `ISceneManager` before the old
   director is replaced. Dropping the final `ComRc<IDirector>` is pure
   unrooting — no implicit release-hook dispatch.
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
that implements `radiance.IUiLayer` and `radiance.IDirector` and forwards
`activate` / `update` to the wrapped Rust `IDirector`, with `render` a no-op.
The canonical adapter shape lives in `yaobow_editor/scripts/editor_consts.p7`.

## Managed References

`radiance/protosept` is pinned by the repository's submodule gitlink to the
remote `dontpanic92-managed-references` branch. Use
`git submodule update --init --recursive` to reproduce that exact revision;
do not use `--remote` for a reproducible build. Cargo uses this submodule's
`p7` path dependency. Rebuild and restart after updating it so scripts and
bindings are recompiled with the new reference lowering.

IDL interfaces marked `[protosept(scriptable)]` generate retained `self: &I`
receivers. Implement these as `self: &Self`: UI callbacks may retain that
receiver, unlike scoped `ref<Self>` / `refmut<Self>` views. Host-only methods
still use scoped mutable receivers.

Shared editor state can live directly in the director. Instead of storing
`project_panel: box<ProjectPanelState>` and passing that box, the editor stores
`project_panel: ProjectPanelState` and passes `&self.project_panel` to functions
taking `state: &ProjectPanelState`. Callbacks retain the existing field location
and its owner; they do not capture a fresh copy of the panel.

Keep `box(...)` where fresh identity or foreign ownership is intended. Director
factories still return fresh `box<IDirector>` values; `foreign_box` still owns
the interned COM reference. External callbacks are rooted by their CCW until its
final `ComRc` is dropped. A raw Rust `Data` clone alone is not a GC root.

## File Map

| Path | Role |
| --- | --- |
| `src/runtime.rs` | `ScriptHost`, `ScriptDirectorHandle`, `RuntimeServices` |
| `src/proxies/ui_frame_renderer.rs` | `ImguiUiFrameRenderer` (production UI renderer) |
| `src/script_bridges/` (auto-generated) | `wrap_director` / `wrap_ui_layer` / `register_*_proto` from `[protosept(scriptable)]` IDLs |
| `src/services/` | `HostContext`, `GameRegistry`, `InputService`, `AudioService`, `TextureService`, `VfsService`, `ImguiUiHost`, `RecordingUiHost`, `with_ui_host` + `UiManagerImmediateExt` (immediate-mode UI on the engine-owned texture cache) |
| `tests/runtime_smoke.rs` | `ScriptHost` lifecycle round-trips |
| `tests/services_smoke.rs` | Typed host-service contracts |
| `tests/ui_host_smoke.rs` | `IUiHost` recording + dispatcher plumbing |
| `tests/wrap_director_smoke.rs` | `wrap_director` activate/update/deactivate |
| `tests/ui_frame_renderer_smoke.rs` | `wrap_director` + `IUiLayer` QI + render dispatch |
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
  implementing `radiance.IDirector` (and, when it draws, also
  `radiance.IUiLayer`). Free functions are only entry points (`init`)
  and helpers.
- **No retained `UiNode` tree.** UI is immediate-mode: scripts call
  `IUiHost` methods directly from `IUiLayer::render`. SAM coercion turns
  p7 closures into `IAction` callbacks for pairing widgets (windows,
  tables, tab bars).
