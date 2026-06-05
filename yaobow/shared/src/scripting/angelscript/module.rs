use std::{
    io::{Cursor, Read},
    sync::Arc,
};

use anyhow::Context;
use byteorder::ReadBytesExt;
use common::read_ext::ReadExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScriptTypeDefinition {
    name: String,
}

impl ScriptTypeDefinition {
    fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let name = read_string(cursor).with_context(|| {
            format!(
                "reading ScriptTypeDefinition.name at {:#x}",
                cursor.position()
            )
        })?;

        Ok(Self { name })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScriptTypeReference {
    name: String,
}

impl ScriptTypeReference {
    fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let name = read_string(cursor).with_context(|| {
            format!(
                "reading ScriptTypeReference.name at {:#x}",
                cursor.position()
            )
        })?;

        Ok(Self { name })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScriptDataType {
    flag: u8,
    unknown: u32,
    type_ref: ScriptTypeReference,
    unknown2: u8,
    unknown3: u8,
    unknown4: u8,
    unknown5: u8,
}

impl ScriptDataType {
    fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let start = cursor.position();
        let flag = cursor.read_u8()?;
        if flag != 0 {
            anyhow::bail!("AClass8.flag = {} (expected 0) at {:#x}", flag, start);
        }

        let unknown = cursor.read_u32_le()?;
        let type_ref = ScriptTypeReference::read(cursor)
            .with_context(|| format!("ScriptDataType.type_ref starting at {:#x}", start))?;
        let unknown2 = cursor.read_u8()?;
        let unknown3 = cursor.read_u8()?;
        let unknown4 = cursor.read_u8()?;
        let unknown5 = cursor.read_u8()?;

        Ok(Self {
            flag,
            unknown,
            type_ref,
            unknown2,
            unknown3,
            unknown4,
            unknown5,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Instruction {
    pub inst: u32,
    pub params: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScriptFunction {
    pub name: String,
    pub ret_type: ScriptDataType,
    pub param_types: Vec<ScriptDataType>,
    pub unknown_dword1: u32,
    pub inst: Vec<u8>,
    pub inst2: Vec<Instruction>,
    pub type_refs: Vec<ScriptTypeReference>,
    pub dword_with_type_ref: Vec<u32>,
    pub unknown_dword: u32,
    pub type_ref: ScriptTypeReference,
    pub dword_vec: Vec<u32>,
}

impl ScriptFunction {
    fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let fn_start = cursor.position();
        let name = read_string(cursor)
            .with_context(|| format!("ScriptFunction.name at {:#x}", fn_start))?;

        let ret_type = ScriptDataType::read(cursor)
            .with_context(|| format!("fn '{}' ret_type at {:#x}", name, cursor.position()))?;
        let param_count = cursor.read_u32_le()? as usize;
        let mut param_types = vec![];

        for p in 0..param_count {
            param_types.push(ScriptDataType::read(cursor).with_context(|| {
                format!(
                    "fn '{}' param_types[{}] at {:#x}",
                    name,
                    p,
                    cursor.position()
                )
            })?);
        }

        let unknown_dword1 = cursor.read_u32_le()?;
        let count2 = cursor.read_u32_le()?;
        let (inst, inst2) =
            Self::read_instructions(cursor, count2 as usize, &name).with_context(|| {
                format!(
                    "fn '{}' instructions ({} bytes declared, starts at {:#x})",
                    name,
                    count2,
                    cursor.position()
                )
            })?;
        let type_ref_count = cursor.read_u32_le()? as usize;

        let mut type_refs = vec![];
        let mut dword_with_type_ref = vec![];
        for t in 0..type_ref_count {
            type_refs.push(
                ScriptTypeReference::read(cursor)
                    .with_context(|| format!("fn '{}' type_refs[{}]", name, t))?,
            );
            dword_with_type_ref.push(cursor.read_u32_le()?);
        }

        let unknown_dword = cursor.read_u32_le()?;
        let type_ref = ScriptTypeReference::read(cursor)
            .with_context(|| format!("fn '{}' trailing type_ref", name))?;
        let dword_count = cursor.read_u32_le()? as usize;
        let mut dword_vec = vec![];
        for _ in 0..dword_count {
            dword_vec.push(cursor.read_u32_le()?);
        }

        Ok(Self {
            name,
            ret_type,
            param_types,
            unknown_dword1,
            inst,
            inst2,
            type_refs,
            dword_with_type_ref,
            unknown_dword,
            type_ref,
            dword_vec,
        })
    }

    /// Minimal `ScriptFunction` constructor for VM unit tests.
    ///
    /// Real functions are deserialised from PAL4 `.csb` modules; this
    /// helper synthesises one with just enough scaffolding to feed a
    /// `ScriptVm` for opcode-level testing. The instruction stream
    /// (`inst`) is the only behaviourally-relevant field.
    #[cfg(test)]
    pub(crate) fn test_function(name: &str, inst: Vec<u8>) -> Self {
        let stub_type = ScriptDataType {
            flag: 0,
            unknown: 0,
            type_ref: ScriptTypeReference {
                name: String::new(),
            },
            unknown2: 0,
            unknown3: 0,
            unknown4: 0,
            unknown5: 0,
        };
        Self {
            name: name.to_string(),
            ret_type: stub_type.clone(),
            param_types: Vec::new(),
            unknown_dword1: 0,
            inst,
            inst2: Vec::new(),
            type_refs: Vec::new(),
            dword_with_type_ref: Vec::new(),
            unknown_dword: 0,
            type_ref: ScriptTypeReference {
                name: String::new(),
            },
            dword_vec: Vec::new(),
        }
    }

    fn read_instructions(
        cursor: &mut Cursor<&[u8]>,
        total_size: usize,
        fn_name: &str,
    ) -> anyhow::Result<(Vec<u8>, Vec<Instruction>)> {
        let mut i = 0;
        let mut instructions = vec![0; total_size];
        let mut instructions2 = vec![];

        while i < total_size {
            let inst_offset = cursor.position();
            let inst = cursor.read_u8().with_context(|| {
                format!(
                    "fn '{}' opcode byte at module-offset {:#x} (inst-byte {}/{})",
                    fn_name, inst_offset, i, total_size
                )
            })?;
            instructions[i] = inst;
            let inst_len = INST_LENGTH[inst as usize];
            // Catch unknown opcodes structurally — INST_LENGTH = 0
            // means we have no length entry for this opcode. Without
            // this guard, `extra_len = 0 - 4` would underflow `usize`
            // and walk the cursor past EOF, producing the famously
            // useless "failed to fill whole buffer" io error.
            if inst_len < 4 {
                anyhow::bail!(
                    "unknown opcode {:#04x} ({0}) at module-offset {:#x} \
                     (fn '{}', inst-byte {}/{}); INST_LENGTH says {}, \
                     refusing to underflow",
                    inst,
                    inst_offset,
                    fn_name,
                    i,
                    total_size,
                    inst_len,
                );
            }
            i += 4;

            let extra_len = inst_len - 4;
            if i + extra_len > total_size {
                anyhow::bail!(
                    "fn '{}' opcode {:#04x} at module-offset {:#x} would read {} extra bytes \
                     but only {} remain in the declared instruction stream (inst-byte {}/{})",
                    fn_name,
                    inst,
                    inst_offset,
                    extra_len,
                    total_size.saturating_sub(i),
                    i,
                    total_size,
                );
            }
            cursor.read_exact(&mut instructions[i..i + extra_len])
                .with_context(|| format!(
                    "fn '{}' opcode {:#04x} extra_len={} at module-offset {:#x} (inst-byte {}/{})",
                    fn_name, inst, extra_len, inst_offset, i, total_size,
                ))?;

            let mut p = Vec::new();
            p.extend_from_slice(&instructions[i..i + extra_len]);
            instructions2.push(Instruction {
                inst: inst as u32,
                params: p,
            });

            i += extra_len;
        }

        Ok((instructions, instructions2))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScriptModule {
    pub type_defs: Vec<ScriptTypeDefinition>,
    pub type_refs: Vec<ScriptTypeReference>,
    pub named_global_count: usize,
    /// Module-local globals array sized by an outer count. The
    /// original parser sized this by what was actually
    /// `named_globals[0].name.strlen` — a misreading exposed by
    /// the RE pass on M02/Q04/etc. Always empty in current
    /// modules; any positive-index `Pga`/`Movga4` against it will
    /// panic with an out-of-bounds error, which is the desired RE
    /// signal that we've hit unfamiliar bytecode.
    pub globals: Vec<u32>,
    pub module_loading: ScriptFunction,
    pub module_unloading: ScriptFunction,

    pub functions: Vec<Arc<ScriptFunction>>,
    pub strings: Vec<String>,
    pub astruct_vec2: Vec<ScriptFunction>,

    /// Per-class global variable declarations exported by this
    /// module. Empty on the 29 PAL4 scene modules that don't
    /// declare any; populated on the 7 that do (M02, M07, M09,
    /// M16, Q04, Q05, Q11). See `NamedGlobal` for layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_globals: Vec<NamedGlobal>,
}

/// One entry in a PAL4 module's named-globals block. The block is a
/// classic length-prefixed array sandwich:
///
/// ```text
/// u32  named_globals_count
/// [ NamedGlobal ; named_globals_count ]
/// u32  named_globals_count   (duplicated for redundancy)
/// ```
///
/// Each `NamedGlobal` serializes as:
///
/// ```text
/// u32  strlen
/// bytes[strlen]  name
/// u8   null      (= 0)
/// u32  kind
/// [8 bytes pad of 0]
/// u32  index     (= position in the array)
/// ```
///
/// PAL4 uses these to declare per-character data shared across
/// cutscenes — Q04 (the only multi-entry sample) declares an
/// `LL_002` class with `LL_shu`, `LL_ming`, `LL_yan` members,
/// matching the in-game character roster with an `LL_` prefix.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NamedGlobal {
    /// Variable / class name (e.g. `"LL_002"`, `"LL_yan"`).
    pub name: String,
    /// AngelScript type tag. Observed values are `0x3E` ('>') and
    /// `0x3C` ('<'); the exact AS-type-id semantics haven't been
    /// fully decoded yet, so the field is surfaced verbatim.
    pub kind: u32,
    /// 0-based ordinal within the module — encoded redundantly
    /// inside each entry, validated against the array position by
    /// the parser.
    pub index: u32,
}

impl ScriptModule {
    pub fn read_from_buffer(buffer: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(buffer);
        Self::read(&mut cursor)
    }

    fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        cursor.set_position(4);

        let type_def_count = cursor.read_u32_le()?;
        let mut type_defs = vec![];
        for t in 0..type_def_count {
            type_defs.push(
                ScriptTypeDefinition::read(cursor).with_context(|| format!("type_defs[{}]", t))?,
            );
        }

        let type_ref_count = cursor.read_u32_le()?;
        let mut type_refs = vec![];
        for t in 0..type_ref_count {
            type_refs.push(
                ScriptTypeReference::read(cursor).with_context(|| format!("type_refs[{}]", t))?,
            );
        }

        // Named-globals block:
        //
        //     u32  named_globals_count
        //     for i in 0..count:
        //         u32  strlen
        //         bytes[strlen]  name
        //         u8   null            (= 0)
        //         u32  kind
        //         [8 bytes pad of 0]
        //         u32  index           (= i)
        //     u32  named_globals_count (duplicated, redundancy check)
        //
        // Every entry has the same shape — including entry 0,
        // whose trailing `index = 0` and 8 zero pad bytes used to
        // look like a single 12-byte zero region. The trailing
        // count_dup is what the original parser misread as a
        // separate `global_count` field: in modules with no
        // named globals both words are 0 (count = count_dup = 0),
        // in the 7 modules that have them count_dup carries the
        // non-zero count again. There is no standalone
        // `global_count` field.
        let named_globals_count = cursor.read_u32_le()? as usize;

        let mut named_globals: Vec<NamedGlobal> = Vec::with_capacity(named_globals_count);
        for i in 0..named_globals_count {
            let strlen = cursor
                .read_u32_le()
                .with_context(|| format!("named_globals[{}].strlen", i))?
                as usize;
            let name = read_raw_string(cursor, strlen)
                .with_context(|| format!("named_globals[{}].name", i))?;
            let null = cursor
                .read_u8()
                .with_context(|| format!("named_globals[{}] null terminator", i))?;
            if null != 0 {
                anyhow::bail!(
                    "named_globals[{}]: expected null terminator after name, got {:#04x}",
                    i,
                    null
                );
            }
            let kind = cursor
                .read_u32_le()
                .with_context(|| format!("named_globals[{}].kind", i))?;
            let mut pad = [0u8; 8];
            cursor
                .read_exact(&mut pad)
                .with_context(|| format!("named_globals[{}].pad8", i))?;
            if pad != [0u8; 8] {
                anyhow::bail!("named_globals[{}]: expected 8 zero bytes, got {:?}", i, pad);
            }
            let index = cursor
                .read_u32_le()
                .with_context(|| format!("named_globals[{}].index", i))?;
            if index as usize != i {
                anyhow::bail!(
                    "named_globals[{}]: expected index = {}, got {}",
                    i,
                    i,
                    index
                );
            }
            named_globals.push(NamedGlobal { name, kind, index });
        }

        // Duplicate count terminator. Always present in the file
        // (including the empty-block case where it's the only word).
        let count_dup = cursor.read_u32_le().context("named_globals count_dup")?;
        if count_dup as usize != named_globals_count {
            anyhow::bail!(
                "named_globals: expected count_dup = {}, got {}",
                named_globals_count,
                count_dup
            );
        }

        // `module.globals` is kept as an empty Vec for backwards
        // compatibility with any `Pga`/`Movga4` paths that might
        // index it positively — there is no separate module-local
        // globals array sized by an outer count, so any positive-
        // index access into this Vec will panic with a clear
        // out-of-bounds error (the desired RE signal).
        let globals: Vec<u32> = Vec::new();

        let module_loading = ScriptFunction::read(cursor).context("module_loading")?;
        let module_unloading = ScriptFunction::read(cursor).context("module_unloading")?;

        let astruct_count1 = cursor.read_u32_le()? as usize;
        let mut functions = vec![];
        for f in 0..astruct_count1 {
            functions.push(Arc::new(ScriptFunction::read(cursor).with_context(
                || format!("functions[{}] (of {})", f, astruct_count1),
            )?));
        }

        let string_count = cursor.read_u32_le()? as usize;
        let mut strings = vec![];
        for s in 0..string_count {
            strings.push(read_string(cursor).with_context(|| format!("strings[{}]", s))?);
        }

        let astruct_count2 = cursor.read_u32_le()? as usize;
        let mut astruct_vec2 = vec![];
        for f in 0..astruct_count2 {
            astruct_vec2.push(
                ScriptFunction::read(cursor)
                    .with_context(|| format!("astruct_vec2[{}] (of {})", f, astruct_count2))?,
            );
        }

        Ok(Self {
            type_defs,
            type_refs,
            named_global_count: named_globals_count,
            globals,
            module_loading,
            module_unloading,
            functions,
            strings,
            astruct_vec2,
            named_globals,
        })
    }

    /// Minimal `ScriptModule` constructor for VM unit tests.
    ///
    /// Wraps a list of pre-built [`ScriptFunction`]s (typically made
    /// via [`ScriptFunction::test_function`]) into an
    /// otherwise-empty module suitable for `ScriptVm::new`.
    #[cfg(test)]
    pub(crate) fn test_module(functions: Vec<ScriptFunction>) -> Self {
        let stub_fn = ScriptFunction::test_function("", Vec::new());
        Self {
            type_defs: Vec::new(),
            type_refs: Vec::new(),
            named_global_count: 0,
            globals: Vec::new(),
            module_loading: stub_fn.clone(),
            module_unloading: stub_fn,
            functions: functions.into_iter().map(std::sync::Arc::new).collect(),
            strings: Vec::new(),
            astruct_vec2: Vec::new(),
            named_globals: Vec::new(),
        }
    }
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<String> {
    let pos = cursor.position();
    let len = cursor.read_u32_le()?;
    cursor
        .read_gbk_string(len as usize)
        .with_context(|| format!("read_string len={} at {:#x}", len, pos))
}

/// Read exactly `len` bytes and decode them as GBK. Used for named-
/// globals entries where the strlen is consumed separately from the
/// name bytes (split across an outer field for entry 0).
fn read_raw_string(cursor: &mut Cursor<&[u8]>, len: usize) -> anyhow::Result<String> {
    let pos = cursor.position();
    cursor
        .read_gbk_string(len)
        .with_context(|| format!("read_raw_string len={} at {:#x}", len, pos))
}

const INST_LENGTH: [usize; 256] = [
    0x06, 0x06, 0x08, 0x04, 0x06, 0x04, 0x04, 0x06, 0x06, 0x04, 0x04, 0x04, 0x08, 0x06, 0x08, 0x08,
    0x08, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x06, 0x08, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x06, 0x08, 0x08, 0x08, 0x08, 0x08,
    0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x04, 0x0C, 0x08, 0x06,
    0x06, 0x06, 0x08, 0x04, 0x04, 0x04, 0x06, 0x06, 0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end parse of every PAL4 `.csb` reachable through the
    /// vfs at `PAL4_ROOT`. Skipped when the env var is unset so the
    /// test suite stays runnable on hosts without the game install
    /// (and so we don't have to commit copyrighted bytecode to a
    /// GPL repo).
    ///
    /// Specifically asserts:
    /// 1. All `.csb` files parse via `read_from_buffer` — including
    ///    the 7 named-globals modules (M02, M07, M09, M16, Q04, Q05,
    ///    Q11) the original parser couldn't handle.
    /// 2. Those 7 modules surface a non-empty `named_globals` Vec
    ///    with the recovered structure (so the proper decoder path
    ///    is exercised end-to-end).
    /// 3. Q04 in particular decodes to exactly 4 entries
    ///    `[LL_002, LL_shu, LL_ming, LL_yan]` — the canonical
    ///    multi-entry RE sample.
    #[test]
    #[ignore = "requires PAL4_ROOT env var pointing at a PAL4 install"]
    fn parses_every_pal4_csb_module() {
        let root = match std::env::var("PAL4_ROOT") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("PAL4_ROOT not set; skipping parses_every_pal4_csb_module");
                return;
            }
        };
        let script_dir = std::path::PathBuf::from(&root)
            .join("gamedata")
            .join("script");
        let entries = match std::fs::read_dir(&script_dir) {
            Ok(it) => it,
            Err(e) => panic!(
                "PAL4_ROOT/gamedata/script not readable: {} ({:#})",
                script_dir.display(),
                e
            ),
        };

        let mut total = 0usize;
        let mut named_global_modules = 0usize;
        let mut saw_q04 = false;
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                != Some("csb".to_string())
            {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            let bytes = std::fs::read(&path).unwrap();
            let module = match ScriptModule::read_from_buffer(&bytes) {
                Ok(m) => m,
                Err(e) => panic!("{}: parse failed:\n{:#}", stem, e),
            };
            total += 1;
            if module.named_global_count > 0 {
                assert_eq!(
                    module.named_globals.len(),
                    module.named_global_count,
                    "{}: named_globals.len ({}) != named_global_count ({})",
                    stem,
                    module.named_globals.len(),
                    module.named_global_count,
                );
                // Indices must be 0..N sequential.
                for (i, g) in module.named_globals.iter().enumerate() {
                    assert_eq!(
                        g.index as usize, i,
                        "{}: named_globals[{}].index = {} (expected {})",
                        stem, i, g.index, i
                    );
                }
                // Observed `kind` values across the corpus are
                // 0x3C and 0x3E. We don't yet know the exact
                // semantics, but anything outside that pair would
                // be a new RE finding worth surfacing.
                for g in &module.named_globals {
                    assert!(
                        g.kind == 0x3C || g.kind == 0x3E,
                        "{}: named_globals[{}].kind = {:#x} (expected 0x3C or 0x3E)",
                        stem,
                        g.index,
                        g.kind
                    );
                }
                named_global_modules += 1;
                if stem.eq_ignore_ascii_case("Q04") {
                    saw_q04 = true;
                    let names: Vec<&str> = module
                        .named_globals
                        .iter()
                        .map(|g| g.name.as_str())
                        .collect();
                    assert_eq!(
                        names,
                        vec!["LL_002", "LL_shu", "LL_ming", "LL_yan"],
                        "Q04 named_globals mismatch"
                    );
                }
            } else {
                assert!(
                    module.named_globals.is_empty(),
                    "{}: named_global_count = 0 but named_globals has {} entries",
                    stem,
                    module.named_globals.len()
                );
            }
        }
        assert!(
            total > 0,
            "no .csb modules found under {}",
            script_dir.display()
        );
        assert!(
            named_global_modules >= 7,
            "expected at least 7 modules with named_global_count > 0 (M02, M07, M09, M16, Q04, Q05, Q11); found {}",
            named_global_modules,
        );
        assert!(
            saw_q04,
            "Q04.csb was not found under {}",
            script_dir.display()
        );
    }
}
