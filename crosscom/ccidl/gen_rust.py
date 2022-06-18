import io
import uuid
from parser import Interface, Class, CrossComIdl, Method, MethodParameter


class Writer:
    def __init__(self):
        self.writer = io.StringIO()

    def ln(self, text="", ident=0):
        prefix = "    " * ident
        return self.writer.write(prefix + text + '\n')

    def w(self, text):
        return self.writer.write(text)

    def get_value(self):
        return self.writer.getvalue()


type_map = {
    'long': ('std::os::raw::c_long', 'i64'),
    'int': ('std::os::raw::c_int', 'i32'),
    'float': ('std::os::raw::c_float', 'f32'),
    'void': ('()', '()'),
}


class RustGen:

    def __init__(self, unit: CrossComIdl):
        self.unit = unit
        self.symbols = RustGen.__collect_symbols(unit)

    def gen(self) -> str:

        w = Writer()

        for i in self.unit.items:
            if isinstance(i, Class):
                self.__gen_class(i, w)
            else:
                self.__gen_interface(i, w)

        return w.get_value()

    def __gen_method_raw_signature(self, method: Method, w: Writer):
        w.ln(r'unsafe extern "system" fn (this: *const std::os::raw::c_void,')

        for p in method.params:
            w.ln(f'{p.name}: {self.__map_type_to_raw(p.ty)}, ')

        w.ln(f') -> {self.__map_type_to_raw(method.ret_ty)}')

    def __gen_method_signature(self, method: Method, w: Writer):
        w.w(f'fn {method.name} (&self, ')

        for p in method.params:
            w.ln(f'{p.name}: {self.__map_type(p.ty)}, ')

        w.ln(f') -> {self.__map_type(method.ret_ty)}')

    def __gen_trait_use(self) -> str:
        w = Writer()
        for item in self.unit:
            if isinstance(item, Interface):
                w.ln(f'use super::{item.name}Impl;')
        return w.get_value()

    def __gen_klass_base_field(self, klass: Class) -> str:
        w = Writer()
        for b in klass.bases:
            w.ln(f'{b.name}: super: : {{b }}')
        return w.get_value()
    
    def __gen_raw_method_impl(self, klass: Class) -> str:
        w = Writer()
        for b in klass.bases:
            w.ln(f'{b.name}: super: : {{b }}')
        return w.get_value()

    def __gen_class(self, klass: Class) -> str:
        w = Writer()
        w.ln(f"""
// Class {klass.name}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod {klass.name}_impl {{
    {self.__gen_trait_use()}
}}

#[repr(C)]
pub struct { klass.name }Ccw<T> {{
    {self.__gen_klass_base_field(klass)}
    ref_count: std::sync::atomic::AtomicU32,
    inner: <T>,
}}

unsafe extern "system" fn query_interface(
    this: *const std::os::raw::c_void,
    guid: uuid::Uuid,
    retval: *mut std::os::raw::c_void,
) -> std::os::raw::c_long {{
    let object = crosscom::get_object::<{klass.name}Ccw>(this);

    0
}}

unsafe extern "system" fn add_ref(this: *const std::os::raw::c_void) -> std::os::raw::c_long {{
    let object = crosscom::get_object::<{klass.name}Ccw>(this);
    let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    (previous + 1) as std::os::raw::c_long
}}

unsafe extern "system" fn release(this: *const std::os::raw::c_void) -> std::os::raw::c_long {{
    let object = crosscom::get_object::<{klass.name}Ccw>(this);

    let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    if previous - 1 == 0 {{
        Box::from_raw(object as *mut {klass.name}Ccw);
    }}

    (previous - 1) as std::os::raw::c_long
}}

{self.__gen_raw_method_impl(klass)}
""")

        return w.get_value()

    def __gen_interface(self, i: Interface, w: Writer):
        w.ln(f'// Interface {i.name}')

        # Virtual Table
        w.ln(f"""
#[repr(C)]
#[allow(non_snake_case)]
pub struct { i.name }VirtualTable {{
""")

        for method in i.methods:
            w.w(f'    pub { method.name }: ')
            self.__gen_method_raw_signature(method, w)
            w.w(',')

        w.ln('}')
        w.ln()

        # Virtual table Ccw
        w.ln(f"""
#[repr(C)]
#[allow(dead_code)]
pub struct { i.name }VirtualTableCcw {{
    pub offset: isize,
    pub vtable: { i.name }VirtualTable,
}}

""")
        # Interface implementation
        w.ln(f"""
#[repr(C)]
#[allow(dead_code)]
pub struct { i.name } {{
    pub vtable: *const { i.name }VirtualTable,
}}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl { i.name } {{
    // {i.attrs["uuid"]}
    pub const INTERFACE_ID: [u8; 16] = [
        {RustGen.__uuid_to_hex_array(i.attrs["uuid"])}
    ];
""")
        for method in i.methods:
            w.w(f'pub ')
            self.__gen_method_signature(method, w)
            w.ln('{')
            w.ln('unsafe {', 1)
            w.ln(f'let this = self as *const {i.name} as *const c_void;', 2)
            w.ln(f'((*self.vtable).{method.name})(this, ', 2)
            for p in method.params:
                w.w(p.name + ', ')
            w.ln()
            w.ln(')', 2)
            w.ln('}', 1)
            w.ln('}')

        w.ln('}')

        # Trait

        w.ln(f'pub trait {i.name}Impl {{')

        for method in i.methods:
            self.__gen_method_signature(method, w)

        w.ln('}')
        w.ln(f'impl ComInterface for {i.name} {{}}')
        w.ln()

    def __map_type_to_raw(self, idl_ty: str) -> str:
        if idl_ty in type_map:
            return type_map[idl_ty][0]
        else:
            ty = self.symbols.get(idl_ty)
            if ty != None:
                if isinstance(ty, Class):
                    raise f'Cannot use class type here: {ty}'
                else:
                    return '*const std::os::raw::c_void'

    def __map_type(self, idl_ty: str) -> str:
        if idl_ty in type_map:
            return type_map[idl_ty][1]
        else:
            ty = self.symbols.get(idl_ty)
            if ty != None:
                if isinstance(ty, Class):
                    raise f'Cannot use class type here: {ty}'
                else:
                    return f'ComRc<{ty.name}>'

    def __collect_symbols(unit: CrossComIdl):
        symbols = {}
        for i in unit.items:
            symbols[i.name] = i

        return symbols

    def __uuid_to_hex_array(id: str):
        guid = uuid.UUID(id)
        return [b for b in guid.bytes]
