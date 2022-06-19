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
    'UUID': ('uuid::Uuid', 'uuid::Uuid'),
    'void': ('()', '()'),
}


class RustGen:

    def __init__(self, unit: CrossComIdl, module_name: str):
        self.unit = unit
        self.symbols = RustGen.__collect_symbols(unit)
        self.module_name = module_name

    def gen(self) -> str:

        w = Writer()

        for i in self.unit.items:
            if isinstance(i, Class):
                w.w(self.__gen_class(i))
            else:
                self.__gen_interface(i, w)

        return w.get_value()

    def __gen_method_raw_param_list(self, method: Method):
        w = Writer()

        for p in method.params:
            w.ln(f'{p.name}: {self.__map_type_to_raw(p.ty, p.attrs)}, ')

        return w.get_value()

    def __gen_method_raw_signature(self, method: Method, w: Writer):
        w.ln(
            f'unsafe extern "system" fn (this: *const *const std::os::raw::c_void, {self.__gen_method_raw_param_list(method)}) -> {self.__map_type_to_raw(method.ret_ty)}')

    def __gen_method_signature2(self, method: Method) -> str:
        w = Writer()
        w.w(f'fn {method.name} (&self, ')

        for p in method.params:
            w.ln(f'{p.name}: {self.__map_type(p.ty, False)}, ')

        w.ln(f') -> {self.__map_type(method.ret_ty, False)}')

        return w.get_value()

    def __gen_trait_use(self) -> str:
        w = Writer()
        w.ln(f'use crosscom::ComInterface;')
        for item in self.unit.items:
            if isinstance(item, Interface) and not item.codegen_ignore():
                w.ln(f'use {self.module_name}::{item.name}Impl;')
        return w.get_value()

    def __gen_klass_base_field(self, klass: Class) -> str:
        w = Writer()
        for b in klass.bases:
            w.ln(f'{b}: {self.module_name}::{b},')
        return w.get_value()

    def __gen_raw_method_impl(self, klass: Class, method: Method) -> str:
        w = Writer()

        w.ln(f"""
unsafe extern "system" fn {method.name} (this: *const *const std::os::raw::c_void, {self.__gen_method_raw_param_list(method)}) -> {self.__map_type_to_raw(method.ret_ty)} {{
    let object = crosscom::get_object::<{klass.name}Ccw>(this);
    (*object).inner.{ method.name }({','.join([p.name for p in method.params])})
}}
""")

        return w.get_value()

    def __gen_raw_method_impl_for_class(self, klass: Class) -> str:
        w = Writer()

        for m in klass.methods:
            w.ln(self.__gen_raw_method_impl(klass, m))

        visited = set()
        ancestors = [b for b in klass.bases]
        while len(ancestors) > 0:
            a = ancestors.pop(0)
            if a in visited:
                continue

            visited.add(a)

            interface = self.unit.find(a)
            if interface is None:
                raise f'Cannot find base type: {a}'

            if isinstance(interface, Class):
                raise f'Class type cannot be used as base: {a}'

            if interface.codegen_ignore():
                continue

            if interface.bases is not None:
                ancestors.extend(interface.bases)

            for m in interface.methods:
                w.ln(self.__gen_raw_method_impl(klass, m))

        return w.get_value()
    def __collect_all_methods2(self, iname: str) -> list[Method]:
        interface = self.unit.find(iname)
        if interface is None:
            raise f'Cannot find base type: {iname}'

        if isinstance(interface, Class):
            raise f'Class type cannot be used as base: {iname}'

        methods = []
        if interface.bases is not None:
            if len(interface.bases) == 1:
                methods = self.__collect_all_methods2(interface.bases[0])
            elif len(interface.bases) > 1:
                raise f'Cannot have more than 1 parent for interface: {interface.name}'

        methods.extend(interface.methods)

        return methods
    
    def __collect_inherit_chain(self, iname: str) -> list[Method]:
        interface = self.unit.find(iname)
        if interface is None:
            raise f'Cannot find base type: {iname}'

        if isinstance(interface, Class):
            raise f'Class type cannot be used as base: {iname}'

        ifaces = []
        if interface.bases is not None:
            if len(interface.bases) == 1:
                ifaces = self.__collect_inherit_chain(interface.bases[0])
            elif len(interface.bases) > 1:
                raise f'Cannot have more than 1 parent for interface: {interface.name}'

        ifaces.append(interface)

        return ifaces

    def __gen_interface_vtbl_methods(self, iname: str) -> str:
        w = Writer()

        methods = self.__collect_all_methods2(iname)
        for m in methods:
            w.ln(m.name + ',')

        return w.get_value()

    def __gen_base_struct(self, klass: Class) -> str:
        w = Writer()
        for b in klass.bases:
            w.ln(f"""
{b}: {self.module_name}::{b} {{
    vtable: &GLOBAL_{b}VirtualTable_CCW_FOR_{klass.name}.vtable
        as *const {self.module_name}::{b}VirtualTable,
}},""")

        return w.get_value()

    def __gen_class_ccw_vtbl(self, klass: Class) -> str:
        w = Writer()
        offset = 1
        for b in klass.bases:
            offset -= 1
            w.ln(f"""
#[allow(non_upper_case_globals)]
pub const GLOBAL_{b}VirtualTable_CCW_FOR_{ klass.name }: {self.module_name}::{b}VirtualTableCcw 
    = {self.module_name}::{b}VirtualTableCcw {{
    offset: {offset},
    vtable: {self.module_name}::{b}VirtualTable {{
        {self.__gen_interface_vtbl_methods(b)}
    }},
}};

""")
        return w.get_value()

    def __gen_query_interface_branches(self, klass: Class) -> str:
        w = Writer()

        visited = set()
        offset = -1
        for i in klass.bases:
            offset += 1
            ifaces = self.__collect_inherit_chain(i)
            for interface in ifaces:
                if interface.name in visited:
                    continue

                visited.add(interface.name)
                mod = 'crosscom' if interface.name == 'IUnknown' else self.module_name
                w.ln(f"""
&{mod}::{interface.name}::INTERFACE_ID => {{
    *retval = (object as *const *const std::os::raw::c_void).offset({offset});
    add_ref(object as *const *const std::os::raw::c_void);
    crosscom::ResultCode::Ok as i32
}}
""")

        return w.get_value()

    def __gen_class(self, klass: Class) -> str:
        w = Writer()
        w.ln(f"""
// Class {klass.name}

#[allow(unused)]
macro_rules! ComObject_{klass.name} {{
    ($impl_type: ty) => {{

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod {klass.name}_crosscom_impl {{
    {self.__gen_trait_use()}

    #[repr(C)]
    pub struct { klass.name }Ccw {{
        {self.__gen_klass_base_field(klass)}
        ref_count: std::sync::atomic::AtomicU32,
        inner: $impl_type,
    }}

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {{
        let object = crosscom::get_object::<{klass.name}Ccw>(this);
        match guid.as_bytes() {{
            {self.__gen_query_interface_branches(klass)}
            _ => crosscom::ResultCode::ENoInterface as i32,
        }}
    }}

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {{
        let object = crosscom::get_object::<{klass.name}Ccw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }}

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {{
        let object = crosscom::get_object::<{klass.name}Ccw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {{
            Box::from_raw(object as *mut {klass.name}Ccw);
        }}

        (previous - 1) as std::os::raw::c_long
    }}


    {self.__gen_raw_method_impl_for_class(klass)}


    {self.__gen_class_ccw_vtbl(klass)}

    impl crosscom::ComObject for $impl_type {{
        type CcwType = {klass.name}Ccw;

        fn create_ccw(self) -> Self::CcwType {{
            Self::CcwType {{
                {self.__gen_base_struct(klass)}
                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }}
        }}
    }}
}}
    }}
}}
""")

        return w.get_value()
    
    def __gen_interface_method_safe_wrapper(self, i: Interface) -> str:
        w = Writer()

        for method in self.__collect_all_methods2(i.name):
            if method.name != 'query_interface':
                w.ln(f"""
pub {self.__gen_method_signature2(method)} {{
    unsafe {{
        let this = self as *const {i.name} as *const *const std::os::raw::c_void;
        ((*self.vtable).{method.name})(this, {','.join([p.name for p in method.params])}).into()
    }}
}}
""")
            else:
                w.ln(f"""
pub fn query_interface<T: crosscom::ComInterface>(&self) -> Option<crosscom::ComRc<T>> {{
    let this = self as *const {i.name} as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe {{ ((*self.vtable).query_interface)(this, guid, &mut raw) }};
    if ret_val != 0 {{
        None
    }} else {{
        Some(unsafe {{ crosscom::ComRc::<T>::from_raw_pointer(raw) }})
    }}
}}
""")

        return w.get_value()


    def __gen_interface(self, i: Interface, w: Writer):
        if i.codegen_ignore():
            return

        w.ln(f'// Interface {i.name}')

        # Virtual Table
        w.ln(f"""
#[repr(C)]
#[allow(non_snake_case)]
pub struct { i.name }VirtualTable {{
""")

        for method in self.__collect_all_methods2(i.name):
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
    {self.__gen_interface_method_safe_wrapper(i)}
}}
""")

        # Trait

        w.ln(f'pub trait {i.name}Impl {{')

        for method in i.methods:
            w.ln(f'{self.__gen_method_signature2(method)};')

        w.ln('}')
        w.ln(f"""
impl crosscom::ComInterface for {i.name} {{
            
    // {i.attrs["uuid"]}
    const INTERFACE_ID: [u8; 16] = {RustGen.__uuid_to_hex_array(i.attrs["uuid"])};
}}""")
        w.ln()

    def __map_type_to_raw(self, idl_ty: str, attrs: list[str] = None) -> str:
        is_out = attrs is not None and 'out' in attrs

        if idl_ty in type_map:
            return type_map[idl_ty][0]
        else:
            ty = self.symbols.get(idl_ty)
            if ty != None:
                if isinstance(ty, Class):
                    raise f'Cannot use class type here: {ty}'
                else:
                    if is_out:
                        return '&mut *const *const std::os::raw::c_void'
                    else:
                        return '*const *const std::os::raw::c_void'

    def __map_type(self, idl_ty: str, mod_prefix = True) -> str:
        if idl_ty in type_map:
            return type_map[idl_ty][1]
        else:
            ty = self.symbols.get(idl_ty)
            if ty != None:
                if isinstance(ty, Class):
                    raise f'Cannot use class type here: {ty}'
                else:
                    if mod_prefix:
                        mod = self.module_name if ty.name != 'IUnknown' else 'crosscom'
                        return f'crosscom::ComRc<{mod}::{ty.name}>'
                    else:
                        return f'crosscom::ComRc<{ty.name}>'

    def __collect_symbols(unit: CrossComIdl):
        symbols = {}
        for i in unit.items:
            symbols[i.name] = i

        return symbols

    def __uuid_to_hex_array(id: str) -> str:
        guid = uuid.UUID(id)
        return '[' + ','.join([str(b) + 'u8' for b in guid.bytes]) + ']'
