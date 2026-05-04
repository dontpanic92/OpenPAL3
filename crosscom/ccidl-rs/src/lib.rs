use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

pub struct GeneratedUnit {
    pub source: String,
    pub dependencies: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    Generate(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => {
                write!(f, "failed to read {}: {}", path.display(), source)
            }
            Error::Parse { path, message } => {
                write!(f, "failed to parse {}: {}", path.display(), message)
            }
            Error::Generate(message) => write!(f, "failed to generate Rust source: {message}"),
        }
    }
}

impl std::error::Error for Error {}

pub fn generate(idl_path: impl AsRef<Path>) -> Result<GeneratedUnit, Error> {
    let idl_path = idl_path.as_ref();
    let mut dependencies = Vec::new();
    let mut visited = HashSet::new();
    collect_dependencies(idl_path, &mut visited, &mut dependencies)?;

    let mut unit = parse_file(idl_path)?;
    process_imports(idl_path, &mut unit)?;
    let source = RustGen::new(unit)?.gen()?;

    Ok(GeneratedUnit {
        source,
        dependencies,
    })
}

pub fn generate_to_file(
    idl_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, Error> {
    let generated = generate(idl_path)?;
    std::fs::write(output_path.as_ref(), generated.source).map_err(|source| Error::Io {
        path: output_path.as_ref().to_path_buf(),
        source,
    })?;
    Ok(generated.dependencies)
}

fn parse_file(path: &Path) -> Result<CrossComIdl, Error> {
    let content = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Parser::new(&content)
        .parse()
        .map_err(|message| Error::Parse {
            path: path.to_path_buf(),
            message,
        })
}

fn collect_dependencies(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    dependencies: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let path = path.canonicalize().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !visited.insert(path.clone()) {
        return Ok(());
    }

    dependencies.push(path.clone());
    let unit = parse_file(&path)?;
    let source_dir = path.parent().unwrap_or_else(|| Path::new(""));
    for import in unit.imports {
        collect_dependencies(&source_dir.join(import.file_name), visited, dependencies)?;
    }

    Ok(())
}

fn process_imports(idl_path: &Path, unit: &mut CrossComIdl) -> Result<(), Error> {
    let source_dir = idl_path.parent().unwrap_or_else(|| Path::new(""));
    let imports = unit.imports.clone();
    for import in imports {
        let import_path = source_dir.join(&import.file_name);
        let import_unit = parse_file(&import_path)?;
        let import_module = rust_module(&import_unit)?.clone();

        for item in import_unit.items {
            if let Item::Interface(mut interface) = item {
                interface
                    .attrs
                    .insert("codegen".to_string(), "ignore".to_string());
                interface.module = Some(import_module.clone());
                unit.items.push(Item::Interface(interface));
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct CrossComIdl {
    items: Vec<Item>,
    imports: Vec<Import>,
    modules: Vec<Module>,
}

#[derive(Clone, Debug)]
struct Import {
    file_name: String,
}

#[derive(Clone, Debug)]
struct Module {
    module_lang: String,
    module_name: String,
}

#[derive(Clone, Debug)]
enum Item {
    Interface(Interface),
    Class(Class),
}

#[derive(Clone, Debug)]
struct Interface {
    name: String,
    bases: Vec<String>,
    methods: Vec<Method>,
    attrs: Attrs,
    module: Option<Module>,
}

impl Interface {
    fn codegen_ignore(&self) -> bool {
        self.attrs.get("codegen").map(String::as_str) == Some("ignore")
    }

    fn public_methods(&self) -> Vec<Method> {
        self.methods
            .iter()
            .filter(|method| !method.attrs.contains_key("internal"))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug)]
struct Class {
    name: String,
    bases: Vec<String>,
    methods: Vec<Method>,
    attrs: Attrs,
    module: Option<Module>,
}

#[derive(Clone, Debug)]
struct Method {
    name: String,
    ret_ty: String,
    params: Vec<MethodParameter>,
    attrs: Attrs,
    interface_module: Option<Module>,
}

#[derive(Clone, Debug)]
struct MethodParameter {
    attrs: Vec<String>,
    name: String,
    ty: String,
}

type Attrs = HashMap<String, String>;

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse(mut self) -> Result<CrossComIdl, String> {
        let mut items = Vec::new();
        let mut imports = Vec::new();
        let mut modules = Vec::new();

        while {
            self.skip_ws();
            !self.eof()
        } {
            let attrs = if self.peek_char() == Some('[') {
                self.parse_attrs()?
            } else {
                Attrs::new()
            };

            if self.consume("module") {
                self.skip_ws();
                self.expect_char('(')?;
                let module_lang = self.read_until(')')?.trim().to_string();
                self.expect_char(')')?;
                self.skip_ws();
                let module_name = self.read_until(';')?.trim().to_string();
                self.expect_char(';')?;
                modules.push(Module {
                    module_lang,
                    module_name,
                });
            } else if self.consume("import") {
                self.skip_ws();
                let file_name = self.read_until(';')?.trim().to_string();
                self.expect_char(';')?;
                imports.push(Import { file_name });
            } else if self.consume("interface") {
                items.push(Item::Interface(self.parse_interface(attrs)?));
            } else if self.consume("class") {
                items.push(Item::Class(self.parse_class(attrs)?));
            } else {
                return Err(format!("unexpected token near byte {}", self.pos));
            }
        }

        Ok(CrossComIdl {
            items,
            imports,
            modules,
        })
    }

    fn parse_interface(&mut self, attrs: Attrs) -> Result<Interface, String> {
        let (name, bases, methods) = self.parse_decl_body(true)?;
        Ok(Interface {
            name,
            bases,
            methods,
            attrs,
            module: None,
        })
    }

    fn parse_class(&mut self, attrs: Attrs) -> Result<Class, String> {
        let (name, bases, methods) = self.parse_decl_body(false)?;
        Ok(Class {
            name,
            bases,
            methods,
            attrs,
            module: None,
        })
    }

    fn parse_decl_body(
        &mut self,
        with_methods: bool,
    ) -> Result<(String, Vec<String>, Vec<Method>), String> {
        self.skip_ws();
        let name = self.read_identifier()?;
        self.skip_ws();

        let mut bases = Vec::new();
        if self.peek_char() == Some(':') {
            self.pos += 1;
            let raw_bases = self.read_until('{')?;
            bases = raw_bases
                .split(',')
                .map(str::trim)
                .filter(|base| !base.is_empty())
                .map(ToOwned::to_owned)
                .collect();
        }

        self.expect_char('{')?;
        let mut methods = Vec::new();
        loop {
            self.skip_ws();
            if self.peek_char() == Some('}') {
                self.pos += 1;
                break;
            }
            if self.eof() {
                return Err("unexpected end of file inside declaration".to_string());
            }

            let attrs = if self.peek_char() == Some('[') {
                self.parse_attrs()?
            } else {
                Attrs::new()
            };
            let decl = self.read_until(';')?;
            self.expect_char(';')?;
            if with_methods {
                methods.push(parse_method_decl(decl.trim(), attrs)?);
            }
        }

        Ok((name, bases, methods))
    }

    fn parse_attrs(&mut self) -> Result<Attrs, String> {
        self.expect_char('[')?;
        let mut attrs = Attrs::new();
        loop {
            self.skip_ws();
            if self.peek_char() == Some(']') {
                self.pos += 1;
                break;
            }

            let name = self.read_attr_name()?;
            self.skip_ws();
            let value = if self.peek_char() == Some('(') {
                self.pos += 1;
                let value = self.read_until(')')?.trim().to_string();
                self.expect_char(')')?;
                value
            } else {
                String::new()
            };
            attrs.insert(name, value);

            self.skip_ws();
            match self.peek_char() {
                Some(',') => self.pos += 1,
                Some(']') => {}
                other => {
                    return Err(format!(
                        "expected ',' or ']' in attributes, found {other:?}"
                    ))
                }
            }
        }
        self.skip_ws();
        Ok(attrs)
    }

    fn read_attr_name(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == '(' || ch == ',' || ch == ']' || ch.is_whitespace() {
                break;
            }
            self.pos += ch.len_utf8();
        }
        if start == self.pos {
            Err(format!("expected attribute name near byte {}", self.pos))
        } else {
            Ok(self.input[start..self.pos].trim().to_string())
        }
    }

    fn read_identifier(&mut self) -> Result<String, String> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() || matches!(ch, ':' | '{' | '}' | '(' | ')' | ';' | ',') {
                break;
            }
            self.pos += ch.len_utf8();
        }
        if start == self.pos {
            Err(format!("expected identifier near byte {}", self.pos))
        } else {
            Ok(self.input[start..self.pos].trim().to_string())
        }
    }

    fn read_until(&mut self, target: char) -> Result<&'a str, String> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == target {
                return Ok(&self.input[start..self.pos]);
            }
            self.pos += ch.len_utf8();
        }
        Err(format!("expected '{target}' near byte {}", self.pos))
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        self.skip_ws();
        match self.peek_char() {
            Some(ch) if ch == expected => {
                self.pos += ch.len_utf8();
                Ok(())
            }
            other => Err(format!(
                "expected '{expected}', found {other:?} near byte {}",
                self.pos
            )),
        }
    }

    fn consume(&mut self, expected: &str) -> bool {
        if self.input[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn parse_method_decl(decl: &str, attrs: Attrs) -> Result<Method, String> {
    let open = decl
        .find('(')
        .ok_or_else(|| format!("method missing '(' in {decl:?}"))?;
    let close = decl
        .rfind(')')
        .ok_or_else(|| format!("method missing ')' in {decl:?}"))?;
    let prefix = decl[..open].trim();
    let (ret_ty, name) = split_type_and_name(prefix)?;

    let params_src = decl[open + 1..close].trim();
    let mut params = Vec::new();
    if !params_src.is_empty() {
        for param in split_params(params_src) {
            params.push(parse_param(param.trim())?);
        }
    }

    Ok(Method {
        name,
        ret_ty,
        params,
        attrs,
        interface_module: None,
    })
}

fn parse_param(param: &str) -> Result<MethodParameter, String> {
    let mut attrs = Vec::new();
    let mut rest = param.trim();
    if rest.starts_with('[') {
        let end = rest
            .find(']')
            .ok_or_else(|| format!("parameter attribute missing ']' in {param:?}"))?;
        attrs = rest[1..end]
            .split(',')
            .map(str::trim)
            .filter(|attr| !attr.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        rest = rest[end + 1..].trim();
    }

    let (ty, name) = split_type_and_name(rest)?;
    Ok(MethodParameter { attrs, name, ty })
}

fn split_type_and_name(input: &str) -> Result<(String, String), String> {
    let idx = input
        .rfind(char::is_whitespace)
        .ok_or_else(|| format!("expected '<type> <name>' in {input:?}"))?;
    let ty = input[..idx].trim();
    let name = input[idx..].trim();
    if ty.is_empty() || name.is_empty() {
        Err(format!("expected '<type> <name>' in {input:?}"))
    } else {
        Ok((ty.to_string(), name.to_string()))
    }
}

fn split_params(input: &str) -> Vec<&str> {
    let mut params = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in input.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                params.push(&input[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    params.push(&input[start..]);
    params
}

fn rust_module(unit: &CrossComIdl) -> Result<&Module, Error> {
    unit.modules
        .iter()
        .find(|module| module.module_lang == "rust")
        .ok_or_else(|| Error::Generate("IDL file does not declare a rust module".to_string()))
}

#[derive(Clone)]
enum Symbol {
    Interface(Interface),
    Class,
}

struct RustGen {
    unit: CrossComIdl,
    symbols: HashMap<String, Symbol>,
    crosscom_module_name: String,
}

impl RustGen {
    fn new(mut unit: CrossComIdl) -> Result<Self, Error> {
        let current_module = rust_module(&unit)?.clone();
        let mut symbols = HashMap::new();

        for item in &mut unit.items {
            match item {
                Item::Interface(interface) => {
                    if interface.module.is_none() {
                        interface.module = Some(current_module.clone());
                    }
                    symbols.insert(interface.name.clone(), Symbol::Interface(interface.clone()));
                }
                Item::Class(class) => {
                    if class.module.is_none() {
                        class.module = Some(current_module.clone());
                    }
                    symbols.insert(class.name.clone(), Symbol::Class);
                }
            }
        }

        Ok(Self {
            unit,
            symbols,
            crosscom_module_name: "crosscom".to_string(),
        })
    }

    fn gen(&self) -> Result<String, Error> {
        let mut out = String::new();
        out.push_str(&format!(
            "#[allow(unused_imports)]\nuse crate as {};\n",
            self.rust_crate()?
        ));

        for item in &self.unit.items {
            match item {
                Item::Interface(interface) => self.gen_interface(interface, &mut out)?,
                Item::Class(class) => out.push_str(&self.gen_class(class)?),
            }
        }

        Ok(out)
    }

    fn rust_crate(&self) -> Result<String, Error> {
        Ok(rust_module(&self.unit)?
            .module_name
            .split("::")
            .next()
            .unwrap()
            .to_string())
    }

    fn find_interface(&self, name: &str) -> Result<&Interface, Error> {
        match self.symbols.get(name) {
            Some(Symbol::Interface(interface)) => Ok(interface),
            Some(Symbol::Class) => Err(Error::Generate(format!(
                "class type cannot be used as interface: {name}"
            ))),
            None => Err(Error::Generate(format!("cannot find base type: {name}"))),
        }
    }

    fn interface_symbol(&self, idl_ty: &str) -> Result<Option<&Interface>, Error> {
        match self.symbols.get(idl_ty) {
            Some(Symbol::Interface(interface)) => Ok(Some(interface)),
            Some(Symbol::Class) => Err(Error::Generate(format!(
                "cannot use class type here: {idl_ty}"
            ))),
            None => Ok(None),
        }
    }

    fn collect_all_methods(&self, iname: &str, only_public: bool) -> Result<Vec<Method>, Error> {
        let interface = self.find_interface(iname)?;
        let mut methods = Vec::new();
        match interface.bases.len() {
            0 => {}
            1 => methods.extend(self.collect_all_methods(&interface.bases[0], only_public)?),
            _ => {
                return Err(Error::Generate(format!(
                    "cannot have more than one parent for interface: {}",
                    interface.name
                )))
            }
        }

        let interface_methods = if only_public {
            interface.public_methods()
        } else {
            interface.methods.clone()
        };
        let module = interface.module.clone();
        methods.extend(interface_methods.into_iter().map(|mut method| {
            method.interface_module = module.clone();
            method
        }));
        Ok(methods)
    }

    fn collect_inherit_chain(&self, iname: &str) -> Result<Vec<Interface>, Error> {
        let interface = self.find_interface(iname)?;
        let mut ifaces = Vec::new();
        match interface.bases.len() {
            0 => {}
            1 => ifaces.extend(self.collect_inherit_chain(&interface.bases[0])?),
            _ => {
                return Err(Error::Generate(format!(
                    "cannot have more than one parent for interface: {}",
                    interface.name
                )))
            }
        }
        ifaces.push(interface.clone());
        Ok(ifaces)
    }

    fn gen_interface_vtbl_methods(&self, iname: &str) -> Result<String, Error> {
        let mut out = String::new();
        for method in self.collect_all_methods(iname, false)? {
            out.push_str(&format!("            {},\n", method.name));
        }
        Ok(out)
    }

    fn gen_class(&self, class: &Class) -> Result<String, Error> {
        let rust_crate = self.rust_crate()?;
        let trait_use = self.gen_trait_use()?;
        let klass_base_field = self.gen_klass_base_field(class)?;
        let query_interface_branches = self.gen_query_interface_branches(class)?;
        let raw_methods = self.gen_raw_method_impl_for_class(class)?;
        let class_ccw_vtbl = self.gen_class_ccw_vtbl(class)?;
        let base_struct = self.gen_base_struct(class)?;
        let name = &class.name;
        Ok(format!(
            r#"
// Class {name}

#[allow(unused)]
#[macro_export]
macro_rules! ComObject_{name} {{
    ($impl_type: ty) => {{

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
mod {name}_crosscom_impl {{
    use crate as {rust_crate};
    {trait_use}

    #[repr(C)]
    pub struct {name}Ccw {{
        {klass_base_field}
        ref_count: std::sync::atomic::AtomicU32,
        pub inner: $impl_type,
    }}

    unsafe extern "system" fn query_interface(
        this: *const *const std::os::raw::c_void,
        guid: uuid::Uuid,
        retval: &mut *const *const std::os::raw::c_void,
    ) -> std::os::raw::c_long {{
        let object = {crosscom}::get_object::<{name}Ccw>(this);
        match guid.as_bytes() {{
            {query_interface_branches}
            _ => {crosscom}::ResultCode::ENoInterface as std::os::raw::c_long,
        }}
    }}

    unsafe extern "system" fn add_ref(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {{
        let object = {crosscom}::get_object::<{name}Ccw>(this);
        let previous = (*object).ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (previous + 1) as std::os::raw::c_long
    }}

    unsafe extern "system" fn release(this: *const *const std::os::raw::c_void) -> std::os::raw::c_long {{
        let object = {crosscom}::get_object::<{name}Ccw>(this);

        let previous = (*object).ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if previous - 1 == 0 {{
            Box::from_raw(object as *mut {name}Ccw);
        }}

        (previous - 1) as std::os::raw::c_long
    }}

    {raw_methods}

    {class_ccw_vtbl}
    impl {crosscom}::ComObject for $impl_type {{
        type CcwType = {name}Ccw;

        fn create_ccw(self) -> Self::CcwType {{
            Self::CcwType {{
                {base_struct}
                ref_count: std::sync::atomic::AtomicU32::new(0),
                inner: self,
            }}
        }}

        fn get_ccw(&self) -> &Self::CcwType {{
            unsafe {{
                let this = self as *const _ as *const u8;
                let this = this.offset(-({crosscom}::offset_of!({name}Ccw, inner) as isize));
                &*(this as *const Self::CcwType)
            }}
        }}
    }}
}}
    }}
}}

// pub use ComObject_{name};
"#,
            crosscom = self.crosscom_module_name
        ))
    }

    fn gen_trait_use(&self) -> Result<String, Error> {
        let mut out = format!("use {}::ComInterface;\n", self.crosscom_module_name);
        for item in &self.unit.items {
            if let Item::Interface(interface) = item {
                let module = interface.module.as_ref().ok_or_else(|| {
                    Error::Generate(format!(
                        "interface {} does not have a module",
                        interface.name
                    ))
                })?;
                out.push_str(&format!(
                    "    use {}::{}Impl;\n",
                    module.module_name, interface.name
                ));
            }
        }
        Ok(out)
    }

    fn gen_klass_base_field(&self, class: &Class) -> Result<String, Error> {
        let mut out = String::new();
        for base in &class.bases {
            let symbol = self.find_interface(base)?;
            let module = symbol.module.as_ref().unwrap();
            out.push_str(&format!("{}: {}::{},\n", base, module.module_name, base));
        }
        Ok(out)
    }

    fn gen_base_struct(&self, class: &Class) -> Result<String, Error> {
        let mut out = String::new();
        for base in &class.bases {
            let symbol = self.find_interface(base)?;
            let module = &symbol.module.as_ref().unwrap().module_name;
            out.push_str(&format!(
                r#"
{base}: {module}::{base} {{
    vtable: &GLOBAL_{base}VirtualTable_CCW_FOR_{class_name}.vtable
        as *const {module}::{base}VirtualTable,
}},
"#,
                class_name = class.name
            ));
        }
        Ok(out)
    }

    fn gen_class_ccw_vtbl(&self, class: &Class) -> Result<String, Error> {
        let mut out = String::new();
        let mut offset = 1isize;
        for base in &class.bases {
            offset -= 1;
            let symbol = self.find_interface(base)?;
            let module = &symbol.module.as_ref().unwrap().module_name;
            let methods = self.gen_interface_vtbl_methods(base)?;
            out.push_str(&format!(
                r#"
#[allow(non_upper_case_globals)]
pub const GLOBAL_{base}VirtualTable_CCW_FOR_{class_name}: {module}::{base}VirtualTableCcw
    = {module}::{base}VirtualTableCcw {{
    offset: {offset},
    vtable: {module}::{base}VirtualTable {{
{methods}
    }},
}};

"#,
                class_name = class.name
            ));
        }
        Ok(out)
    }

    fn gen_query_interface_branches(&self, class: &Class) -> Result<String, Error> {
        let mut out = String::new();
        let mut visited = HashSet::new();
        let mut offset = -1isize;
        for base in &class.bases {
            offset += 1;
            for interface in self.collect_inherit_chain(base)? {
                if !visited.insert(interface.name.clone()) {
                    continue;
                }
                let module = &interface.module.as_ref().unwrap().module_name;
                out.push_str(&format!(
                    r#"
&{module}::{name}::INTERFACE_ID => {{
    *retval = (object as *const *const std::os::raw::c_void).offset({offset});
    add_ref(object as *const *const std::os::raw::c_void);
    {crosscom}::ResultCode::Ok as std::os::raw::c_long
}}
"#,
                    name = interface.name,
                    crosscom = self.crosscom_module_name
                ));
            }
        }
        Ok(out)
    }

    fn gen_raw_method_impl_for_class(&self, class: &Class) -> Result<String, Error> {
        let mut out = String::new();
        for method in &class.methods {
            out.push_str(&self.gen_raw_method_impl(class, method)?);
        }

        let mut visited = HashSet::new();
        let mut ancestors = class.bases.clone();
        while let Some(ancestor) = ancestors.first().cloned() {
            ancestors.remove(0);
            if !visited.insert(ancestor.clone()) {
                continue;
            }
            let interface = self.find_interface(&ancestor)?;
            if interface.name == "IUnknown" {
                continue;
            }
            ancestors.extend(interface.bases.clone());
            for method in &interface.methods {
                let mut method = method.clone();
                method.interface_module = interface.module.clone();
                out.push_str(&self.gen_raw_method_impl(class, &method)?);
            }
        }
        Ok(out)
    }

    fn gen_raw_method_impl(&self, class: &Class, method: &Method) -> Result<String, Error> {
        let raw_params = self.gen_method_raw_params(method)?;
        let arg_names = method
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let field_name = class
            .attrs
            .get("rust_inner_field")
            .map(|field| format!(".{field}"))
            .unwrap_or_default();
        if method.attrs.contains_key("internal") {
            Ok(format!(
                r#"
    fn {method_name}(this: *const *const std::os::raw::c_void{raw_params}) -> {ret_ty} {{
        unsafe {{
            let __crosscom_object = {crosscom}::get_object::<{class_name}Ccw>(this);
            (*__crosscom_object).inner{field_name}.{method_name}({arg_names})
        }}
    }}
"#,
                method_name = method.name,
                ret_ty = method.ret_ty,
                crosscom = self.crosscom_module_name,
                class_name = class.name
            ))
        } else {
            let raw_ret = self.map_raw_type(&method.ret_ty, &[])?;
            let param_mapping = self.gen_method_param_mapping(method)?;
            let call_args = method
                .params
                .iter()
                .map(|param| format!("{}.into()", param.name))
                .collect::<Vec<_>>()
                .join(",");
            Ok(format!(
                r#"
    unsafe extern "system" fn {method_name}(this: *const *const std::os::raw::c_void{raw_params}) -> {raw_ret} {{
        {param_mapping}
        let __crosscom_object = {crosscom}::get_object::<{class_name}Ccw>(this);
        (*__crosscom_object).inner{field_name}.{method_name}({call_args}).into()
    }}
"#,
                method_name = method.name,
                crosscom = self.crosscom_module_name,
                class_name = class.name
            ))
        }
    }

    fn gen_interface(&self, interface: &Interface, out: &mut String) -> Result<(), Error> {
        if interface.codegen_ignore() {
            return Ok(());
        }

        out.push_str(&format!("// Interface {}\n\n", interface.name));
        out.push_str("#[repr(C)]\n#[allow(non_snake_case)]\n");
        out.push_str(&format!("pub struct {}VirtualTable {{\n", interface.name));
        for method in self.collect_all_methods(&interface.name, false)? {
            out.push_str(&format!(
                "    pub {}: {},\n",
                method.name,
                self.gen_method_raw_signature(&method)?
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!(
            r#"
#[repr(C)]
#[allow(dead_code)]
pub struct {name}VirtualTableCcw {{
    pub offset: isize,
    pub vtable: {name}VirtualTable,
}}

#[repr(C)]
#[allow(dead_code)]
pub struct {name} {{
    pub vtable: *const {name}VirtualTable,
}}

#[allow(dead_code)]
#[allow(non_snake_case)]
#[allow(unused)]
impl {name} {{
    {safe_wrappers}

    pub fn uuid() -> uuid::Uuid {{
        use crosscom::ComInterface;
        uuid::Uuid::from_bytes({name}::INTERFACE_ID)
    }}
}}
"#,
            name = interface.name,
            safe_wrappers = self.gen_interface_method_safe_wrapper(interface)?
        ));

        out.push_str(&format!("pub trait {}Impl {{\n", interface.name));
        for method in &interface.methods {
            let mut method = method.clone();
            method.interface_module = interface.module.clone();
            out.push_str(&format!("    {};\n", self.gen_method_signature2(&method)?));
        }
        out.push_str("}\n");

        let uuid = interface.attrs.get("uuid").ok_or_else(|| {
            Error::Generate(format!("interface {} is missing uuid", interface.name))
        })?;
        out.push_str(&format!(
            r#"
impl {crosscom}::ComInterface for {name} {{
    // {uuid}
    const INTERFACE_ID: [u8; 16] = {uuid_bytes};
}}

"#,
            crosscom = self.crosscom_module_name,
            name = interface.name,
            uuid_bytes = uuid_to_hex_array(uuid)?
        ));

        Ok(())
    }

    fn gen_interface_method_safe_wrapper(&self, interface: &Interface) -> Result<String, Error> {
        let mut out = String::new();
        for method in self.collect_all_methods(&interface.name, false)? {
            if method.name == "query_interface" {
                out.push_str(&format!(
                    r#"
pub fn query_interface<T: {crosscom}::ComInterface>(&self) -> Option<{crosscom}::ComRc<T>> {{
    let this = self as *const {interface_name} as *const *const std::os::raw::c_void;
    let mut raw = 0 as *const *const std::os::raw::c_void;
    let guid = uuid::Uuid::from_bytes(T::INTERFACE_ID);
    let ret_val = unsafe {{ ((*self.vtable).query_interface)(this, guid, &mut raw) }};
    if ret_val != 0 {{
        None
    }} else {{
        Some(unsafe {{ {crosscom}::ComRc::<T>::from_raw_pointer(raw) }})
    }}
}}
"#,
                    crosscom = self.crosscom_module_name,
                    interface_name = interface.name
                ));
            } else {
                let signature = self.gen_method_signature2(&method)?;
                let call_args = method
                    .params
                    .iter()
                    .map(|param| format!("{}.into()", param.name))
                    .collect::<Vec<_>>()
                    .join(",");
                let ret_mapping = self.gen_method_ret_mapping(&method)?;
                out.push_str(&format!(
                    r#"
pub {signature} {{
    unsafe {{
        let this = self as *const {interface_name} as *const *const std::os::raw::c_void;
        let ret = ((*self.vtable).{method_name})(this{call_args_prefix});
        {ret_mapping}
        ret
    }}
}}
"#,
                    interface_name = interface.name,
                    method_name = method.name,
                    call_args_prefix = if call_args.is_empty() {
                        String::new()
                    } else {
                        format!(", {call_args}")
                    }
                ));
            }
        }
        Ok(out)
    }

    fn gen_method_raw_signature(&self, method: &Method) -> Result<String, Error> {
        let params = self.gen_method_raw_params(method)?;
        if method.attrs.contains_key("internal") {
            Ok(format!(
                "fn(this: *const *const std::os::raw::c_void{params}) -> {}",
                method.ret_ty
            ))
        } else {
            Ok(format!(
                "unsafe extern \"system\" fn(this: *const *const std::os::raw::c_void{params}) -> {}",
                self.map_raw_type(&method.ret_ty, &[])?
            ))
        }
    }

    fn gen_method_raw_params(&self, method: &Method) -> Result<String, Error> {
        let mut out = String::new();
        for param in &method.params {
            let ty = if method.attrs.contains_key("internal") {
                self.map_rust_internal_type(
                    &param.ty,
                    method.interface_module.as_ref().ok_or_else(|| {
                        Error::Generate(format!(
                            "method {} does not have an interface module",
                            method.name
                        ))
                    })?,
                )?
            } else {
                self.map_raw_type(&param.ty, &param.attrs)?
            };
            out.push_str(&format!(", {}: {}", param.name, ty));
        }
        Ok(out)
    }

    fn gen_method_param_mapping(&self, method: &Method) -> Result<String, Error> {
        let mut out = String::new();
        for param in &method.params {
            if method.attrs.contains_key("internal") {
                let ty = self.map_rust_internal_type(
                    &param.ty,
                    method.interface_module.as_ref().ok_or_else(|| {
                        Error::Generate(format!(
                            "method {} does not have an interface module",
                            method.name
                        ))
                    })?,
                )?;
                out.push_str(&format!("let {}: {} = {};\n", param.name, ty, param.name));
            } else {
                out.push_str(&format!(
                    "let {}: {} = {};\n",
                    param.name,
                    self.map_type(&param.ty, true)?,
                    self.gen_param_ty_convert(param)
                ));
            }
        }
        Ok(out)
    }

    fn gen_method_ret_mapping(&self, method: &Method) -> Result<String, Error> {
        if method.attrs.contains_key("internal") {
            Ok(String::new())
        } else {
            Ok(format!(
                "let ret: {} = {};",
                self.map_type(&method.ret_ty, false)?,
                self.gen_ret_ty_convert(method)
            ))
        }
    }

    fn gen_method_signature2(&self, method: &Method) -> Result<String, Error> {
        let mut params = Vec::new();
        for param in &method.params {
            let ty = if method.attrs.contains_key("internal") {
                param.ty.clone()
            } else {
                self.map_type(&param.ty, true)?
            };
            params.push(format!("{}: {}", param.name, ty));
        }

        let ret_ty = if method.attrs.contains_key("internal") {
            method.ret_ty.clone()
        } else {
            self.map_type(&method.ret_ty, true)?
        };

        Ok(format!(
            "fn {}(&self, {}) -> {}",
            method.name,
            params.join(", "),
            ret_ty
        ))
    }

    fn gen_param_ty_convert(&self, param: &MethodParameter) -> String {
        match type_map(&param.ty).and_then(|mapped| mapped.2) {
            Some(conversion) => format!("{}{}", param.name, conversion),
            None => format!("{}.into()", param.name),
        }
    }

    fn gen_ret_ty_convert(&self, method: &Method) -> String {
        match type_map(&method.ret_ty).and_then(|mapped| mapped.2) {
            Some(conversion) => format!("ret{conversion}"),
            None => "ret.into()".to_string(),
        }
    }

    fn map_rust_internal_type(
        &self,
        rust_ty: &str,
        method_iface_module: &Module,
    ) -> Result<String, Error> {
        if rust_ty.contains("crate::") {
            let method_crate = method_iface_module.module_name.split("::").next().unwrap();
            let current_crate = rust_module(&self.unit)?
                .module_name
                .split("::")
                .next()
                .unwrap();
            if method_crate != current_crate {
                return Ok(rust_ty.replace("crate::", &format!("{method_crate}::")));
            }
        }
        Ok(rust_ty.to_string())
    }

    fn map_raw_type(&self, idl_ty: &str, attrs: &[String]) -> Result<String, Error> {
        let is_out = attrs.iter().any(|attr| attr == "out");
        if idl_ty.ends_with("[]") {
            Ok("*const *const std::os::raw::c_void".to_string())
        } else if idl_ty.ends_with('?') {
            Ok("crosscom::RawPointer".to_string())
        } else if let Some((raw, _, _)) = type_map(idl_ty) {
            Ok(raw.to_string())
        } else if self.interface_symbol(idl_ty)?.is_some() {
            if is_out {
                Ok("&mut *const *const std::os::raw::c_void".to_string())
            } else {
                Ok("*const *const std::os::raw::c_void".to_string())
            }
        } else {
            Err(Error::Generate(format!("cannot find type: {idl_ty}")))
        }
    }

    fn map_type(&self, idl_ty: &str, mod_prefix: bool) -> Result<String, Error> {
        if let Some(inner_idl_ty) = idl_ty.strip_suffix("[]") {
            if let Some(interface) = self.interface_symbol(inner_idl_ty)? {
                let module = &interface.module.as_ref().unwrap().module_name;
                return Ok(format!(
                    "{}::ObjectArray<{}::{}>",
                    self.crosscom_module_name, module, interface.name
                ));
            }
        } else if let Some(inner_idl_ty) = idl_ty.strip_suffix('?') {
            if let Some(interface) = self.interface_symbol(inner_idl_ty)? {
                let module = &interface.module.as_ref().unwrap().module_name;
                return Ok(format!(
                    "Option<crosscom::ComRc<{}::{}>>",
                    module, interface.name
                ));
            }
        } else if let Some((_, mapped, _)) = type_map(idl_ty) {
            return Ok(mapped.to_string());
        } else if let Some(interface) = self.interface_symbol(idl_ty)? {
            if mod_prefix {
                let module = &interface.module.as_ref().unwrap().module_name;
                return Ok(format!(
                    "{}::ComRc<{}::{}>",
                    self.crosscom_module_name, module, interface.name
                ));
            }
            return Ok(format!(
                "{}::ComRc<{}>",
                self.crosscom_module_name, interface.name
            ));
        }

        Err(Error::Generate(format!("cannot find type: {idl_ty}")))
    }
}

fn type_map(idl_ty: &str) -> Option<(&'static str, &'static str, Option<&'static str>)> {
    match idl_ty {
        "long" => Some(("std::os::raw::c_long", "std::os::raw::c_long", None)),
        "longlong" => Some(("std::os::raw::c_longlong", "std::os::raw::c_longlong", None)),
        "int" => Some(("std::os::raw::c_int", "std::os::raw::c_int", None)),
        "float" => Some(("std::os::raw::c_float", "f32", None)),
        "byte" => Some(("std::os::raw::c_uchar", "std::os::raw::c_uchar", None)),
        "byte*" => Some((
            "*const std::os::raw::c_uchar",
            "*const std::os::raw::c_uchar",
            None,
        )),
        "UUID" => Some(("uuid::Uuid", "uuid::Uuid", None)),
        "bool" => Some(("std::os::raw::c_int", "bool", Some(" != 0"))),
        "void" => Some(("()", "()", None)),
        _ => None,
    }
}

fn uuid_to_hex_array(id: &str) -> Result<String, Error> {
    let bytes = parse_uuid_bytes(id)?;
    Ok(format!(
        "[{}]",
        bytes
            .iter()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn parse_uuid_bytes(id: &str) -> Result<[u8; 16], Error> {
    let hex = id.replace('-', "");
    if hex.len() != 32 {
        return Err(Error::Generate(format!("invalid UUID: {id}")));
    }
    let mut bytes = [0u8; 16];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[idx * 2..idx * 2 + 2], 16)
            .map_err(|_| Error::Generate(format!("invalid UUID: {id}")))?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_repository_idls() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let idl_dir = manifest_dir.join("..").join("ccidl").join("idl");
        for idl_name in [
            "crosscom.idl",
            "editor.idl",
            "openpal3.idl",
            "openpal4.idl",
            "openpal5.idl",
            "openswd5.idl",
            "radiance.idl",
            "test.idl",
            "yaobow.idl",
            "yaobow_editor.idl",
        ] {
            let generated = generate(idl_dir.join(idl_name)).unwrap();
            assert!(!generated.source.is_empty(), "{idl_name}");
            assert!(!generated.dependencies.is_empty(), "{idl_name}");
        }
    }

    #[test]
    fn parses_uuid_bytes_in_network_order() {
        assert_eq!(
            parse_uuid_bytes("00000000-0000-0000-C000-000000000046").unwrap(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x46,
            ]
        );
    }
}
