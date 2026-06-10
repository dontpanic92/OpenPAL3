//! Script-bridge emitter for crosscom IDL.
//!
//! Emits the Rust glue that lets a script-side `struct[I]` be reverse-
//! wrapped as `ComRc<I>` via `crosscom-protosept`. For every interface
//! marked `[protosept(scriptable)]` (and not `codegen(ignore)`) we
//! generate, inside the same `*_comdef.rs` file as `RustGen`:
//!
//! * `pub fn register_<i>_proto()` — `OnceLock`-guarded; calls
//!   `crosscom_protosept::register_proto_ccw` with a `ProtoSpec` whose
//!   `methods` follow vtable order, `additional_query_uuids` carry the
//!   transitive non-IUnknown base UUIDs, and any interfaces referenced
//!   by foreign args/returns have their own `register_*_proto()`
//!   invoked first (idempotent).
//! * `pub fn wrap_<i>(handle, data) -> Result<ComRc<I>, HostError>` —
//!   calls `register_<i>_proto()` then `wrap_proto::<I>`.
//!
//! The function names follow `<verb>_<pascal_to_snake(name without
//! leading I)>` (e.g. `IDirector` → `register_director_proto`,
//! `wrap_director`; `IImmediateDirector` → `register_immediate_director_proto`,
//! `wrap_immediate_director`).
//!
//! Type mapping (must stay in lock-step with `protosept.rs` and
//! `crosscom_protosept::proto_ccw`):
//!
//! | IDL | ArgKind | RetKind |
//! | --- | ------- | ------- |
//! | `void` | (unsupported as arg) | `Void` |
//! | `bool` | `Bool` | `Bool` |
//! | `int`/`long`/`longlong`/`byte` | `Int` | `Int` |
//! | `float` | `Float` | `Float` |
//! | `&str`/`string` | `Str` | (unsupported in return) |
//! | `IFoo` | `Foreign { type_tag, uuid }` | `Foreign { type_tag, uuid }` |
//! | `IFoo?` | (unsupported — `ArgKind::OptionalForeign` not yet implemented) | `OptionalForeign { type_tag, uuid }` |
//!
//! Any other shape on a `[protosept(scriptable)]` interface — arrays,
//! `UUID`, raw pointer types — is a hard build error so the IDL author
//! can either drop the `scriptable` flag or extend the marshalling
//! layer.

use std::collections::{HashMap, HashSet};

use crate::{CrossComIdl, Error, Interface, Item, Module, Symbol, rust_module};

pub(crate) fn generate(
    unit: &CrossComIdl,
    consumer_crate: &str,
    bridge_root: &str,
    local_bridge_stem: &str,
) -> Result<String, Error> {
    ScriptBridgeGen::new(unit, consumer_crate, bridge_root, local_bridge_stem)?.r#gen()
}

struct ScriptBridgeGen<'a> {
    unit: &'a CrossComIdl,
    symbols: HashMap<String, Symbol>,
    current_module: Module,
    consumer_crate: String,
    bridge_root: String,
    /// Filename stem of the IDL we're generating for (e.g.
    /// `radiance` for `radiance.idl`). Used as the local bridge
    /// module name in the consumer crate.
    local_bridge_stem: String,
}

impl<'a> ScriptBridgeGen<'a> {
    fn new(
        unit: &'a CrossComIdl,
        consumer_crate: &str,
        bridge_root: &str,
        local_bridge_stem: &str,
    ) -> Result<Self, Error> {
        let current_module = rust_module(unit)?.clone();
        let mut symbols: HashMap<String, Symbol> = HashMap::new();
        for item in &unit.items {
            match item {
                Item::Interface(interface) => {
                    symbols.insert(interface.name.clone(), Symbol::Interface(interface.clone()));
                }
                Item::Class(class) => {
                    symbols.insert(class.name.clone(), Symbol::Class);
                }
            }
        }
        Ok(Self {
            unit,
            symbols,
            current_module,
            consumer_crate: consumer_crate.to_string(),
            bridge_root: bridge_root.to_string(),
            local_bridge_stem: local_bridge_stem.to_string(),
        })
    }

    fn r#gen(&self) -> Result<String, Error> {
        let mut local_scriptable: Vec<&Interface> = Vec::new();
        for item in &self.unit.items {
            if let Item::Interface(interface) = item {
                if interface.codegen_ignore() {
                    continue;
                }
                if interface.lang_flag("protosept", "scriptable") {
                    local_scriptable.push(interface);
                }
            }
        }
        if local_scriptable.is_empty() {
            return Ok(String::new());
        }

        let mut out = String::new();
        out.push_str("\n// --- Script bridge (auto-generated from [protosept(scriptable)]) ---\n");
        // Each scriptable interface gets `register_<i>_proto()` (the
        // IDL-derived ProtoSpec / COM-vtable metadata) and `wrap_<i>()`
        // (reverse-wrap a script struct into a real `ComRc<I>`). That's
        // the entire script-bridge surface — no typed clients, no
        // per-arg intern helpers, no aggregate registration entry.
        for interface in &local_scriptable {
            self.gen_interface(interface, &mut out)?;
        }
        Ok(out)
    }

    fn gen_interface(&self, interface: &Interface, out: &mut String) -> Result<(), Error> {
        let snake = pascal_to_snake_drop_leading_i(&interface.name);
        let register_fn = format!("register_{snake}_proto");
        let wrap_fn = format!("wrap_{snake}");

        let methods = self.collect_public_methods(&interface.name)?;
        let bases = self.collect_transitive_bases(&interface.name)?;

        // Collect deps that need register_*_proto calls: only
        // *scriptable* interfaces (the only ones for which a
        // `register_*_proto` exists). Walk bases + every Foreign /
        // OptionalForeign reference; skip self.
        let mut deps: Vec<&Interface> = Vec::new();
        let mut dep_seen: HashSet<String> = HashSet::new();
        for base in &bases {
            if base.name == interface.name {
                continue;
            }
            if base.lang_flag("protosept", "scriptable") && dep_seen.insert(base.name.clone()) {
                deps.push(*base);
            }
        }
        for (_, method) in &methods {
            for p in &method.params {
                if let Some(iface) = self.referenced_interface(&p.ty)? {
                    if iface.name != interface.name
                        && iface.lang_flag("protosept", "scriptable")
                        && dep_seen.insert(iface.name.clone())
                    {
                        deps.push(iface);
                    }
                }
            }
            if let Some(iface) = self.referenced_interface(&method.ret_ty)? {
                if iface.name != interface.name
                    && iface.lang_flag("protosept", "scriptable")
                    && dep_seen.insert(iface.name.clone())
                {
                    deps.push(iface);
                }
            }
        }

        let self_path = self.interface_rust_path(interface);
        let self_type_tag = self.interface_type_tag(interface);

        out.push_str(&format!(
            "\n#[allow(non_snake_case, dead_code)]\npub fn {register_fn}() {{\n",
        ));
        out.push_str(
            "    static GUARD: ::std::sync::OnceLock<()> = ::std::sync::OnceLock::new();\n",
        );
        out.push_str("    GUARD.get_or_init(|| {\n");
        for dep in &deps {
            let dep_snake = pascal_to_snake_drop_leading_i(&dep.name);
            let dep_register = self.register_fn_path(dep, &dep_snake);
            out.push_str(&format!("        {dep_register}();\n"));
        }
        out.push_str(
            "        let _ = ::crosscom_protosept::register_proto_ccw(::crosscom_protosept::ProtoSpec {\n",
        );
        out.push_str(&format!(
            "            uuid: <{self_path} as ::crosscom::ComInterface>::INTERFACE_ID,\n",
        ));
        out.push_str(&format!(
            "            type_tag: {:?}.to_string(),\n",
            self_type_tag
        ));
        out.push_str("            methods: vec![\n");
        for (origin_iface, method) in &methods {
            let (args_lit, ret_lit) =
                self.method_marshalling(&interface.name, origin_iface, method)?;
            out.push_str("                ::crosscom_protosept::MethodSpec {\n");
            out.push_str(&format!(
                "                    name: {:?}.to_string(),\n",
                method.name
            ));
            out.push_str(&format!("                    args: {args_lit},\n"));
            out.push_str(&format!("                    ret: {ret_lit},\n"));
            out.push_str("                },\n");
        }
        out.push_str("            ],\n");
        out.push_str("            additional_query_uuids: vec![\n");
        for base in &bases {
            if base.name == interface.name {
                continue;
            }
            let base_path = self.interface_rust_path(base);
            out.push_str(&format!(
                "                <{base_path} as ::crosscom::ComInterface>::INTERFACE_ID,\n",
            ));
        }
        out.push_str("            ],\n");
        out.push_str("        });\n");
        out.push_str("    });\n");
        out.push_str("}\n");

        out.push_str(&format!(
            "\n#[allow(non_snake_case, dead_code)]\npub fn {wrap_fn}(\n    \
             handle: &::crosscom_protosept::RuntimeHandle,\n    \
             data: ::p7::interpreter::context::Data,\n\
             ) -> ::std::result::Result<::crosscom::ComRc<{self_path}>, ::crosscom_protosept::HostError> {{\n",
        ));
        out.push_str(&format!("    {register_fn}();\n"));
        out.push_str(&format!(
            "    ::crosscom_protosept::wrap_proto::<{self_path}>(handle, data)\n"
        ));
        out.push_str("}\n");

        Ok(())
    }

    /// All public methods of `iname`, walking bases the same way
    /// `protosept.rs` does. Returns (origin_interface_name, method).
    fn collect_public_methods(&self, iname: &str) -> Result<Vec<(String, crate::Method)>, Error> {
        let interface = self.find_interface(iname)?;
        let mut methods: Vec<(String, crate::Method)> = Vec::new();
        if let Some(base_name) = interface.bases.first() {
            if interface.bases.len() > 1 {
                return Err(Error::Generate(format!(
                    "cannot have more than one parent for interface: {}",
                    interface.name
                )));
            }
            let base = self.find_interface(base_name)?;
            let synthetic_root = base.bases.is_empty() && base.codegen_ignore();
            if !synthetic_root {
                methods.extend(self.collect_public_methods(base_name)?);
            }
        }
        for method in interface.public_methods() {
            methods.push((interface.name.clone(), method));
        }
        Ok(methods)
    }

    /// All transitive non-IUnknown bases of `iname`. The synthetic
    /// root (`IUnknown`) is excluded. Order: outermost ancestor first.
    fn collect_transitive_bases(&self, iname: &str) -> Result<Vec<&Interface>, Error> {
        let interface = self.find_interface(iname)?;
        let mut chain: Vec<&Interface> = Vec::new();
        if let Some(base_name) = interface.bases.first() {
            let base = self.find_interface(base_name)?;
            let synthetic_root = base.bases.is_empty() && base.codegen_ignore();
            if !synthetic_root {
                chain.extend(self.collect_transitive_bases(base_name)?);
                chain.push(base);
            }
        }
        Ok(chain)
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

    /// If `idl_ty` references an interface (bare, `?`, or `[]`),
    /// return that interface.
    fn referenced_interface(&self, idl_ty: &str) -> Result<Option<&Interface>, Error> {
        let trimmed = idl_ty.trim();
        let inner = trimmed
            .strip_suffix("[]")
            .or_else(|| trimmed.strip_suffix('?'))
            .unwrap_or(trimmed)
            .trim();
        if inner == "IUnknown" || inner == "void" {
            return Ok(None);
        }
        match self.symbols.get(inner) {
            Some(Symbol::Interface(interface)) => Ok(Some(self.find_interface(&interface.name)?)),
            _ => Ok(None),
        }
    }

    fn interface_rust_path(&self, interface: &Interface) -> String {
        let module = interface.module.as_ref().unwrap_or(&self.current_module);
        self.path_for(&module.module_name, &interface.name)
    }

    /// Path to the bridge's `register_<dep>_proto` function. The
    /// bridge function does *not* live next to the type definition —
    /// it lives in the consumer crate at
    /// `crate::<bridge_root>::<idl_stem>::register_<dep>_proto`.
    /// `<idl_stem>` is taken from the dep's `idl_origin` attr when
    /// it's an import; for locally-declared deps we use the current
    /// IDL's stem (the file we're generating for).
    fn register_fn_path(&self, interface: &Interface, snake: &str) -> String {
        let stem = interface
            .attrs
            .get("idl_origin")
            .map(String::as_str)
            .unwrap_or(&self.local_bridge_stem);
        if stem == self.local_bridge_stem {
            // Sibling function in the same emitted file.
            format!("register_{snake}_proto")
        } else {
            format!(
                "crate::{root}::{stem}::register_{snake}_proto",
                root = self.bridge_root
            )
        }
    }

    /// Compose a Rust path to `item` declared in `module_name`. Uses
    /// `crate::...` for items in the consumer crate, otherwise an
    /// absolute external-crate path.
    fn path_for(&self, module_name: &str, item: &str) -> String {
        let (crate_name, rest) = match module_name.split_once("::") {
            Some((c, r)) => (c, Some(r)),
            None => (module_name, None),
        };
        if crate_name == self.consumer_crate {
            match rest {
                Some(r) => format!("crate::{r}::{item}"),
                None => format!("crate::{item}"),
            }
        } else {
            format!("::{module_name}::{item}")
        }
    }

    fn interface_type_tag(&self, interface: &Interface) -> String {
        let module = interface.module.as_ref().unwrap_or(&self.current_module);
        format!(
            "{}.{}",
            module.module_name.replace("::", "."),
            interface.name
        )
    }

    /// Build the args and ret literals for a method. Errors out
    /// loudly when any type can't be mapped, naming the offending
    /// interface/method/type.
    fn method_marshalling(
        &self,
        iface_name: &str,
        origin_iface: &str,
        method: &crate::Method,
    ) -> Result<(String, String), Error> {
        let mut args_buf = String::from("vec![");
        for (i, p) in method.params.iter().enumerate() {
            if i > 0 {
                args_buf.push_str(", ");
            }
            args_buf.push_str(&self.idl_to_arg_kind(iface_name, origin_iface, method, &p.ty)?);
        }
        args_buf.push(']');

        let ret_lit = self.idl_to_ret_kind(iface_name, origin_iface, method, &method.ret_ty)?;
        Ok((args_buf, ret_lit))
    }

    fn idl_to_arg_kind(
        &self,
        iface_name: &str,
        origin_iface: &str,
        method: &crate::Method,
        idl_ty: &str,
    ) -> Result<String, Error> {
        let trimmed = idl_ty.trim();
        match trimmed {
            "bool" => return Ok("::crosscom_protosept::ArgKind::Bool".to_string()),
            "int" | "long" | "longlong" | "byte" => {
                return Ok("::crosscom_protosept::ArgKind::Int".to_string());
            }
            "float" => return Ok("::crosscom_protosept::ArgKind::Float".to_string()),
            "&str" | "string" => return Ok("::crosscom_protosept::ArgKind::Str".to_string()),
            _ => {}
        }

        // Bare foreign interface (e.g. `IUiHost`) — supported.
        if let Some(iface) = self.referenced_interface(trimmed)? {
            if !trimmed.ends_with('?') && !trimmed.ends_with("[]") {
                let path = self.interface_rust_path(iface);
                let tag = self.interface_type_tag(iface);
                return Ok(format!(
                    "::crosscom_protosept::ArgKind::Foreign {{ type_tag: {:?}.to_string(), uuid: <{path} as ::crosscom::ComInterface>::INTERFACE_ID }}",
                    tag
                ));
            }
        }

        Err(Error::Generate(format!(
            "interface {iface_name}: method {origin_iface}.{m} parameter type {idl_ty:?} is \
             not currently supported by [protosept(scriptable)]; drop the attribute or extend \
             the marshalling layer",
            m = method.name,
        )))
    }

    fn idl_to_ret_kind(
        &self,
        iface_name: &str,
        origin_iface: &str,
        method: &crate::Method,
        idl_ty: &str,
    ) -> Result<String, Error> {
        let trimmed = idl_ty.trim();
        match trimmed {
            "void" => return Ok("::crosscom_protosept::RetKind::Void".to_string()),
            "bool" => return Ok("::crosscom_protosept::RetKind::Bool".to_string()),
            "int" | "long" | "longlong" | "byte" => {
                return Ok("::crosscom_protosept::RetKind::Int".to_string());
            }
            "float" => return Ok("::crosscom_protosept::RetKind::Float".to_string()),
            _ => {}
        }

        if let Some(inner) = trimmed.strip_suffix('?') {
            let inner = inner.trim();
            // Nullable primitive: `?float` → OptionalFloat with NaN
            // sentinel on the C ABI. The runtime decodes NaN → null.
            if inner == "float" {
                return Ok("::crosscom_protosept::RetKind::OptionalFloat".to_string());
            }
            if let Some(iface) = self.referenced_interface(inner)? {
                let path = self.interface_rust_path(iface);
                let tag = self.interface_type_tag(iface);
                return Ok(format!(
                    "::crosscom_protosept::RetKind::OptionalForeign {{ type_tag: {:?}.to_string(), uuid: <{path} as ::crosscom::ComInterface>::INTERFACE_ID }}",
                    tag
                ));
            }
        }

        // Non-optional foreign interface return (e.g. `IImmediateDirector`).
        // Maps to `RetKind::Foreign`: the script's returned box is recursively
        // reverse-wrapped and a `null`/absent return is a hard error. Arrays
        // (`IFoo[]`) remain unsupported.
        if !trimmed.ends_with("[]") {
            if let Some(iface) = self.referenced_interface(trimmed)? {
                let path = self.interface_rust_path(iface);
                let tag = self.interface_type_tag(iface);
                return Ok(format!(
                    "::crosscom_protosept::RetKind::Foreign {{ type_tag: {:?}.to_string(), uuid: <{path} as ::crosscom::ComInterface>::INTERFACE_ID }}",
                    tag
                ));
            }
        }

        Err(Error::Generate(format!(
            "interface {iface_name}: method {origin_iface}.{m} return type {idl_ty:?} is \
             not currently supported by [protosept(scriptable)]; drop the attribute or extend \
             the marshalling layer (string returns and foreign-array returns are not \
             yet implemented)",
            m = method.name,
        )))
    }
}

pub(crate) fn pascal_to_snake_drop_leading_i(name: &str) -> String {
    let body: &str = if name.starts_with('I')
        && name
            .chars()
            .nth(1)
            .map_or(false, |c| c.is_ascii_uppercase())
    {
        &name[1..]
    } else {
        name
    };
    let mut out = String::new();
    let mut prev_lower_or_digit = false;
    for c in body.chars() {
        if c.is_ascii_uppercase() {
            if prev_lower_or_digit {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_lower_or_digit = false;
        } else {
            out.push(c);
            prev_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_conversion_matches_expected() {
        assert_eq!(pascal_to_snake_drop_leading_i("IAction"), "action");
        assert_eq!(pascal_to_snake_drop_leading_i("IDirector"), "director");
        assert_eq!(
            pascal_to_snake_drop_leading_i("IImmediateDirector"),
            "immediate_director"
        );
        assert_eq!(
            pascal_to_snake_drop_leading_i("IPal4DebugOverlay"),
            "pal4_debug_overlay"
        );
        // Names without the leading-I-followed-by-uppercase pattern
        // stay as-is (lowercased PascalCase).
        assert_eq!(pascal_to_snake_drop_leading_i("Plain"), "plain");
    }

    /// A `[protosept(scriptable)]` interface with a non-optional
    /// `IFoo` return must emit `RetKind::Foreign` (Phase 1/2). Before
    /// `RetKind::Foreign` existed this was a hard build error.
    #[test]
    fn non_optional_foreign_return_emits_retkind_foreign() {
        let idl = r#"
module(rust) test::comdef;

[uuid(00000000-0000-0000-0000-0000000000ff), codegen(ignore)]
interface IUnknown {}

[uuid(11111111-1111-1111-1111-111111111111), protosept(scriptable)]
interface IThing: IUnknown {
    IThing make_thing();
    IThing? maybe_thing();
}
"#;
        let unit = crate::parse_source(idl).expect("parse");
        let unit = crate::RustGen::new(unit).expect("rustgen").into_unit();
        let src = generate(&unit, "test", "script_bridges", "test").expect("generate");

        // Non-optional return → RetKind::Foreign.
        assert!(
            src.contains("::crosscom_protosept::RetKind::Foreign {"),
            "expected RetKind::Foreign in:\n{src}"
        );
        // Optional return → RetKind::OptionalForeign (regression guard).
        assert!(
            src.contains("::crosscom_protosept::RetKind::OptionalForeign {"),
            "expected RetKind::OptionalForeign in:\n{src}"
        );
        // Both carry the IThing type tag.
        assert!(
            src.contains("test.comdef.IThing"),
            "expected IThing type_tag in:\n{src}"
        );
    }
}
