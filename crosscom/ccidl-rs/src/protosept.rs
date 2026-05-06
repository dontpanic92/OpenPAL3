//! Protosept emitter for crosscom IDL.
//!
//! Generates a `.p7` source file from the same parsed IDL that `RustGen`
//! consumes. For each `interface I` we emit a flattened `@foreign proto I`
//! whose `@foreign` attribute carries the canonical dispatcher
//! (`com.invoke`), finalizer (`com.release`), `type_tag`
//! (`<rust-module-dotted>.<Interface>`), and the interface UUID. Methods
//! are emitted in IDL/vtable order: the index of a method inside the
//! emitted proto matches its vtable slot index relative to the first
//! non-IUnknown slot. The host dispatcher relies on this invariant.
//!
//! Classes are no longer emitted as script-visible artifacts: scripts
//! interact with COM objects through `box<I>` foreign cells whose
//! handle is interned by the host (see `crosscom-protosept` for
//! the dispatcher implementation).
//!
//! Methods that contain any IDL parameter or return type that cannot be
//! represented in protosept (e.g. raw byte pointers, untagged generics in
//! `[internal(), rust()]` methods) are skipped silently — they are simply
//! not callable from script. The `[internal(), rust()]` attribute already
//! removes such methods via `Interface::public_methods()`.
//!
//! Type mapping (IDL → Protosept):
//!
//! | IDL          | Protosept |
//! | ------------ | --------- |
//! | `void`       | `int`     |
//! | `bool`       | `int`     |
//! | `int` / `long` / `longlong` / `byte` | `int` |
//! | `float`      | `float`   |
//! | `UUID`       | `string`  |
//! | `&str` / `string` | `string` |
//! | `IFoo`       | `box<IFoo>` |
//! | `IFoo?`      | `?box<IFoo>` |
//! | `IFoo[]`     | `array<box<IFoo>>` |
//!
//! The generated file is intended to be read by the `p7` compiler. It does
//! not exercise any unstable language features; it stays within the subset
//! covered by `protosept/tests/` (foreign protos, nullable types, arrays).

use std::collections::HashMap;

use crate::{
    CrossComIdl, Error, Interface, Item, Method, Module, Symbol, parse_uuid_bytes,
    rust_module,
};

/// Top-level entry point: generate a protosept source file from a parsed IDL
/// unit (which must already have its imports processed).
pub(crate) fn generate(unit: CrossComIdl) -> Result<String, Error> {
    ProtoseptGen::new(unit)?.gen()
}

struct ProtoseptGen {
    unit: CrossComIdl,
    symbols: HashMap<String, Symbol>,
    /// IDL `module(rust)` path of the *current* IDL (e.g. `radiance::comdef`).
    current_module: Module,
}

impl ProtoseptGen {
    fn new(mut unit: CrossComIdl) -> Result<Self, Error> {
        let current_module = rust_module(&unit)?.clone();
        let mut symbols: HashMap<String, Symbol> = HashMap::new();

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
            current_module,
        })
    }

    fn gen(&self) -> Result<String, Error> {
        let mut out = String::new();
        out.push_str(&format!(
            "// Auto-generated from IDL by crosscom-ccidl. Do not edit.\n\
             // Module: {module}\n\n",
            module = self.current_module.module_name
        ));

        // Emit `import` directives for every imported IDL file. We assume
        // the consumer has placed the generated .p7 files alongside each
        // other under a common protosept package, with each IDL file named
        // `foo.idl` producing a module accessible as `foo.*`. Consumers that
        // use a different layout can post-process the import lines.
        for import in &self.unit.imports {
            // `crosscom.idl` only declares IUnknown/IObjectArray; IUnknown is
            // synthetic (not script-callable) and IObjectArray is rarely
            // needed. Importing it would create a useless dependency, so
            // skip it.
            if import.file_name == "crosscom.idl" {
                continue;
            }
            let module_stem = import
                .file_name
                .strip_suffix(".idl")
                .unwrap_or(&import.file_name);
            out.push_str(&format!("import {module_stem};\n"));
        }
        if !self.unit.imports.is_empty() {
            out.push('\n');
        }

        // Emit interfaces as `@foreign` protos. IDL classes are no longer
        // emitted as script-visible artifacts — scripts hold `box<I>`
        // directly and the host dispatcher backs every method call.
        for item in &self.unit.items {
            if let Item::Interface(interface) = item {
                if !interface.codegen_ignore() {
                    self.gen_interface(interface, &mut out)?;
                }
            }
        }

        Ok(out)
    }

    fn gen_interface(&self, interface: &Interface, out: &mut String) -> Result<(), Error> {
        // UUID constant.
        let uuid = interface.attrs.get("uuid").ok_or_else(|| {
            Error::Generate(format!(
                "interface {} is missing uuid attribute",
                interface.name
            ))
        })?;
        // Validate the UUID string up front; if it's malformed we fail loudly.
        parse_uuid_bytes(uuid)?;
        let uuid_lc = uuid.to_lowercase();
        out.push_str(&format!(
            "pub let {iface}_UUID: string = \"{uuid}\";\n\n",
            iface = interface.name,
            uuid = uuid_lc,
        ));

        // Flatten methods up the inheritance chain so the proto is
        // self-contained for the script side.
        //
        // Method emission order mirrors `collect_public_methods` (bases
        // first, then own methods, in IDL declaration order). The host
        // dispatcher relies on this matching the COM vtable order: the
        // index of a method in the emitted proto is its vtable slot
        // relative to the first non-IUnknown slot (i.e. the host adds 3
        // for IUnknown's prefix). To keep slot indices accurate, any
        // public method whose signature cannot be mapped to protosept
        // is a hard error here rather than a silent skip — silently
        // skipping would shift every later method's slot index. The
        // `[internal(), rust()]` filter (in `public_methods()`) handles
        // genuinely unrepresentable methods upstream; if anything still
        // slips past it, the IDL needs a fix.
        let methods = self.collect_public_methods(&interface.name)?;
        let mut emitted = Vec::new();
        for (origin, method) in &methods {
            match self.proto_method_signature(&interface.name, method)? {
                Some(sig) => emitted.push((origin.clone(), method.clone(), sig)),
                None => {
                    return Err(Error::Generate(format!(
                        "interface {iface}: public method {origin}.{name} contains a type \
                         not representable in protosept; mark it `[internal(), rust()]` or \
                         add the missing type mapping. Skipping would shift downstream \
                         vtable slot indices.",
                        iface = interface.name,
                        origin = origin,
                        name = method.name,
                    )));
                }
            }
        }

        // The dotted form of the IDL's rust module path is the canonical
        // type_tag prefix: `<rust_module_dotted>.<Interface>` uniquely
        // identifies a proto across all loaded p7 modules.
        let module_dot = self.current_module.module_name.replace("::", ".");
        let type_tag = format!("{module_dot}.{}", interface.name);

        out.push_str(
            "@foreign(dispatcher=\"com.invoke\", finalizer=\"com.release\",\n",
        );
        out.push_str(&format!(
            "         type_tag=\"{type_tag}\",\n         uuid=\"{uuid_lc}\")\n",
        ));
        out.push_str(&format!("pub proto {iface} {{\n", iface = interface.name));
        if emitted.is_empty() {
            out.push_str("    // (no script-callable methods)\n");
        }
        for (_, _, sig) in &emitted {
            out.push_str(&format!("    {sig};\n"));
        }
        out.push_str("}\n\n");

        Ok(())
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

    /// Collect all public (non-internal) methods of `iname`, walking bases.
    /// Walking stops at a "synthetic root" interface — one with no bases and
    /// `codegen(ignore)` set (i.e. `IUnknown`). Its methods
    /// (`query_interface`, `add_ref`, `release`) are not flattened because
    /// scripts access those via the dedicated `com.*` builtins, not via the
    /// raw vtable shape.
    ///
    /// Crucially, imported interfaces are *not* synthetic roots: imports
    /// receive `codegen(ignore)` from `process_imports` so that RustGen does
    /// not re-emit them, but their methods are real and must be flattened.
    /// This is what distinguishes them from `IUnknown` (which is also marked
    /// `codegen(ignore)` but has no bases).
    ///
    /// Each returned entry is `(origin_interface_name, method)`: the
    /// interface where the method was originally declared.
    fn collect_public_methods(&self, iname: &str) -> Result<Vec<(String, Method)>, Error> {
        let interface = self.find_interface(iname)?;
        let mut methods: Vec<(String, Method)> = Vec::new();
        match interface.bases.len() {
            0 => {}
            1 => {
                let base_name = &interface.bases[0];
                let base = self.find_interface(base_name)?;
                let synthetic_root = base.bases.is_empty() && base.codegen_ignore();
                if !synthetic_root {
                    methods.extend(self.collect_public_methods(base_name)?);
                }
            }
            _ => {
                return Err(Error::Generate(format!(
                    "cannot have more than one parent for interface: {}",
                    interface.name
                )));
            }
        }

        let module = interface.module.clone();
        for mut method in interface.public_methods() {
            method.interface_module = module.clone();
            methods.push((interface.name.clone(), method));
        }

        Ok(methods)
    }

    /// Build the `fn name(self: ref<I>, ...) -> T` signature for a proto
    /// method. Returns `Ok(None)` if any type cannot be mapped to protosept.
    fn proto_method_signature(
        &self,
        iface: &str,
        method: &Method,
    ) -> Result<Option<String>, Error> {
        let mut params = format!("self: ref<{iface}>");
        for p in &method.params {
            let Some(ty) = self.protosept_type(&p.ty)? else {
                return Ok(None);
            };
            params.push_str(", ");
            params.push_str(&p.name);
            params.push_str(": ");
            params.push_str(&ty);
        }

        let Some(ret) = self.protosept_type(&method.ret_ty)? else {
            return Ok(None);
        };

        Ok(Some(format!(
            "fn {name}({params}) -> {ret}",
            name = method.name,
        )))
    }

    /// Map an IDL type string to its protosept equivalent, or `Ok(None)` if
    /// the type cannot be represented in script. Interface types lower to
    /// `box<I>` carriers backed by a host-managed `ComRc<I>` (see
    /// `crosscom-protosept` for the dispatcher).
    fn protosept_type(&self, idl_ty: &str) -> Result<Option<String>, Error> {
        let trimmed = idl_ty.trim();

        if let Some(inner) = trimmed.strip_suffix("[]") {
            let inner = inner.trim();
            if matches!(self.symbols.get(inner), Some(Symbol::Interface(_))) {
                return Ok(Some(format!("array<box<{inner}>>")));
            }
            // Primitive-element arrays could be added later.
            return Ok(None);
        }

        if let Some(inner) = trimmed.strip_suffix('?') {
            let inner = inner.trim();
            if matches!(self.symbols.get(inner), Some(Symbol::Interface(_))) {
                return Ok(Some(format!("?box<{inner}>")));
            }
            // Nullable primitives are not currently used in any IDL; map them
            // anyway for completeness.
            if let Some(prim) = primitive_protosept(inner) {
                return Ok(Some(format!("?{prim}")));
            }
            return Ok(None);
        }

        if let Some(prim) = primitive_protosept(trimmed) {
            return Ok(Some(prim.to_string()));
        }

        if matches!(self.symbols.get(trimmed), Some(Symbol::Interface(_))) {
            return Ok(Some(format!("box<{trimmed}>")));
        }

        // Unknown — caller treats as "skip method".
        Ok(None)
    }
}

fn primitive_protosept(idl_ty: &str) -> Option<&'static str> {
    match idl_ty {
        "void" => Some("int"),
        "bool" => Some("int"),
        "int" | "long" | "longlong" | "byte" => Some("int"),
        "float" => Some("float"),
        "UUID" => Some("string"),
        "string" | "&str" => Some("string"),
        _ => None,
    }
}
