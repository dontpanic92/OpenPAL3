//! Headless diagnostic for PAL4 AngelScript `.csb` modules.
//!
//! Walks every `.csb` reachable through the same vfs the live engine
//! uses (`packfs::init_virtual_fs`) and tries to parse each via
//! `ScriptModule::read_from_buffer`. On failure, prints the structured
//! error chain (fn name + module-offset + opcode, courtesy of the
//! instrumentation added in `yaobow/shared/src/scripting/angelscript/
//! module.rs`) plus 32 bytes preceding and following the failure
//! position. On success, prints a per-opcode histogram so opcode-set
//! diffs between failing and passing modules are easy to eyeball.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use common::store_ext::StoreExt2;
use packfs::init_virtual_fs;
use shared::scripting::angelscript::{disasm, ScriptModule};

#[derive(Parser)]
#[command(about = "Diagnose PAL4 .csb parse failures")]
struct Cli {
    /// PAL4 install root (directory containing `gamedata/`). Works
    /// with both extracted layouts and CPK-only Steam installs.
    #[arg(long)]
    root: PathBuf,

    /// Inspect only this module stem (e.g. `M02` → /gamedata/script/M02.csb).
    /// Repeatable; if absent and `--all-failing` is not set, every
    /// `.csb` under /gamedata/script/ is inspected.
    #[arg(long)]
    file: Vec<String>,

    /// Shortcut: inspect only the known-failing modules from the
    /// pal4_plot_dump WARN list (M02, M07, M09, M16, Q04, Q05, Q11).
    #[arg(long)]
    all_failing: bool,

    /// On parse success, also print a per-opcode histogram (sum
    /// across module_loading, module_unloading, every function, every
    /// astruct_vec2 entry). Off by default since it's verbose.
    #[arg(long)]
    histogram: bool,

    /// Window radius for the byte-context dump on failure.
    #[arg(long, default_value_t = 32)]
    context: usize,

    /// When set, disassemble the named function in each --file module
    /// and print its instructions (addr + opcode + operands). Useful
    /// for understanding gating conditions that `pal4_plot_dump`'s
    /// abstract walker doesn't capture (jns/js/jp/jnp branches, fns
    /// called inside conditional blocks the cmp_literal extractor
    /// skips).
    #[arg(long)]
    disasm: Option<String>,

    /// List functions in each --file module by their module-index
    /// (the integer `Call { function: N }` refers to). Useful when
    /// reading a disassembly's `Call`/`CallBnd` to figure out which
    /// script function the VM hands control to.
    #[arg(long)]
    list_functions: bool,

    /// List the module's string-literal table by index. The literal
    /// at index N is what `Str { index: N }` pushes onto the operand
    /// stack — combine with `--disasm` to recover concrete strings
    /// (scene names, dialog text, etc.) without re-disassembling.
    #[arg(long)]
    list_strings: bool,

    /// Probe whether the given VFS path resolves. Repeatable. Useful
    /// for verifying that the on-disk asset tree carries every
    /// (scene, block) the .csb scripts reference — e.g. `M02/3`'s
    /// `/gamedata/PALWorld/M02/3/3.bsp` was missing from the shipped
    /// cpk in some PAL4 distributions, causing `giArenaLoad` to
    /// abort the script when reached from `LinkObj01`.
    #[arg(long)]
    probe: Vec<String>,
}

const KNOWN_FAILING: &[&str] = &["M02", "M07", "M09", "M16", "Q04", "Q05", "Q11"];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli
        .root
        .canonicalize()
        .with_context(|| format!("resolve --root {}", cli.root.display()))?;
    eprintln!("Mounting PAL4 vfs from {}", root.display());
    let vfs = init_virtual_fs(&root, None);

    if !cli.probe.is_empty() {
        for path in &cli.probe {
            // Best-effort directory listing via mini-fs's
            // `entries_path`. Falls through to a single-file read
            // on listing failure (typical for individual files).
            let trimmed = path.trim_end_matches('/');
            let p = Path::new(trimmed);
            match <mini_fs::MiniFs as mini_fs::Store>::entries_path(&vfs, p) {
                Ok(entries) => {
                    let mut names: Vec<String> = Vec::new();
                    let mut had_err = false;
                    for entry in entries {
                        match entry {
                            Ok(e) => {
                                let basename = std::path::Path::new(&e.name)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| e.name.to_string_lossy().into_owned());
                                let kind = match e.kind {
                                    mini_fs::EntryKind::File => "F",
                                    mini_fs::EntryKind::Dir => "D",
                                };
                                names.push(format!("{} {}", kind, basename));
                            }
                            Err(_) => {
                                had_err = true;
                            }
                        }
                    }
                    if names.is_empty() && had_err {
                        // entries() iterator surfaced no usable rows
                        // — fall through to the single-file read so
                        // we still report something useful.
                    } else {
                        names.sort();
                        names.dedup();
                        eprintln!("PROBE DIR {} ({} entries):", path, names.len());
                        for n in &names {
                            eprintln!("  {}", n);
                        }
                        continue;
                    }
                }
                Err(_) => {}
            }
            match vfs.read_to_end(p) {
                Ok(b) => eprintln!("PROBE OK  {} ({} bytes)", path, b.len()),
                Err(e) => eprintln!("PROBE MISS {}: {}", path, e),
            }
        }
        return Ok(());
    }

    let mut stems: Vec<String> = if !cli.file.is_empty() {
        cli.file.clone()
    } else if cli.all_failing {
        KNOWN_FAILING.iter().map(|s| s.to_string()).collect()
    } else {
        let mut v = Vec::new();
        let script_dir = Path::new("/gamedata/script");
        let entries =
            <mini_fs::MiniFs as mini_fs::Store>::entries_path(&vfs, script_dir)
                .with_context(|| format!("list {}", script_dir.display()))?;
        for entry in entries {
            let entry = entry?;
            if !matches!(entry.kind, mini_fs::EntryKind::File) {
                continue;
            }
            // CpkFs returns basenames, LocalFs returns relative paths
            // — normalise to basename either way (mirrors the
            // pal4_plot_dump fix).
            let basename = std::path::Path::new(&entry.name)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| entry.name.to_string_lossy().into_owned());
            if !basename.to_ascii_lowercase().ends_with(".csb") {
                continue;
            }
            v.push(basename[..basename.len() - 4].to_string());
        }
        v.sort();
        v.dedup();
        v
    };
    stems.sort();
    stems.dedup();

    let mut failures = 0usize;
    for stem in &stems {
        let vfs_path = format!("/gamedata/script/{}.csb", stem);
        eprintln!("\n=== {} ===", vfs_path);
        let bytes = match vfs.read_to_end(&vfs_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("  read failed: {:#}", e);
                failures += 1;
                continue;
            }
        };
        eprintln!("  size: {} bytes", bytes.len());

        match ScriptModule::read_from_buffer(&bytes) {
            Ok(module) => {
                eprintln!(
                    "  OK: {} fns, {} strings, {} globals, {} astruct_vec2, {} named_globals",
                    module.functions.len(),
                    module.strings.len(),
                    module.globals.len(),
                    module.astruct_vec2.len(),
                    module.named_globals.len(),
                );
                for g in &module.named_globals {
                    eprintln!(
                        "    named_global[{}]: name={:?} kind={:#04x}",
                        g.index, g.name, g.kind
                    );
                }
                if cli.histogram {
                    print_histogram(&module);
                }
                if cli.list_functions {
                    eprintln!("  function table ({} entries):", module.functions.len());
                    for (i, f) in module.functions.iter().enumerate() {
                        eprintln!("    [{}] {}", i, f.name);
                    }
                }
                if cli.list_strings {
                    eprintln!("  string table ({} entries):", module.strings.len());
                    for (i, s) in module.strings.iter().enumerate() {
                        eprintln!("    [{}] {:?}", i, s);
                    }
                }
                if let Some(target_fn) = &cli.disasm {
                    print_disasm(&module, target_fn);
                }
            }
            Err(e) => {
                failures += 1;
                eprintln!("  FAIL:");
                for (i, cause) in e.chain().enumerate() {
                    eprintln!("    [{}] {}", i, cause);
                }
                if let Some(off) = parse_offset(&e) {
                    dump_context(&bytes, off, cli.context);
                }
            }
        }
    }

    eprintln!("\n--- summary: {}/{} modules failed ---", failures, stems.len());
    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn print_histogram(module: &ScriptModule) {
    let mut counts = [0u64; 256];
    for f in std::iter::once(&module.module_loading)
        .chain(std::iter::once(&module.module_unloading))
        .chain(module.functions.iter().map(|f| f.as_ref()))
        .chain(module.astruct_vec2.iter())
    {
        for inst in &f.inst2 {
            counts[(inst.inst & 0xff) as usize] += 1;
        }
    }
    eprintln!("  per-opcode histogram (nonzero only):");
    for (op, &n) in counts.iter().enumerate() {
        if n > 0 {
            eprintln!("    0x{:02x} ({:3}) -> {}", op, op, n);
        }
    }
}

fn print_disasm(module: &ScriptModule, target_fn: &str) {
    let Some(func) = module.functions.iter().find(|f| f.name == target_fn) else {
        eprintln!("  disasm: function {} not found in module", target_fn);
        eprintln!(
            "  available: {}",
            module
                .functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return;
    };
    let insts = disasm(func);
    eprintln!("  disasm of {} ({} bytes):", target_fn, func.inst.len());
    for inst in insts {
        eprintln!("    {:04x}  {:?}", inst.addr, inst.inst);
    }
}

/// Best-effort scan of the structured error chain for the first
/// `module-offset 0x…` mention added by the parser instrumentation.
fn parse_offset(err: &anyhow::Error) -> Option<u64> {
    for cause in err.chain() {
        let s = format!("{}", cause);
        if let Some(idx) = s.find("module-offset 0x") {
            let hex = s[idx + "module-offset 0x".len()..]
                .split(|c: char| !c.is_ascii_hexdigit())
                .next()
                .unwrap_or("");
            if let Ok(off) = u64::from_str_radix(hex, 16) {
                return Some(off);
            }
        }
    }
    None
}

fn dump_context(bytes: &[u8], off: u64, radius: usize) {
    let off = off as usize;
    let start = off.saturating_sub(radius);
    let end = (off + radius).min(bytes.len());
    eprintln!(
        "  context bytes around offset {:#x} (file size {:#x}):",
        off,
        bytes.len()
    );
    let mut line_start = start & !0xf;
    while line_start < end {
        let mut hex = String::new();
        let mut ascii = String::new();
        for i in line_start..(line_start + 16) {
            if i >= start && i < end {
                let b = bytes[i];
                let marker = if i == off { '>' } else { ' ' };
                hex.push_str(&format!("{}{:02x}", marker, b));
                ascii.push(if (0x20..0x7f).contains(&b) { b as char } else { '.' });
            } else {
                hex.push_str("   ");
                ascii.push(' ');
            }
        }
        eprintln!("    {:08x}  {}  |{}|", line_start, hex, ascii);
        line_start += 16;
    }
}
