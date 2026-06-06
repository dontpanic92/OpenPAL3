//! Build-script-facing API: walks `scripts/` directories and writes a
//! `.ypk` whose wire format is identical to
//! `radiance::asset::ypk::YpkWriter`.
//!
//! Gated behind the `build` feature.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

mod ypk_writer;

use ypk_writer::PackerYpkWriter;

/// One extra file to inject into the ypk, typically a build-script-generated
/// `.p7` sitting in `OUT_DIR`.
pub struct ExtraFile<'a> {
    /// Absolute path on disk.
    pub source_path: &'a Path,
    /// Path inside the ypk (e.g. `yaobow_services.p7`). Must be unique
    /// across all walked + extra files.
    pub virtual_entry: &'a str,
}

/// Input to [`pack`].
pub struct PackInput<'a> {
    /// Directory to walk recursively for `*.p7` files.
    pub scripts_dir: &'a Path,
    /// Extra files (typically from `OUT_DIR`) to inject. Order is
    /// preserved after walked files.
    pub extra_files: &'a [ExtraFile<'a>],
}

/// Pack `input` into a `.ypk` at `output_ypk`.
///
/// Walks `input.scripts_dir` recursively, collects every `*.p7` file
/// in deterministic lexicographic order, then writes them all to the
/// ypk via the vendored [`PackerYpkWriter`]. Each walked file maps to
/// its `scripts_dir`-relative path (forward-slash normalised) inside
/// the ypk; `extra_files` are appended at their declared
/// `virtual_entry`. Module resolution is purely path-based at read
/// time — see `radiance_scripting::script_vfs::ScriptVfsProvider`.
///
/// Emits `cargo:rerun-if-changed=` for every walked file and the
/// `scripts_dir` itself. `extra_files` are intentionally NOT tracked
/// here: their generators (typically `crosscom-ccidl`) already emit
/// `rerun-if-changed=` for the underlying IDL/JSON inputs, and the
/// generated `.p7` files are rewritten on every build script run —
/// tracking them here would cause a feedback loop.
pub fn pack(input: &PackInput, output_ypk: &Path) -> anyhow::Result<()> {
    let entries = collect_entries(input)?;

    println!("cargo:rerun-if-changed={}", input.scripts_dir.display());
    for entry in &entries {
        if entry.source_path.starts_with(input.scripts_dir) {
            println!("cargo:rerun-if-changed={}", entry.source_path.display());
        }
    }

    if let Some(parent) = output_ypk.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let file = std::fs::File::create(output_ypk)?;
    let mut writer = PackerYpkWriter::new(Box::new(file))?;

    for entry in entries {
        let data = std::fs::read(&entry.source_path)?;
        writer.write_file(&entry.virtual_entry, &data)?;
    }

    writer.finish()?;
    Ok(())
}

/// One classified entry collected from `scripts_dir` or `extra_files`.
#[derive(Clone, Debug)]
struct CollectedEntry {
    /// Absolute path on disk to read bytes from.
    source_path: PathBuf,
    /// Path inside the ypk.
    virtual_entry: String,
}

fn collect_entries(input: &PackInput) -> anyhow::Result<Vec<CollectedEntry>> {
    if !input.scripts_dir.is_dir() {
        anyhow::bail!(
            "PackInput::scripts_dir does not exist or is not a directory: {}",
            input.scripts_dir.display()
        );
    }

    let mut walked: Vec<(PathBuf, PathBuf)> = walkdir::WalkDir::new(input.scripts_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("p7"))
                .unwrap_or(false)
        })
        .map(|e| {
            let abs = e.path().to_path_buf();
            let rel = abs
                .strip_prefix(input.scripts_dir)
                .expect("walkdir paths are under scripts_dir")
                .to_path_buf();
            (abs, rel)
        })
        .collect();

    // Deterministic order so the resulting ypk is byte-stable across
    // rebuilds and platforms.
    walked.sort_by(|a, b| forward_slashes(&a.1).cmp(&forward_slashes(&b.1)));

    let mut entries: Vec<CollectedEntry> =
        Vec::with_capacity(walked.len() + input.extra_files.len());
    let mut seen_virtual_entries: HashSet<String> = HashSet::new();

    for (abs, rel) in walked {
        let virtual_entry = forward_slashes(&rel);
        ensure_unique(&mut seen_virtual_entries, &virtual_entry)?;
        entries.push(CollectedEntry {
            source_path: abs,
            virtual_entry,
        });
    }

    for extra in input.extra_files {
        let virtual_entry = forward_slashes(Path::new(extra.virtual_entry));
        ensure_unique(&mut seen_virtual_entries, &virtual_entry)?;
        entries.push(CollectedEntry {
            source_path: extra.source_path.to_path_buf(),
            virtual_entry,
        });
    }

    Ok(entries)
}

fn ensure_unique(seen: &mut HashSet<String>, value: &str) -> anyhow::Result<()> {
    if !seen.insert(value.to_string()) {
        anyhow::bail!("duplicate virtual entry {value:?} in script package input");
    }
    Ok(())
}

fn forward_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "yaobow_script_pkg_{}_{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pack_emits_ypk_with_magic_header() {
        let scripts = tmp_dir("magic_src");
        fs::write(scripts.join("foo.p7"), "// foo").unwrap();

        let out_dir = tmp_dir("magic_out");
        let out = out_dir.join("bundle.ypk");

        pack(
            &PackInput {
                scripts_dir: &scripts,
                extra_files: &[],
            },
            &out,
        )
        .unwrap();

        let bytes = fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], b"YPK\x01");
    }

    #[test]
    fn duplicate_virtual_entry_rejected() {
        let scripts = tmp_dir("dup_src");
        fs::write(scripts.join("foo.p7"), "// foo").unwrap();
        let extra = tmp_dir("dup_extra").join("foo.p7");
        fs::write(&extra, "// extra foo").unwrap();

        let out_dir = tmp_dir("dup_out");
        let out = out_dir.join("bundle.ypk");

        let err = pack(
            &PackInput {
                scripts_dir: &scripts,
                extra_files: &[ExtraFile {
                    source_path: &extra,
                    virtual_entry: "foo.p7",
                }],
            },
            &out,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("duplicate virtual entry"));
    }

    #[test]
    fn deterministic_output_across_two_packs() {
        let scripts = tmp_dir("det_src");
        fs::write(scripts.join("a.p7"), "// a").unwrap();
        fs::write(scripts.join("b.p7"), "// b").unwrap();
        fs::create_dir_all(scripts.join("nested")).unwrap();
        fs::write(scripts.join("nested").join("c.p7"), "// c").unwrap();

        let out_dir = tmp_dir("det_out");
        let out1 = out_dir.join("one.ypk");
        let out2 = out_dir.join("two.ypk");

        let mk_input = || PackInput {
            scripts_dir: &scripts,
            extra_files: &[],
        };
        pack(&mk_input(), &out1).unwrap();
        pack(&mk_input(), &out2).unwrap();

        assert_eq!(fs::read(&out1).unwrap(), fs::read(&out2).unwrap());
    }
}
