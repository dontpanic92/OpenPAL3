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
