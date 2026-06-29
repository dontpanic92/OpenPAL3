# OpenPAL3 / Yaobow — Copilot instructions

OpenPAL3 (binary name **`yaobow`**) is an open-source reimplementation of
*Chinese Paladin 3* and other Softstar/Aurogon RPGs (PAL3/3A/4/5/5Q, SWD5/HC/CF,
Gujian 1/2). The codebase is a Cargo workspace written in Rust, with a custom
COM-style runtime (`crosscom`), a 3D engine (`radiance`), and a custom
scripting language (`p7` / **protosept**) used for high-level UI and game
glue.

## Repository layout

| Crate / dir                      | Purpose |
| -------------------------------- | ------- |
| `crosscom/idl/*.idl`             | Source of truth for all interface definitions. Edits here regenerate Rust + p7 bindings via build scripts. |
| `crosscom/ccidl-rs`              | IDL compiler used by every `build.rs`. Run standalone: `cargo run -p crosscom-ccidl -- crosscom/idl/radiance.idl` |
| `crosscom/runtime/rust`          | Rust-side `crosscom` runtime: `ComRc<I>`, `ComObject_*!` macros, fat CCWs, QI. |
| `crosscom/runtime/protosept`     | Bridges `crosscom` ↔ the p7 scripting VM (foreign boxes, dispatcher, adapter). |
| `radiance/radiance`              | Engine: rendering (Vulkan), scene graph, input, imgui, audio, video, math. |
| `radiance/radiance_editor`       | Editor-facing host UI (imgui-based). |
| `radiance/radiance_scripting`    | `ScriptHost`, runtime services, p7 ↔ engine bridge proxies. |
| `radiance/protosept` (submodule) | The `p7` language: lexer, parser, semantic, bytecode interpreter, builtins. Language specs live in `radiance/protosept/specs/`. |
| `yaobow/agent_server`            | Optional HTTP+JSON automation server (PAL4, PAL3, PAL3A, PAL5). Transport-agnostic; no engine deps. |
| `yaobow/shared`                  | All per-game logic (`openpal3`, `openpal4`, …), loaders, scripting glue, UI widgets. |
| `yaobow/fileformats`             | Pure-Rust decoders for game asset formats (cpk, sce, dff, gob, …). |
| `yaobow/yaobow`                  | The `yaobow` binary (title selector + per-game entry points). |
| `yaobow/yaobow_editor`           | The `yaobow_editor` binary (asset/scene browser, p7-scripted UI). |
| `tools/*`                        | Standalone CLIs (`pol_exporter`, `repacker`, …). |
| `docs/`                          | Build instructions and the PAL4 `agent_interface.md` reference. |

`generated/*.md` are AI-generated analysis notes — informative, not authoritative.

## Build, test, lint

* **Build everything:** `cargo build --workspace` (release: `--release`). CI runs `cargo build --workspace --release --verbose` on Windows/Linux/macOS.
* **Targeted check (preferred during iteration):**
  `cargo check -p <crate> --all-targets` — much faster than a full build. Useful crates to scope changes to: `radiance`, `radiance_scripting`, `shared`, `yaobow_editor`, `yaobow_lib`, `agent_server`, `fileformats`, `crosscom-protosept`.
* **Run all tests in a crate:** `cargo test -p <crate>`
* **Run a single test:** `cargo test -p <crate> <test_fn_or_substring>` (e.g. `cargo test -p crosscom-protosept proto_ccw_e2e`). Tests live in each crate's `tests/` directory and in `#[cfg(test)]` modules.
* **Format:** `cargo fmt --all` (project pins `newline_style = "Unix"` in `rustfmt.toml`).
* **No project-wide clippy gate.** Don't introduce one as part of a feature change.
* **PSVita:** requires Rust nightly + `cargo-make`; build via `cd yaobow/yaobow && cargo make vpk`. Don't touch the Vita target from a non-Vita workstation unless the task explicitly requires it.

### System prerequisites (host-side)

Vulkan SDK + `nasm` + ffmpeg via `vcpkg install --triplet=<triplet>` (triplet per `docs/BUILD_INSTRUCTIONS.md`). On macOS: `brew install nasm molten-vk vulkan-headers vulkan-loader shaderc openal-soft`. The repo uses **git submodules** (`radiance/protosept`, asset repos); clone with `--recursive` or run `git submodule update --init --recursive`.

### Running the binary on macOS (Vulkan + OpenAL dyld paths)

`libvulkan.dylib` (vulkan-loader) is symlinked into `/opt/homebrew/lib`, but **`openal-soft` is keg-only on Homebrew** — `brew install openal-soft` puts `libopenal.dylib` under `/opt/homebrew/opt/openal-soft/lib/` with **no symlink** into `/opt/homebrew/lib`. So even with the package installed, the dynamic loader still won't find it unless that keg-specific path is on `DYLD_LIBRARY_PATH`. macOS SIP also strips `DYLD_*` from non-interactive shells, so `DYLD_LIBRARY_PATH=… ./yaobow` **does not** propagate when launched from a plain bash one-liner.

> Note: when OpenAL is missing the panic message mentions `OpenAL32.dll`. That's misleading — `alto`'s `load_default()` walks a fallback list (`libopenal.so` → `libopenal.dylib` → `soft_oal.dll` → `OpenAL32.dll`) and only reports the last name. The fix on macOS is still `libopenal.dylib` via Homebrew + the keg path below. (See `al-sys/src/lib.rs` in the `alto` checkout.)

Two reliable launch patterns:

1. **Interactive zsh wrapper** — sources `~/.zshrc`, which must export both prefixes so the keg-only `openal-soft` is reachable:
   ```sh
   # in ~/.zshrc
   export DYLD_LIBRARY_PATH="$(brew --prefix)/lib:/opt/homebrew/opt/openal-soft/lib/:$DYLD_LIBRARY_PATH"
   ```
   Then launch via an interactive zsh so the var actually reaches the child:
   ```bash
   nohup zsh -i -c 'cd target/debug; exec ./yaobow --pal4 --agent-port 8765' \
        > /tmp/pal4.log 2>&1 &
   ```
2. **Symlink the libs next to the binary** (no env vars needed):
   ```bash
   ln -sf /opt/homebrew/lib/libvulkan.dylib                 target/debug/libvulkan.dylib
   ln -sf /opt/homebrew/opt/openal-soft/lib/libopenal.dylib target/debug/libopenal.dylib
   ```

The Vulkan ICD (`/opt/homebrew/etc/vulkan/icd.d/MoltenVK_icd.json`) is auto-discovered once the loader is found; no need to set `VK_ICD_FILENAMES`.

## Architecture: the three runtimes

Working on this codebase almost always means crossing one of these boundaries:

1. **Rust ↔ crosscom (COM-like FFI).**
   * Every interface lives in `crosscom/idl/*.idl`. Editing an IDL regenerates Rust scaffolding into `OUT_DIR` (`*_comdef.rs`); **do not hand-edit generated `comdef` files** — they're not committed.
   * Rust impls use the `ComObject_*!` macros from `crosscom`. Objects are reference-counted via `ComRc<I>`. Cross-interface navigation uses `query_interface::<IFoo>()`.
   * Engine-only "inherent" extensions live as `IFooExt` traits (e.g. `IApplicationExt`, `ISceneExt`) re-exported from each crate's `comdef` module — they replace what used to be IDL `[rust()]` accessors.

2. **crosscom ↔ p7 (script bridge).**
   * Interfaces marked `[protosept(scriptable)]` in IDL get a generated p7 binding (`*_p7`) plus a Rust bridge (`*_bridge.rs`) under each crate's `script_bridges` module. See `yaobow/shared/build.rs` for the three-call pattern (`generate_comdef` / `generate_p7` / `generate_script_bridge`).
   * Scripts receive Rust ComRcs as "foreign boxes" via `ScriptHost::intern` + `ScriptHost::foreign_box(type_tag, id)`. `type_tag` must match the IDL-derived module path (e.g. `radiance.comdef.IEntity`, `shared.openpal4.comdef.IPal4GameContext`).
   * Returned p7 boxes are reverse-wrapped with helpers like `wrap_director`, `wrap_overlay`, `wrap_actor_controller`. A fat CCW dispatches QI by the script struct's `conforming_to` set, so the box's static p7 type doesn't have to match the receiving Rust interface.
   * Marshalling a string argument across the COM boundary copies it (`CString::new`) every call. For large/per-frame text, **register once and refer by handle/key** (see `IUiHost::set_text_buffer` / `show_text_buffer`).

3. **AngelScript VM (game-script bytecode, `yaobow/shared/src/scripting/angelscript`).**
   * Used to execute PAL4 (and earlier games') original `.sce` scripts. Distinct from p7; same crate just to keep VM details in one place.
   * **Calling convention pitfall:** every registered system fn must `stack_pop` exactly its declared args and `stack_push` its return. There is no generic `CALLSYS` arg cleanup; using `stack_peek` (no sp advance) leaks the value stack until `sp` underflows. Use the `as_params!` macro pattern in `global_context.rs`.
   * `fp` is a constant set once at VM init; only `sp` moves. There is no per-call frame to delimit args structurally.

## Per-game wiring (yaobow)

Each `GameType` (`PAL3/PAL3A/PAL4/PAL5/PAL5Q/SWD5/SWDHC/SWDCF/Gujian/Gujian2`) has its own `app_name`, `config_key`, and (for PAL4/5/SWD5) a `DffLoaderConfig`. When adding a new title or modifying loader behaviour, update `yaobow/shared/src/lib.rs::GameType` consistently across `app_name`, `full_name`, `config_key`, `from_config_key`, `all`, and `dff_loader_config`.

The `yaobow` binary picks a game via CLI flag (`--pal3`, `--pal4`, …); with no flag it launches the p7-scripted title selector. Configuration lives in `~/Library/Application Support/yaobow/yaobow.toml` (macOS), `%APPDATA%\yaobow\yaobow.toml` (Windows), `~/.config/yaobow/yaobow.toml` (Linux); `YAOBOW_CONFIG` env var overrides the path. Save slots live under `<save_dir>/<app_name>/Save/`.

## p7 / protosept conventions

* Source files live alongside each crate's Rust code: `yaobow/yaobow/scripts/*.p7`, `yaobow/yaobow_editor/scripts/*.p7`. They are loaded via `include_str!` and registered with `ScriptHost::add_binding(name, src)` before `load_source`.
* Top-level exported constants use `pub let NAME: type = value;` — there is **no `pub const`** in p7.
* `import <module>;` resolves against a dedicated script `AssetManager` carried by `ScriptHost`. Each crate's `<crate>.ypk` is mounted at `/<crate>/` (engine bindings live at `/`); the VFS-backed `ScriptVfsProvider` translates `a.b.c` to `/a/b/c.p7`. Build with `script_package::pack` and mount via `<crate>::mount_scripts(&AssetManager)` at boot. The top-level composer is `yaobow_lib::script_source::install_script_assets`.
* Language reference: `radiance/protosept/specs/protosept-language.md`.

## Agent server (test/automation surface)

PAL4, PAL3, PAL3A and PAL5 can boot with an embedded HTTP+JSON server for headless automation:

```bash
yaobow --pal4 --agent-port 8765 [--agent-bind 127.0.0.1] [--agent-token <secret>]
yaobow --pal3 --agent-port 8765    # also: --pal3a, --pal5
```

Full PAL4 endpoint reference and Python/curl examples in `docs/agent_interface.md`. Key signals: `/v1/state` exposes `script_running` and `movie_playing` (use these, not `current_script_fn`, as the authoritative "engine busy" flag); `/v1/screenshot` returns a binary PNG of the last presented frame; `/v1/time/fast_forward` skips `giWait`, dialog waits, and movie playback. Commands are drained on the game thread; the transport stays single-threaded.

PAL3 dispatch (`yaobow/shared/src/openpal3/agent.rs::dispatch_pal3_command`) adds gameplay routes: `GET /v1/state`, `GET /v1/screenshot`, `POST /v1/menu/new_game`, `/v1/dialog/advance` (taps Space), teleport, save/load slots, and script globals. PAL3 has no in-place restore, so load is rebuilt from a slot via a fresh `AdventureDirector`.

When the agent server is enabled, `main.rs` swaps the global logger for a `TeeLogger` that fans every record into both `AgentLogSink` (drained by `/v1/log/tail`) and `SimpleLogger` (stdout). Anything that re-registers the logger after boot will break `/v1/log/tail`.

## Conventions and gotchas

* **Don't commit generated artifacts.** `*_comdef.rs`, `*_bridge.rs`, generated `.p7` bindings, and `vcpkg_installed/` all rebuild from sources in `OUT_DIR`.
* **Imgui themes are deferred.** `ImguiContext::apply_theme(name)` stashes the request; the style mutation runs at the top of the next `draw_ui`. This makes it safe to call from inside a frame (e.g. a menu callback) without triggering a `RefCell already borrowed` panic. Theme TOML files: `radiance/radiance/src/imgui/themes/*.toml`.
* **`SyntheticInputBridge`** (in `radiance/radiance/src/input/synthetic.rs`) OR-merges synthetic key/axis state with the real engine. Call `.end_frame()` each tick to clear pressed/released edges.
* **Frame readback:** `RenderingEngine::capture_last_frame()` returns `Option<CapturedFrame>` (Vulkan impl does BGRA→RGBA swap); returns `None` in headless or when no frame has been presented.
* **Contributor eligibility (from `CONTRIBUTING.md`):** anyone submitting code must affirm they have *not* worked at Softstar on PAL3 and have *not* seen internal materials (source, unreleased docs). Code is GPL-3.0. **Reverse-engineer formats clean-room from binary data only — do not reference external PAL3 reimplementations.**

## Reverse-engineering notes (verified)

* **PAL3 actor lighting:** characters use a high material ambient (~0.55), separate from the dim ~0.10 scene ambient (scenery only); shaders must lift ambient or roles render too dark. Original is per-vertex Lambert from 1–2 nearest point lights × MtlDiffuse + MtlAmbient (no N·L floor). `radiance/.../shaders/openpal3/pal3_actor.frag`.
* **PAL3 character shadow:** blob is basedata.cpk `/basedata/basedata/shadow.tga` (64×64 grayscale soft disc), rendered as a flat ground quad with `BlendMode::Multiply` (not AlphaBlend). `yaobow/shared/src/openpal3/scene/shadow.rs`.
* **PAL3A scn:** node records are 604 bytes (PAL3 = 620; tail 192 vs 208); roles 456 in both — `scn_loader.rs` branches on `GameType`. Scene assets live in shared `scn.cpk`/`sce.cpk` packs, not next to each scene cpk. UI atlas is `ui/UIArtist.plug` (not PAL3's `UI_opt.tli`).
* **PAL5 sun:** per-map sunX/Y/Z from `MapInfo.ini` (not envinfo.env). Scene objects dynamically lit via `SceneLighting.sun` → `set_sun`; buildings opt in via `DffLoaderConfig.dynamic_lighting`. Leaves (graded-alpha) cast shadows only with `MaterialParams.casts_shadow=true`.

## When in doubt

* For COM/IDL questions, read the IDL first, then the generated file under `target/.../build/<crate>-*/out/`.
* For p7 questions, read `radiance/protosept/specs/protosept-language.md` and the worked examples in `yaobow/yaobow/scripts/` / `yaobow/yaobow_editor/scripts/`.
* For agent-server protocol questions, `docs/agent_interface.md` is the spec.
* For game-specific data layout, check `yaobow/fileformats/src/<game>/` first — most formats have reverse-engineering notes inline.
