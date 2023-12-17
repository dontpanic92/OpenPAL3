use lua50_32_sys::lua_State;

use crate::scripting::lua50_32::Lua5032Vm;

use super::asset_loader::AssetLoader;

pub struct SWD5Context {
    test: i32,
}

impl SWD5Context {
    pub fn isfon(&self, f: f64) -> i32 {
        println!("in context: {} {}", f, self.test);
        0
    }
}

macro_rules! def_func {
    ($vm: ident, $fn_name: ident $(, $param_names: ident : $param_types: ident)* -> $ret_type: ident) => {
        paste::paste! {
            extern "C" fn $fn_name(state: *mut lua_State) -> i32 {
                unsafe {
                    let v = lua50_32_sys::lua_touserdata(state, lua50_32_sys::LUA_GLOBALSINDEX - 1);
                    let context = &*(v as *const SWD5Context);
                    $(let $param_names = lua50_32_sys::[<lua_to $param_types>](state, 1);lua50_32_sys::lua_remove(state, 1);)*

                    let ret = context.$fn_name($($param_names),*);
                    lua50_32_sys::[<lua_push $ret_type>](state, ret.into());
                }

                1
            }

            $vm.register(stringify!($fn_name), Some($fn_name));
        }
    };
}

pub fn create_lua_vm(asset_loader: &AssetLoader) -> anyhow::Result<Lua5032Vm<SWD5Context>> {
    let context = SWD5Context { test: 999 };
    let script = asset_loader.load_main_script()?;
    let vm = Lua5032Vm::new(script, "initiatelua", context)?;

    def_func!(vm, isfon, f: number -> number);
    Ok(vm)
}

/*extern "C" fn isfon(state: *mut lua_State) -> i32 {
    unsafe {
        let v = lua50_32_sys::lua_touserdata(state, lua50_32_sys::LUA_GLOBALSINDEX - 1);
        let context = &*(v as *const SWD5Context);
        let s = lua50_32_sys::lua_tonumber(state, 1);
        println!("isfon: {} v {:?}", s, v);

        let ret = context.isfon(s);

        lua50_32_sys::lua_pushboolean(state, ret as i32);
    }

    1
}
*/
