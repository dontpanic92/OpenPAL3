//! PAL5 Lua command bridge: `extern "C"` trampolines, namespaced
//! registration, the dispatch harness, and the `__pal5_load`/
//! `__pal5_done` engine hooks.
//!
//! PAL5's script API is table-namespaced (`global.Wait`, `npc.Create`,
//! …) and coroutine-driven (`global.Wait` / `WaitForCameraLerp` yield
//! the script thread). Because Lua 5.0 cannot `yield` across a C-call
//! boundary, `Include`/`CallScript` are implemented in the Lua harness
//! (so the dispatched script's inner `Wait` stays a pure Lua→Lua call),
//! and only the leaf commands are C functions.

use std::cell::RefCell;

use lua50_32_sys::lua_State;

use shared::scripting::lua50_32::Lua5032Vm;

use super::context::Pal5ScriptContext;

/// Lua dispatch harness. Loaded first, before `NewGame`. `global` (and
/// the other namespace tables) already exist here because the C command
/// registration created them. `__pal5_load` returns the loaded script's
/// entry function (or nil), so `CallScript` invokes it as a Lua→Lua call
/// — keeping the inner `Wait` yield legal.
const HARNESS: &str = r#"
function global.Include(id)
  __pal5_load(id)
end

function global.CallScript(id)
  local f = __pal5_load(id)
  if f then return f() end
end

function __pal5_main()
  NewGame()
  __pal5_done()
end
"#;

macro_rules! borrow_ctx {
    ($state:ident) => {{
        let v = lua50_32_sys::lua_touserdata($state, lua50_32_sys::LUA_GLOBALSINDEX - 1);
        &*(v as *const RefCell<Pal5ScriptContext>)
    }};
}

/// Register a namespaced command that maps to a `Pal5ScriptContext`
/// method. Two arms: void (`cmd!(vm, "ns", "Name", rust_fn, a: number)`)
/// and numeric-return (`cmd!(vm, "ns", "Name", rust_fn, ... -> num)`).
macro_rules! cmd {
    ($vm:ident, $ns:expr, $lua:expr, $rust:ident $(, $p:ident : $t:ident)*) => {
        paste::paste! {
            extern "C" fn [<__pal5_ $rust>](state: *mut lua_State) -> i32 {
                unsafe {
                    let context = borrow_ctx!(state);
                    $(
                        let $p = lua50_32_sys::[<lua_to $t>](state, 1);
                        lua50_32_sys::lua_remove(state, 1);
                    )*
                    context.borrow_mut().$rust($($p),*);
                }
                0
            }
            $vm.register_namespaced($ns, $lua, Some([<__pal5_ $rust>]));
        }
    };
    ($vm:ident, $ns:expr, $lua:expr, $rust:ident $(, $p:ident : $t:ident)* => num) => {
        paste::paste! {
            extern "C" fn [<__pal5_ $rust>](state: *mut lua_State) -> i32 {
                unsafe {
                    let context = borrow_ctx!(state);
                    $(
                        let $p = lua50_32_sys::[<lua_to $t>](state, 1);
                        lua50_32_sys::lua_remove(state, 1);
                    )*
                    let ret = context.borrow_mut().$rust($($p),*);
                    lua50_32_sys::lua_pushnumber(state, ret);
                }
                1
            }
            $vm.register_namespaced($ns, $lua, Some([<__pal5_ $rust>]));
        }
    };
}

/// Logged no-op for commands not yet implemented for the bootstrap.
extern "C" fn pal5_stub(_state: *mut lua_State) -> i32 {
    0
}

/// `global.Wait(sec)` — yields `sec` to the driver as the sleep time.
extern "C" fn pal5_wait(state: *mut lua_State) -> i32 {
    unsafe {
        let delay = lua50_32_sys::lua_tonumber(state, 1);
        lua50_32_sys::lua_remove(state, 1);
        lua50_32_sys::lua_pushnumber(state, delay);
        lua50_32_sys::lua_yield(state, 1)
    }
}

/// `global.WaitForCameraLerp()` — yields the remaining camera-lerp time.
extern "C" fn pal5_wait_camera_lerp(state: *mut lua_State) -> i32 {
    unsafe {
        let context = borrow_ctx!(state);
        let remaining = context.borrow().camera_lerp_remaining();
        lua50_32_sys::lua_pushnumber(state, remaining as f64);
        lua50_32_sys::lua_yield(state, 1)
    }
}

/// `__pal5_done()` — flags the story as finished (the coroutine returns
/// right after, so it is never resumed again).
extern "C" fn pal5_done(state: *mut lua_State) -> i32 {
    unsafe {
        let context = borrow_ctx!(state);
        context.borrow_mut().mark_finished();
    }
    0
}

/// `__pal5_load(id)` — resolve, read (auto-decrypt) and execute a script
/// by id so its functions are defined on the shared state, then push and
/// return its entry function (or nil). Used by the harness'
/// `Include`/`CallScript`.
extern "C" fn pal5_load(state: *mut lua_State) -> i32 {
    unsafe {
        let context = borrow_ctx!(state);
        let id = lua50_32_sys::lua_tonumber(state, 1) as u32;
        lua50_32_sys::lua_remove(state, 1);

        let loaded = {
            let c = context.borrow();
            c.script_index().load_source(c.asset_loader().vfs(), id)
        };
        let (name, source) = match loaded {
            Ok(x) => x,
            Err(e) => {
                log::error!("PAL5: __pal5_load({}) failed: {}", id, e);
                lua50_32_sys::lua_pushnil(state);
                return 1;
            }
        };

        let cname = std::ffi::CString::new(name.clone()).unwrap();
        let chunk_name = std::ffi::CString::new(format!("script:{}", name)).unwrap();
        let ret = lua50_32_sys::luaL_loadbuffer(
            state,
            source.as_ptr() as *const std::os::raw::c_char,
            source.len(),
            chunk_name.as_ptr(),
        );
        if ret > 0 {
            log::error!("PAL5: loadbuffer({}) failed: {}", name, ret);
            lua50_32_sys::lua_pushnil(state);
            return 1;
        }
        // Execute the chunk to define its functions (no yield here).
        let call = lua50_32_sys::lcall(state, 0, 0);
        if call > 0 {
            log::error!("PAL5: exec({}) failed: {}", name, call);
            lua50_32_sys::lua_pushnil(state);
            return 1;
        }

        // Push the entry function (file stem); nil if there is none
        // (e.g. the `macro` include defines many helpers, no `macro()`).
        lua50_32_sys::lgetglobal(state, cname.as_ptr());
        if lua50_32_sys::lua_type(state, -1) != lua50_32_sys::LUA_TFUNCTION as i32 {
            lua50_32_sys::lua_settop(state, lua50_32_sys::lua_gettop(state) - 1);
            lua50_32_sys::lua_pushnil(state);
        }
        1
    }
}

/// Build the PAL5 VM: register every first-segment command (essentials
/// real, the rest logged stubs), then load the dispatch harness. The
/// caller loads `NewGame` and sets `__pal5_main` as the entry.
pub fn create_lua_vm(
    context: std::rc::Rc<RefCell<Pal5ScriptContext>>,
) -> anyhow::Result<Lua5032Vm<Pal5ScriptContext>> {
    let vm = Lua5032Vm::create(context);

    // Engine hooks used by the harness.
    vm.register("__pal5_load", Some(pal5_load));
    vm.register("__pal5_done", Some(pal5_done));

    // Coroutine yields.
    vm.register_namespaced("global", "Wait", Some(pal5_wait));
    vm.register_namespaced("global", "WaitForCameraLerp", Some(pal5_wait_camera_lerp));

    // ---- global ----
    cmd!(vm, "global", "Print", global_print, t: string);
    cmd!(vm, "global", "BeginScene", global_begin_scene, a: number);
    cmd!(vm, "global", "EndScene", global_end_scene);
    cmd!(vm, "global", "SetWideScreen", global_set_wide_screen, a: number);
    cmd!(vm, "global", "PlayMusic", global_play_music, a: number, b: number);
    cmd!(vm, "global", "GetMusicID", global_get_music_id => num);
    cmd!(vm, "global", "MusicFadeIn", global_music_fade_in, a: number);
    cmd!(vm, "global", "MusicFadeOut", global_music_fade_out, a: number);
    cmd!(vm, "global", "PlaySound", global_play_sound, a: number, b: number);
    cmd!(vm, "global", "StopLastSound", global_stop_last_sound);
    cmd!(vm, "global", "PlayCg", global_play_cg, a: number);

    // ---- flag ----
    cmd!(vm, "flag", "SetValue", flag_set_value, a: number, b: number);
    cmd!(vm, "flag", "GetValue", flag_get_value, a: number => num);

    // ---- player ----
    cmd!(vm, "player", "Create", player_create, a: number, b: number, c: number);
    cmd!(vm, "player", "SetPos", player_set_pos, a: number, b: number);
    cmd!(vm, "player", "SetVisible", player_set_visible, a: number, b: number);
    cmd!(vm, "player", "Remove", player_remove, a: number);
    cmd!(vm, "player", "IsPlayerInTeam", player_is_in_team, a: number => num);
    cmd!(vm, "player", "GetItemCount", player_get_item_count, a: number => num);

    // ---- npc ----
    cmd!(vm, "npc", "Create", npc_create, a: number, b: number, c: number, d: number);
    cmd!(vm, "npc", "SetPos", npc_set_pos, a: number, b: number, c: number);
    cmd!(vm, "npc", "SetPos3D", npc_set_pos_3d, a: number, b: number, c: number, d: number);
    cmd!(vm, "npc", "MoveTo", npc_move_to, a: number, b: number, c: number);
    cmd!(vm, "npc", "RunTo", npc_run_to, a: number, b: number, c: number);
    cmd!(vm, "npc", "SetVisible", npc_set_visible, a: number, b: number);
    cmd!(vm, "npc", "Destroy", npc_destroy, a: number);
    cmd!(vm, "npc", "IsCreated", npc_is_created, a: number => num);

    // ---- camera ----
    cmd!(vm, "camera", "ChangeCameraStatic", camera_change_static,
        a: number, b: number, c: number, d: number, e: number, f: number);
    cmd!(vm, "camera", "ChangeCameraStaticEye", camera_change_static_eye,
        a: number, b: number, c: number, d: number, e: number, f: number);
    cmd!(vm, "camera", "ResetLerp", camera_reset_lerp, a: number);

    // ---- effect ----
    cmd!(vm, "effect", "FadeIn", effect_fade_in, a: number, b: number);
    cmd!(vm, "effect", "FadeOut", effect_fade_out, a: number, b: number);

    // ---- ui ----
    cmd!(vm, "ui", "Dialog", ui_dialog, t: string);
    cmd!(vm, "ui", "Message", ui_message, t: string);
    cmd!(vm, "ui", "CloseStartMenu", ui_close_start_menu);

    // ---- map ----
    cmd!(vm, "map", "ChangeNoScript", map_change_no_script, a: number, b: number);
    cmd!(vm, "map", "GetCurrentMapID", map_get_current_map_id => num);

    // ---- stubs (logged no-ops for the bootstrap) ----
    for (ns, name) in STUBS {
        vm.register_namespaced(ns, name, Some(pal5_stub));
    }

    vm.load_chunk(HARNESS.as_bytes(), "pal5_harness")?;
    Ok(vm)
}

/// Commands registered as no-ops for the first-segment bootstrap. These
/// either have no visible effect for the intro (item/magic grants,
/// patrol AI) or are deferred (movies, anim chains, camera paths). They
/// MUST still be registered so the scripts don't hit `call nil`.
const STUBS: &[(&str, &str)] = &[
    // global waits whose conditions are already satisfied (NPC moves
    // and anims resolve instantly in the bootstrap) or are deferred.
    ("global", "WaitForNpcPos"),
    ("global", "WaitForNpcPos3D"),
    ("global", "WaitForNpcAnim"),
    ("global", "WaitForNpcTurn"),
    ("global", "WaitForCgEnd"),
    // player grants / control.
    ("player", "AddItem"),
    ("player", "AddMagic"),
    ("player", "AddEquip"),
    ("player", "AddFormula"),
    ("player", "Control"),
    ("player", "Stop"),
    ("player", "SetAt"),
    ("player", "ChangeMP"),
    ("player", "ChangeHP"),
    // npc behaviour not visible in a single static frame.
    ("npc", "SetAt"),
    ("npc", "SetAtPos"),
    ("npc", "SetAnim"),
    ("npc", "AddAnimChain"),
    ("npc", "TurnTo"),
    ("npc", "TurnToNpc"),
    ("npc", "TurnToPos"),
    ("npc", "AddPatrolPoint"),
    ("npc", "SetPatrolType"),
    ("npc", "SetSpeed"),
    ("npc", "FloatTo"),
    ("npc", "CreateSE"),
    ("npc", "CreateObject"),
    ("npc", "CreateChest"),
    // camera extras.
    ("camera", "ChangeCameraPath"),
    ("camera", "ChangeCameraStaticToNpc"),
    ("camera", "Save"),
    ("camera", "Resume"),
    ("camera", "Shake"),
    // map / effect / ui extras.
    ("map", "AddEvent"),
    ("map", "CreateNameSE"),
    ("map", "Change"),
    ("effect", "SetFilterTexture"),
    ("ui", "SetDialogFontSize"),
    ("ui", "MirrorPic"),
    ("ui", "Dialog_t"),
    ("ui", "AddQuest"),
];
