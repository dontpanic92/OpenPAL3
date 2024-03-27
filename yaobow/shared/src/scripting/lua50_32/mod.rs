use std::{cell::RefCell, rc::Rc};

use anyhow::bail;
use encoding::{DecoderTrap, Encoding};
use lua50_32_sys::lua_State;

pub struct Lua5032Vm<TContext> {
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
                lib.as_ptr() as *const i8,
                lib.len(),
                b"main\0".as_ptr() as *const i8,
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
