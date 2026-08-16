use std::{cell::RefCell, os::raw::c_char, rc::Rc};

use anyhow::bail;
use encoding::{DecoderTrap, Encoding};
use lua50_32_sys::lua_State;

pub struct Lua5032Vm<TContext> {
    // The Lua VM keeps raw pointers into this buffer; the field must stay
    // alive for the lifetime of `lua` even though no Rust code reads it.
    #[allow(dead_code)]
    lib: Vec<u8>,
    lua: *mut lua_State,
    thread: *mut lua_State,
    context: Rc<RefCell<TContext>>,
}

impl<TContext> Lua5032Vm<TContext> {
    pub fn new(
        lib: Vec<u8>,
        function: &str,
        context: Rc<RefCell<TContext>>,
    ) -> anyhow::Result<Self> {
        unsafe {
            let lua = lua50_32_sys::lua_open();
            lua50_32_sys::luaopen_base(lua);
            lua50_32_sys::luaopen_table(lua);
            lua50_32_sys::luaopen_io(lua);
            lua50_32_sys::luaopen_string(lua);
            lua50_32_sys::luaopen_math(lua);
            lua50_32_sys::luaopen_debug(lua);
            lua50_32_sys::luaopen_loadlib(lua);

            let ret = lua50_32_sys::luaL_loadbuffer(
                lua,
                lib.as_ptr() as *const c_char,
                lib.len(),
                b"main\0".as_ptr() as *const c_char,
            );

            if ret > 0 {
                bail!("luaL_loadbuffer failed: {}", ret);
            }

            let call_ret = lua50_32_sys::lcall(lua, 0, 0);
            if call_ret > 0 {
                bail!(get_error(lua));
            }

            let thread = lua50_32_sys::lua_newthread(lua);
            let cname = std::ffi::CString::new(function).unwrap();

            lua50_32_sys::lgetglobal(thread, cname.as_ptr());

            Ok(Self {
                lib,
                lua,
                thread,
                context,
            })
        }
    }

    /// Construct an empty VM (state + standard libs + a coroutine
    /// thread) without loading or entering any script. Use together
    /// with [`load_chunk`](Self::load_chunk),
    /// [`register`](Self::register) /
    /// [`register_namespaced`](Self::register_namespaced) and
    /// [`set_entry`](Self::set_entry) when the script set is built up
    /// incrementally (e.g. PAL5's `Include`/`CallScript` dispatch),
    /// rather than from a single pre-known main chunk + entry function
    /// like [`new`](Self::new).
    pub fn create(context: Rc<RefCell<TContext>>) -> Self {
        unsafe {
            let lua = lua50_32_sys::lua_open();
            lua50_32_sys::luaopen_base(lua);
            lua50_32_sys::luaopen_table(lua);
            lua50_32_sys::luaopen_io(lua);
            lua50_32_sys::luaopen_string(lua);
            lua50_32_sys::luaopen_math(lua);
            lua50_32_sys::luaopen_debug(lua);
            lua50_32_sys::luaopen_loadlib(lua);

            let thread = lua50_32_sys::lua_newthread(lua);

            Self {
                lib: Vec::new(),
                lua,
                thread,
                context,
            }
        }
    }

    /// Load + execute a Lua source chunk on the main state. Top-level
    /// statements run immediately (defining functions, building tables);
    /// it must not `yield`. Globals defined here are visible from the
    /// coroutine thread (threads share the global table in Lua 5.0).
    pub fn load_chunk(&self, src: &[u8], chunk_name: &str) -> anyhow::Result<()> {
        let cname = std::ffi::CString::new(chunk_name).unwrap();
        unsafe {
            let ret = lua50_32_sys::luaL_loadbuffer(
                self.lua,
                src.as_ptr() as *const c_char,
                src.len(),
                cname.as_ptr(),
            );
            if ret > 0 {
                bail!(
                    "luaL_loadbuffer({}) failed: {}",
                    chunk_name,
                    get_error(self.lua)
                );
            }

            let call_ret = lua50_32_sys::lcall(self.lua, 0, 0);
            if call_ret > 0 {
                bail!("chunk {} failed: {}", chunk_name, get_error(self.lua));
            }
        }
        Ok(())
    }

    /// Push the named global function onto the coroutine thread so the
    /// next [`execute`](Self::execute) resumes into it. Call exactly
    /// once, after all `register*` calls (which keep the thread stack
    /// balanced) and after the chunk that defines `function`.
    pub fn set_entry(&self, function: &str) -> anyhow::Result<()> {
        let cname = std::ffi::CString::new(function).unwrap();
        unsafe {
            lua50_32_sys::lgetglobal(self.thread, cname.as_ptr());
            if lua50_32_sys::lua_type(self.thread, -1) != lua50_32_sys::LUA_TFUNCTION as i32 {
                lua50_32_sys::lua_settop(self.thread, lua50_32_sys::lua_gettop(self.thread) - 1);
                bail!("entry function '{}' is not defined", function);
            }
        }
        Ok(())
    }

    pub fn register(&self, name: &str, func: lua50_32_sys::lua_CFunction) {
        let cname = std::ffi::CString::new(name).unwrap();

        unsafe {
            let p = self.context.as_ref() as *const _ as *mut _;
            lua50_32_sys::lua_pushlightuserdata(self.thread, p);
            lua50_32_sys::lua_pushcclosure(self.thread, func, 1);
            lua50_32_sys::lsetglobal(self.thread, cname.as_ptr());

            // lua50_32_sys::lregister(self.thread, cname.as_ptr(), func);
        }
    }

    /// Register a C function as a field of a global namespace table
    /// (`<namespace>.<name>`), creating the table on first use. PAL5's
    /// script API is table-namespaced (`global.Wait`, `npc.Create`, …),
    /// so flat [`register`](Self::register) is not enough. Leaves the
    /// thread stack balanced.
    pub fn register_namespaced(
        &self,
        namespace: &str,
        name: &str,
        func: lua50_32_sys::lua_CFunction,
    ) {
        let ns = std::ffi::CString::new(namespace).unwrap();
        let field = std::ffi::CString::new(name).unwrap();

        unsafe {
            let top = lua50_32_sys::lua_gettop(self.thread);

            // Fetch (or create) the namespace table on the globals.
            lua50_32_sys::lgetglobal(self.thread, ns.as_ptr());
            if lua50_32_sys::lua_type(self.thread, -1) != lua50_32_sys::LUA_TTABLE as i32 {
                lua50_32_sys::lua_settop(self.thread, top); // drop the nil
                lua50_32_sys::lua_newtable(self.thread); // [t]
                lua50_32_sys::lua_pushvalue(self.thread, -1); // [t, t]
                lua50_32_sys::lsetglobal(self.thread, ns.as_ptr()); // _G[ns]=t -> [t]
            }

            // table[name] = closure(func, upvalue = context ptr)
            lua50_32_sys::lua_pushstring(self.thread, field.as_ptr()); // [t, name]
            let p = self.context.as_ref() as *const _ as *mut _;
            lua50_32_sys::lua_pushlightuserdata(self.thread, p); // [t, name, ud]
            lua50_32_sys::lua_pushcclosure(self.thread, func, 1); // [t, name, closure]
            lua50_32_sys::lua_settable(self.thread, -3); // t[name]=closure -> [t]

            lua50_32_sys::lua_settop(self.thread, top); // drop the table
        }
    }

    pub fn execute(&self) -> anyhow::Result<f32> {
        unsafe {
            let ret = lua50_32_sys::lua_resume(self.thread, 0);
            if ret != 0 {
                bail!(get_error(self.thread));
            }

            let param = lua50_32_sys::lua_tonumber(self.thread, -1);
            Ok(param as f32)
        }
    }

    /// Enumerate the script's global table as `(name, value)` pairs,
    /// sorted by name.
    ///
    /// Walked on the **main state** (`self.lua`), never on
    /// `self.thread`: the coroutine is suspended mid-`sleep` with live
    /// values on its stack, and pushing a traversal key/value pair
    /// there would corrupt the resume point. This is safe because Lua
    /// 5.0 shares one globals table between a thread and its parent
    /// (`setobj2n(gt(L1), gt(L))` in `lstate.c`), so both states see
    /// the same names.
    ///
    /// Only string keys are reported. Values are marshalled for
    /// nil / boolean / number / string; anything else (function,
    /// table, userdata, thread) is reported as a `LuaValue::Other`
    /// type tag rather than recursed into. The stack is restored to
    /// its entry depth before returning.
    pub fn enumerate_globals(&self) -> Vec<(String, LuaValue)> {
        let mut out = Vec::new();

        unsafe {
            let l = self.lua;
            let top = lua50_32_sys::lua_gettop(l);

            lua50_32_sys::lua_pushnil(l);
            while lua50_32_sys::lua_next(l, lua50_32_sys::LUA_GLOBALSINDEX) != 0 {
                // Stack now holds [.., key, value]. Only read the key
                // when it is *already* a string: calling `lua_tostring`
                // on a number key would coerce it in place and break
                // the `lua_next` traversal.
                if lua50_32_sys::lua_type(l, -2) == lua50_32_sys::LUA_TSTRING as i32 {
                    if let Some(name) = read_lua_string(l, -2) {
                        out.push((name, read_lua_value(l, -1)));
                    }
                }

                // Pop the value, leave the key for the next iteration.
                lua50_32_sys::lua_settop(l, lua50_32_sys::lua_gettop(l) - 1);
            }

            // `lua_next` popped the final key itself; restore anyway so
            // an early bail can never leak stack slots.
            lua50_32_sys::lua_settop(l, top);
        }

        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// A marshalled Lua value read out of the global table by
/// [`Lua5032Vm::enumerate_globals`].
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Nil,
    Bool(bool),
    Number(f64),
    /// BIG5-decoded string (the SWD5-family scripts are BIG5-encoded).
    Str(String),
    /// Type tag for values the bridge deliberately does not walk
    /// (`"function"`, `"table"`, `"userdata"`, `"thread"`, …).
    Other(&'static str),
}

/// Read the string at `idx` (which must already be of type
/// `LUA_TSTRING`) and decode it from BIG5. Uses `lua_strlen` rather
/// than `CStr` so embedded NULs don't truncate the value.
fn read_lua_string(state: *mut lua_State, idx: i32) -> Option<String> {
    unsafe {
        let ptr = lua50_32_sys::lua_tostring(state, idx);
        if ptr.is_null() {
            return None;
        }

        let len = lua50_32_sys::lua_strlen(state, idx);
        let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
        Some(
            encoding::all::BIG5_2003
                .decode(bytes, DecoderTrap::Ignore)
                .unwrap_or_else(|s| s.into_owned()),
        )
    }
}

/// Marshal the value at `idx` into a [`LuaValue`].
fn read_lua_value(state: *mut lua_State, idx: i32) -> LuaValue {
    unsafe {
        let ty = lua50_32_sys::lua_type(state, idx) as u32;
        match ty {
            lua50_32_sys::LUA_TNIL => LuaValue::Nil,
            lua50_32_sys::LUA_TBOOLEAN => {
                LuaValue::Bool(lua50_32_sys::lua_toboolean(state, idx) != 0)
            }
            lua50_32_sys::LUA_TNUMBER => LuaValue::Number(lua50_32_sys::lua_tonumber(state, idx)),
            lua50_32_sys::LUA_TSTRING => {
                read_lua_string(state, idx).map_or(LuaValue::Nil, LuaValue::Str)
            }
            lua50_32_sys::LUA_TTABLE => LuaValue::Other("table"),
            lua50_32_sys::LUA_TFUNCTION => LuaValue::Other("function"),
            lua50_32_sys::LUA_TUSERDATA | lua50_32_sys::LUA_TLIGHTUSERDATA => {
                LuaValue::Other("userdata")
            }
            lua50_32_sys::LUA_TTHREAD => LuaValue::Other("thread"),
            _ => LuaValue::Other("unknown"),
        }
    }
}

impl<TContext> Drop for Lua5032Vm<TContext> {
    fn drop(&mut self) {
        unsafe {
            lua50_32_sys::lua_close(self.lua);
        }
    }
}

fn get_error(state: *mut lua_State) -> String {
    unsafe {
        let s = lua50_32_sys::lua_tostring(state, -1);
        let str = std::ffi::CStr::from_ptr(s);
        let str = encoding::all::BIG5_2003.decode(str.to_bytes(), DecoderTrap::Ignore);
        match str {
            Ok(str) => str,
            Err(str) => format!("{:?}", str),
        }
    }
}

#[cfg(vita)]
#[no_mangle]
pub extern "C" fn popen() {
    panic!("popen not supported on vita");
}

#[cfg(vita)]
#[no_mangle]
pub extern "C" fn pclose() {
    panic!("pclose not supported on vita");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vm_with(src: &str) -> Lua5032Vm<()> {
        let vm = Lua5032Vm::create(Rc::new(RefCell::new(())));
        vm.load_chunk(src.as_bytes(), "test").expect("chunk loads");
        vm
    }

    fn lookup(globals: &[(String, LuaValue)], name: &str) -> Option<LuaValue> {
        globals
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    #[test]
    fn enumerate_globals_marshals_each_scalar_type() {
        let vm = vm_with(
            r#"
            g_num = 42
            g_float = 1.5
            g_true = true
            g_false = false
            g_str = "hello"
            g_table = { 1, 2 }
            function g_fn() end
            "#,
        );

        let globals = vm.enumerate_globals();

        assert_eq!(lookup(&globals, "g_num"), Some(LuaValue::Number(42.0)));
        assert_eq!(lookup(&globals, "g_float"), Some(LuaValue::Number(1.5)));
        assert_eq!(lookup(&globals, "g_true"), Some(LuaValue::Bool(true)));
        assert_eq!(lookup(&globals, "g_false"), Some(LuaValue::Bool(false)));
        assert_eq!(
            lookup(&globals, "g_str"),
            Some(LuaValue::Str("hello".into()))
        );
        assert_eq!(lookup(&globals, "g_table"), Some(LuaValue::Other("table")));
        assert_eq!(lookup(&globals, "g_fn"), Some(LuaValue::Other("function")));
    }

    #[test]
    fn enumerate_globals_is_sorted_by_name() {
        let vm = vm_with("zzz = 1 aaa = 2 mmm = 3");
        let names: Vec<String> = vm
            .enumerate_globals()
            .into_iter()
            .map(|(n, _)| n)
            .filter(|n| ["aaa", "mmm", "zzz"].contains(&n.as_str()))
            .collect();

        assert_eq!(names, vec!["aaa", "mmm", "zzz"]);
    }

    #[test]
    fn enumerate_globals_decodes_big5_strings() {
        // 0xA4A4 is BIG5 for 中.
        let vm = vm_with(r#" g_zh = "\164\164" "#);
        assert_eq!(
            lookup(&vm.enumerate_globals(), "g_zh"),
            Some(LuaValue::Str("中".into()))
        );
    }

    #[test]
    fn enumerate_globals_leaves_the_stack_balanced() {
        let vm = vm_with("a = 1 b = 'two' c = { }");

        let depth_before = unsafe { lua50_32_sys::lua_gettop(vm.lua) };
        let first = vm.enumerate_globals();
        let depth_after = unsafe { lua50_32_sys::lua_gettop(vm.lua) };
        assert_eq!(depth_before, depth_after, "traversal must not leak slots");

        // Repeated traversals must be stable — a corrupted stack or a
        // coerced key would change the result on the second pass.
        let second = vm.enumerate_globals();
        assert_eq!(first, second);
    }

    #[test]
    fn numeric_keys_do_not_break_the_traversal() {
        // A number key in the globals table used to be a hazard: calling
        // `lua_tostring` on it coerces it in place and desynchronises
        // `lua_next`. Only string keys are read, so this must be stable.
        let vm = vm_with("x = 1 rawset(_G, 7, 'seven') y = 2");

        let globals = vm.enumerate_globals();
        assert_eq!(lookup(&globals, "x"), Some(LuaValue::Number(1.0)));
        assert_eq!(lookup(&globals, "y"), Some(LuaValue::Number(2.0)));
        assert!(
            globals.iter().all(|(n, _)| n != "7"),
            "numeric keys are skipped, not stringified"
        );
    }

    #[test]
    fn globals_set_on_the_coroutine_thread_are_visible_from_the_main_state() {
        // `register` writes via `lsetglobal` on `self.thread`, while
        // `enumerate_globals` reads `self.lua`. Lua 5.0 shares one
        // globals table between a thread and its parent; if that ever
        // changed, /v1/script/globals would silently go blind.
        let vm: Lua5032Vm<()> = Lua5032Vm::create(Rc::new(RefCell::new(())));
        vm.register("host_fn", Some(noop_cfunction));

        assert_eq!(
            lookup(&vm.enumerate_globals(), "host_fn"),
            Some(LuaValue::Other("function")),
        );
    }

    extern "C" fn noop_cfunction(_state: *mut lua_State) -> i32 {
        0
    }
}
