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
//! | `IFoo` | `Foreign { type_tag, uuid }` | (unsupported — `RetKind::Foreign` not yet implemented) |
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
        let mut local_app_roots: Vec<&Interface> = Vec::new();
        let mut local_intern_targets: Vec<&Interface> = Vec::new();
        for item in &self.unit.items {
            if let Item::Interface(interface) = item {
                if interface.codegen_ignore() {
                    continue;
                }
                if interface.lang_flag("protosept", "scriptable") {
                    local_scriptable.push(interface);
                }
                if interface.lang_flag("protosept", "script_app_root") {
                    local_app_roots.push(interface);
                }
                // Every locally-declared, non-ignored interface gets
                // an `intern_<i>_arg` helper so call sites passing it
                // into a script method don't need the magic type_tag
                // string. Non-scriptable interfaces (e.g.
                // `IPal4GameContext`, `IInputService`) are passed as
                // foreign boxes too — they just don't have a
                // `register_<i>_proto` / `wrap_<i>` companion.
                local_intern_targets.push(interface);
            }
        }
        if local_scriptable.is_empty()
            && local_app_roots.is_empty()
            && local_intern_targets.is_empty()
        {
            return Ok(String::new());
        }

        let mut out = String::new();
        out.push_str("\n// --- Script bridge (auto-generated from [protosept(scriptable)]) ---\n");
        for interface in &local_scriptable {
            self.gen_interface(interface, &mut out)?;
        }
        for interface in &local_intern_targets {
            self.gen_intern_arg(interface, &mut out)?;
        }
        for interface in &local_app_roots {
            self.gen_script_app_client(interface, &mut out)?;
        }
        self.gen_register_all_protos(&local_scriptable, &mut out);
        Ok(out)
    }

    /// Emits `pub fn intern_<i>_arg(host, rc) -> Result<Data, HostError>`
    /// for every locally-declared interface. Lets typed clients and
    /// hand-written call sites pass a `ComRc<I>` into a script method
    /// without hard-coding the IDL-derived `type_tag` string.
    fn gen_intern_arg(&self, interface: &Interface, out: &mut String) -> Result<(), Error> {
        let snake = pascal_to_snake_drop_leading_i(&interface.name);
        let fn_name = format!("intern_{snake}_arg");
        let self_path = self.interface_rust_path(interface);
        let tag = self.interface_type_tag(interface);
        let script_host_path = self.script_host_path();

        out.push_str(&format!(
            "\n#[allow(non_snake_case, dead_code)]\n\
             pub fn {fn_name}(\n    \
             host: &{script_host_path},\n    \
             rc: ::crosscom::ComRc<{self_path}>,\n\
             ) -> ::std::result::Result<::p7::interpreter::context::Data, ::crosscom_protosept::HostError> {{\n",
        ));
        out.push_str(&format!("    let id = host.intern(rc);\n"));
        out.push_str(&format!("    host.foreign_box({:?}, id)\n", tag));
        out.push_str("}\n");
        Ok(())
    }

    /// Emits `pub fn register_all_protos()` that calls every
    /// `register_<i>_proto()` in the file in IDL order. Each
    /// individual `register_<i>_proto` already registers its own
    /// deps idempotently, so IDL order is sufficient.
    fn gen_register_all_protos(&self, local_scriptable: &[&Interface], out: &mut String) {
        out.push_str(
            "\n/// Calls every `register_<i>_proto()` in this bridge file once.\n\
             /// Used by host setup paths (e.g. `ScriptHost::install`) to ensure\n\
             /// every scriptable interface declared in this IDL is registered\n\
             /// before any wrap/dispatch occurs.\n\
             #[allow(non_snake_case, dead_code)]\n\
             pub fn register_all_protos() {\n",
        );
        for interface in local_scriptable {
            let snake = pascal_to_snake_drop_leading_i(&interface.name);
            out.push_str(&format!("    register_{snake}_proto();\n"));
        }
        out.push_str("}\n");
    }

    /// Emits a typed Rust client for an `[protosept(script_app_root)]`
    /// interface: a struct holding `Rc<ScriptHost>` + a rooted
    /// `ScriptDirectorHandle`, with one Rust method per IDL method.
    ///
    /// Each generated method:
    ///   * deref the rooted app handle (returns `HostError` if the
    ///     handle was invalidated by `ScriptHost::reload`),
    ///   * intern + foreign-box every `ComRc<IArg>` parameter using
    ///     the sibling `intern_<i>_arg` helpers (paths resolved via
    ///     `intern_arg_fn_path`),
    ///   * `call_method_returning_data(app, "<method>", args)`,
    ///   * reverse-wrap the returned `Data` via the matching
    ///     `wrap_<i>` (paths via `wrap_fn_path`).
    ///
    /// Bare primitive returns and string returns are intentionally
    /// rejected by codegen for now — `yaobow_services.idl` only
    /// returns foreign interfaces, and a primitive-extraction
    /// strategy that matches `p7::Data` variants exactly needs its
    /// own shared helper before it's safe to emit.
    fn gen_script_app_client(&self, interface: &Interface, out: &mut String) -> Result<(), Error> {
        let client_name = format!("{}Client", interface.name);
        let methods = self.collect_public_methods(&interface.name)?;
        let script_host_path = self.script_host_path();
        let handle_path = self.script_handle_path();

        out.push_str(&format!(
            "\n#[allow(non_snake_case, dead_code)]\npub struct {client_name} {{\n    \
             host: ::std::rc::Rc<{script_host_path}>,\n    \
             app: {handle_path},\n\
             }}\n",
        ));

        // Constructor + Drop. We always unroot on Drop because clients
        // own the rooted handle; engine-cached singletons (yaobow's
        // `YaobowScriptProject`) keep the `Rc<...Client>` alive as
        // long as the host, so this is a no-op in practice for them.
        out.push_str(&format!(
            "\nimpl {client_name} {{\n    \
             #[allow(dead_code)]\n    \
             pub fn new(\n        \
             host: ::std::rc::Rc<{script_host_path}>,\n        \
             app: {handle_path},\n    \
             ) -> ::std::rc::Rc<Self> {{\n        \
             ::std::rc::Rc::new(Self {{ host, app }})\n    \
             }}\n\n    \
             #[allow(dead_code)]\n    \
             pub fn host(&self) -> &::std::rc::Rc<{script_host_path}> {{ &self.host }}\n\n    \
             #[allow(dead_code)]\n    \
             pub fn app_handle(&self) -> {handle_path} {{ self.app }}\n",
        ));

        for (origin_iface, method) in &methods {
            self.gen_script_app_method(&interface.name, origin_iface, method, out)?;
        }

        out.push_str("}\n");

        out.push_str(&format!(
            "\nimpl ::std::ops::Drop for {client_name} {{\n    \
             fn drop(&mut self) {{ self.host.unroot(self.app); }}\n\
             }}\n",
        ));

        Ok(())
    }

    fn gen_script_app_method(
        &self,
        iface_name: &str,
        origin_iface: &str,
        method: &crate::Method,
        out: &mut String,
    ) -> Result<(), Error> {
        // Build Rust param list + body fragments.
        let mut params_rust = String::new();
        let mut intern_lines = String::new();
        let mut arg_idents = Vec::new();

        for (idx, p) in method.params.iter().enumerate() {
            let rust_ty = self.idl_to_client_arg_rust(iface_name, origin_iface, method, &p.ty)?;
            let ident = format!("arg{idx}");
            params_rust.push_str(&format!(", {ident}: {rust_ty}"));

            // Foreign args go through `intern_<i>_arg`; primitives are
            // pushed as `Data` literals directly. We classify by
            // looking at the IDL type again.
            let trimmed = p.ty.trim();
            if let Some(iface) = self.referenced_interface(trimmed)? {
                if !trimmed.ends_with('?') && !trimmed.ends_with("[]") {
                    let snake = pascal_to_snake_drop_leading_i(&iface.name);
                    let intern_path = self.intern_arg_fn_path(iface, &snake);
                    intern_lines.push_str(&format!(
                        "        let {ident}_box = {intern_path}(&self.host, {ident})?;\n",
                    ));
                    arg_idents.push(format!("{ident}_box"));
                    continue;
                }
            }
            // Primitive Data conversions (best-effort for the shapes
            // we currently need). Currently no script_app_root method
            // takes a primitive arg, so this path is exercised only
            // when a future IDL adds one.
            let lit = match trimmed {
                "bool" => format!(
                    "        let {ident}_box = ::p7::interpreter::context::Data::Bool({ident});\n",
                ),
                "int" | "long" | "longlong" | "byte" => format!(
                    "        let {ident}_box = ::p7::interpreter::context::Data::Int({ident} as i64);\n",
                ),
                "float" => format!(
                    "        let {ident}_box = ::p7::interpreter::context::Data::Float({ident} as f64);\n",
                ),
                "&str" | "string" => format!(
                    "        let {ident}_box = ::p7::interpreter::context::Data::String(::std::string::String::from({ident}).into());\n",
                ),
                _ => {
                    return Err(Error::Generate(format!(
                        "script_app_root client {iface_name}.{m}: parameter type {ty:?} is not \
                         supported by the typed-client emitter; foreign interfaces, bool, \
                         int/long/longlong/byte, float, and string are currently allowed",
                        m = method.name,
                        ty = p.ty,
                    )));
                }
            };
            intern_lines.push_str(&lit);
            arg_idents.push(format!("{ident}_box"));
        }

        // Return classification.
        let ret_ty_rust = self.idl_to_client_ret_rust(iface_name, origin_iface, method)?;

        out.push_str(&format!(
            "\n    #[allow(dead_code)]\n    pub fn {m}(&self{params_rust}) -> {ret_ty_rust} {{\n        \
             let app_data = self.host.deref_handle(self.app).ok_or_else(|| ::crosscom_protosept::HostError::message({err:?}))?;\n",
            m = method.name,
            err = format!("script app handle invalidated (call to {iface_name}.{m})", m = method.name),
        ));
        out.push_str(&intern_lines);

        let args_vec = if arg_idents.is_empty() {
            "::std::vec::Vec::new()".to_string()
        } else {
            format!("vec![{}]", arg_idents.join(", "))
        };

        // Dispatch: void or value-returning?
        let trimmed_ret = method.ret_ty.trim();
        if trimmed_ret == "void" {
            out.push_str(&format!(
                "        self.host.call_method_void(app_data, {name:?}, {args_vec})\n",
                name = method.name,
            ));
        } else {
            out.push_str(&format!(
                "        let result = self.host.call_method_returning_data(app_data, {name:?}, {args_vec})?;\n",
                name = method.name,
            ));
            // Foreign return → wrap_<i>.
            if let Some(iface) = self.referenced_interface(trimmed_ret)? {
                let snake = pascal_to_snake_drop_leading_i(&iface.name);
                let wrap_path = self.wrap_fn_path(iface, &snake);
                if trimmed_ret.ends_with('?') {
                    // OptionalForeign: wrap_<i> currently returns
                    // Err(...) on Null. Translate that to Ok(None).
                    out.push_str("        let h = self.host.runtime_handle();\n");
                    out.push_str(&format!(
                        "        match {wrap_path}(&h, result) {{\n            \
                         Ok(rc) => Ok(Some(rc)),\n            \
                         Err(err) => {{\n                \
                         if format!(\"{{:?}}\", err).contains(\"null\") || \
                                  format!(\"{{:?}}\", err).contains(\"Null\") {{\n                    \
                         Ok(None)\n                \
                         }} else {{\n                    \
                         Err(err)\n                \
                         }}\n            \
                         }}\n        \
                         }}\n"
                    ));
                } else {
                    out.push_str("        let h = self.host.runtime_handle();\n");
                    out.push_str(&format!("        {wrap_path}(&h, result)\n"));
                }
            } else {
                // Primitive return — only emitted on the "stretch"
                // shapes; primary use case (yaobow_services.idl)
                // doesn't hit this path. Surface a clean error so a
                // future maintainer doesn't get confused by silent
                // garbage extraction.
                return Err(Error::Generate(format!(
                    "script_app_root client {iface_name}.{m}: return type {ty:?} is not \
                     supported by the typed-client emitter; foreign interfaces (bare or `?`) \
                     and `void` are currently allowed",
                    m = method.name,
                    ty = method.ret_ty,
                )));
            }
        }

        out.push_str("    }\n");
        Ok(())
    }

    fn idl_to_client_arg_rust(
        &self,
        iface_name: &str,
        _origin_iface: &str,
        method: &crate::Method,
        idl_ty: &str,
    ) -> Result<String, Error> {
        let trimmed = idl_ty.trim();
        match trimmed {
            "bool" => return Ok("bool".to_string()),
            "int" | "long" => return Ok("i32".to_string()),
            "longlong" => return Ok("i64".to_string()),
            "byte" => return Ok("u8".to_string()),
            "float" => return Ok("f32".to_string()),
            "&str" | "string" => return Ok("&str".to_string()),
            _ => {}
        }
        if let Some(iface) = self.referenced_interface(trimmed)? {
            if !trimmed.ends_with('?') && !trimmed.ends_with("[]") {
                let path = self.interface_rust_path(iface);
                return Ok(format!("::crosscom::ComRc<{path}>"));
            }
        }
        Err(Error::Generate(format!(
            "script_app_root client {iface_name}.{m}: parameter type {idl_ty:?} is not \
             supported by the typed-client emitter",
            m = method.name,
        )))
    }

    fn idl_to_client_ret_rust(
        &self,
        iface_name: &str,
        origin_iface: &str,
        method: &crate::Method,
    ) -> Result<String, Error> {
        let trimmed = method.ret_ty.trim();
        if trimmed == "void" {
            return Ok("::std::result::Result<(), ::crosscom_protosept::HostError>".to_string());
        }
        if let Some(iface) = self.referenced_interface(trimmed)? {
            let path = self.interface_rust_path(iface);
            if trimmed.ends_with('?') {
                return Ok(format!(
                    "::std::result::Result<::std::option::Option<::crosscom::ComRc<{path}>>, ::crosscom_protosept::HostError>"
                ));
            }
            return Ok(format!(
                "::std::result::Result<::crosscom::ComRc<{path}>, ::crosscom_protosept::HostError>"
            ));
        }
        let _ = origin_iface;
        Err(Error::Generate(format!(
            "script_app_root client {iface_name}.{m}: return type {ty:?} is not supported by \
             the typed-client emitter (foreign interfaces and void only)",
            m = method.name,
            ty = method.ret_ty,
        )))
    }

    /// Path to `radiance_scripting::ScriptHost`, resolved from the
    /// consumer crate. Uses `crate::ScriptHost` when the consumer is
    /// `radiance_scripting` itself (so the generated bridge inside
    /// that crate doesn't try to import itself externally).
    fn script_host_path(&self) -> String {
        if self.consumer_crate == "radiance_scripting" {
            "crate::ScriptHost".to_string()
        } else {
            "::radiance_scripting::ScriptHost".to_string()
        }
    }

    /// Path to `radiance_scripting::ScriptDirectorHandle`, with the
    /// same crate-aware resolution as [`Self::script_host_path`].
    fn script_handle_path(&self) -> String {
        if self.consumer_crate == "radiance_scripting" {
            "crate::ScriptDirectorHandle".to_string()
        } else {
            "::radiance_scripting::ScriptDirectorHandle".to_string()
        }
    }

    /// Sibling path to `intern_<i>_arg` for `interface`. Mirrors
    /// [`register_fn_path`] — local if `interface` lives in the
    /// current IDL, cross-bridge otherwise.
    fn intern_arg_fn_path(&self, interface: &Interface, snake: &str) -> String {
        let stem = interface
            .attrs
            .get("idl_origin")
            .map(String::as_str)
            .unwrap_or(&self.local_bridge_stem);
        if stem == self.local_bridge_stem {
            format!("intern_{snake}_arg")
        } else {
            format!(
                "crate::{root}::{stem}::intern_{snake}_arg",
                root = self.bridge_root
            )
        }
    }

    /// Sibling path to `wrap_<i>` for `interface`. Same shape as
    /// [`register_fn_path`].
    fn wrap_fn_path(&self, interface: &Interface, snake: &str) -> String {
        let stem = interface
            .attrs
            .get("idl_origin")
            .map(String::as_str)
            .unwrap_or(&self.local_bridge_stem);
        if stem == self.local_bridge_stem {
            format!("wrap_{snake}")
        } else {
            format!(
                "crate::{root}::{stem}::wrap_{snake}",
                root = self.bridge_root
            )
        }
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

        Err(Error::Generate(format!(
            "interface {iface_name}: method {origin_iface}.{m} return type {idl_ty:?} is \
             not currently supported by [protosept(scriptable)]; drop the attribute or extend \
             the marshalling layer (non-optional foreign returns and string returns are not \
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
}
