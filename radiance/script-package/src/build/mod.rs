//! Build-script-facing API: walks `scripts/` directories, classifies entries
//! (root / idl binding / module), and writes a `.ypk` whose wire format is
//! identical to `radiance::asset::ypk::YpkWriter`.
//!
//! Gated behind the `build` feature so runtime crates that only need
//! [`super::PackageManifest`] / [`super::ManifestEntry`] don't pull in
//! `walkdir` / `binrw` / `zstd` / `xxhash-rust`.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{ManifestEntry, PACKAGE_MANIFEST_ENTRY, PackageManifest};

mod ypk_writer;

use ypk_writer::PackerYpkWriter;

/// Classification of a script entry inside the ypk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
    /// Codegen-derived p7 binding (registered before modules so app-owned
    /// modules can `import <name>;`).
    IdlBinding,
    /// App-owned `.p7` source module imported by the root.
    Module,
}

/// One extra file to inject into the ypk, typically a build-script-generated
/// `.p7` sitting in `OUT_DIR`.
pub struct ExtraFile<'a> {
    /// Absolute path on disk.
    pub source_path: &'a Path,
    /// Path inside the ypk (e.g. `idl/yaobow_services.p7`). Must be unique
    /// across all walked + extra files.
    pub virtual_entry: &'a str,
    /// p7 import name (e.g. `yaobow_services`).
    pub module_name: &'a str,
    /// Whether to register this entry under `idl_bindings` or `modules`.
    pub kind: ModuleKind,
}

/// Input to [`pack`].
pub struct PackInput<'a> {
    /// Directory to walk recursively for `*.p7` files.
    pub scripts_dir: &'a Path,
    /// File name (relative to `scripts_dir`, e.g. `app.p7`) of the root
    /// source. `None` produces a module-bundle package (no root).
    pub root_entry: Option<&'a str>,
    /// Human-readable root name (e.g. `app`, `main`). Required iff
    /// `root_entry` is `Some`.
    pub root_name: Option<&'a str>,
    /// Extra files (typically from `OUT_DIR`) to inject. Order is preserved
    /// after walked files, except the manifest itself which is always the
    /// first entry.
    pub extra_files: &'a [ExtraFile<'a>],
}

/// Pack `input` into a `.ypk` at `output_ypk`.
///
/// Walks `input.scripts_dir` recursively, collects every `*.p7` file in
/// deterministic lexicographic order, classifies each entry (root /
/// idl_bindings / modules), then writes them all (plus the
/// `__package.json` manifest as the first entry) using a slim
/// [`PackerYpkWriter`] whose wire format matches
/// `radiance::asset::ypk::YpkWriter`.
///
/// Emits `cargo:rerun-if-changed=` for every walked file, every
/// `extra_files[i].source_path`, and the `scripts_dir` itself (so adds /
/// deletes invalidate the build).
pub fn pack(input: &PackInput, output_ypk: &Path) -> anyhow::Result<()> {
    let entries = collect_entries(input)?;

    // rerun-if-changed bookkeeping.
    //
    // We only track files that originate in `scripts_dir` because
    // those are the ones the build script "owns". Files contributed
    // via `extra_files` are typically codegen outputs sitting in
    // `OUT_DIR`; their upstream generators already emit their own
    // `cargo:rerun-if-changed=` for the underlying inputs (e.g. IDL
    // files). Tracking them here would cause a feedback loop because
    // the codegen rewrites the OUT_DIR p7 on every build, refreshing
    // its mtime and forcing the next build to re-run this build.rs.
    println!("cargo:rerun-if-changed={}", input.scripts_dir.display());
    for entry in &entries {
        let from_scripts_dir = entry.source_path.starts_with(input.scripts_dir);
        if from_scripts_dir {
            println!("cargo:rerun-if-changed={}", entry.source_path.display());
        }
    }

    if let Some(parent) = output_ypk.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let (manifest, ordered) = build_manifest(input, &entries)?;
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    let file = std::fs::File::create(output_ypk)?;
    let mut writer = PackerYpkWriter::new(Box::new(file))?;

    // Manifest is always the first entry so a reader can locate it without
    // a full directory scan if it wants to.
    writer.write_file(PACKAGE_MANIFEST_ENTRY, &manifest_bytes)?;

    for entry in ordered {
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
    /// p7 import name.
    module_name: String,
    /// `Root` is only used internally — the manifest separates root from
    /// `idl_bindings` / `modules`.
    kind: ClassifiedKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassifiedKind {
    Root,
    IdlBinding,
    Module,
}

fn collect_entries(input: &PackInput) -> anyhow::Result<Vec<CollectedEntry>> {
    if input.root_entry.is_some() != input.root_name.is_some() {
        anyhow::bail!(
            "PackInput::root_entry and root_name must be set together (both Some or both None)"
        );
    }

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

    // Deterministic order: lexicographic on the virtual (relative) path,
    // using forward-slash normalization so behavior is stable across
    // Windows/macOS/Linux.
    walked.sort_by(|a, b| forward_slashes(&a.1).cmp(&forward_slashes(&b.1)));

    let mut entries: Vec<CollectedEntry> = Vec::with_capacity(walked.len() + input.extra_files.len());
    let mut seen_module_names: HashSet<String> = HashSet::new();
    let mut seen_virtual_entries: HashSet<String> = HashSet::new();

    for (abs, rel) in walked {
        let virtual_entry = forward_slashes(&rel);
        let module_name = rel
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Cannot derive module name from {}", abs.display()))?
            .to_string();

        let kind = if input
            .root_entry
            .map(|root| eq_relative(&rel, root))
            .unwrap_or(false)
        {
            ClassifiedKind::Root
        } else if is_idl_path(&rel) {
            ClassifiedKind::IdlBinding
        } else {
            ClassifiedKind::Module
        };

        ensure_unique(&mut seen_module_names, &module_name, "module name")?;
        ensure_unique(&mut seen_virtual_entries, &virtual_entry, "virtual entry")?;

        entries.push(CollectedEntry {
            source_path: abs,
            virtual_entry,
            module_name,
            kind,
        });
    }

    if let Some(root) = input.root_entry {
        if !entries.iter().any(|e| e.kind == ClassifiedKind::Root) {
            anyhow::bail!(
                "PackInput::root_entry = {root:?} but no matching file under {}",
                input.scripts_dir.display()
            );
        }
    }

    for extra in input.extra_files {
        let virtual_entry = forward_slashes(Path::new(extra.virtual_entry));
        ensure_unique(&mut seen_module_names, extra.module_name, "module name")?;
        ensure_unique(&mut seen_virtual_entries, &virtual_entry, "virtual entry")?;

        let kind = match extra.kind {
            ModuleKind::IdlBinding => ClassifiedKind::IdlBinding,
            ModuleKind::Module => ClassifiedKind::Module,
        };

        entries.push(CollectedEntry {
            source_path: extra.source_path.to_path_buf(),
            virtual_entry,
            module_name: extra.module_name.to_string(),
            kind,
        });
    }

    Ok(entries)
}

fn build_manifest(
    input: &PackInput,
    entries: &[CollectedEntry],
) -> anyhow::Result<(PackageManifest, Vec<CollectedEntry>)> {
    // Bucket walked + extra entries deterministically. BTreeMap keyed on
    // module name keeps idl_bindings / modules sorted so the manifest JSON
    // is byte-deterministic across rebuilds.
    let mut idl: BTreeMap<String, ManifestEntry> = BTreeMap::new();
    let mut modules: BTreeMap<String, ManifestEntry> = BTreeMap::new();
    let mut root: Option<ManifestEntry> = None;

    for entry in entries {
        let manifest_entry = ManifestEntry {
            name: entry.module_name.clone(),
            entry: entry.virtual_entry.clone(),
        };

        match entry.kind {
            ClassifiedKind::Root => {
                if root.is_some() {
                    anyhow::bail!(
                        "Multiple files matched root_entry={:?}",
                        input.root_entry
                    );
                }
                root = Some(manifest_entry);
            }
            ClassifiedKind::IdlBinding => {
                idl.insert(manifest_entry.name.clone(), manifest_entry);
            }
            ClassifiedKind::Module => {
                modules.insert(manifest_entry.name.clone(), manifest_entry);
            }
        }
    }

    let manifest = PackageManifest {
        root_name: input.root_name.map(str::to_string),
        root_entry: root.as_ref().map(|e| e.entry.clone()),
        idl_bindings: idl.into_values().collect(),
        modules: modules.into_values().collect(),
    };

    // Write order: root first (if any), then idl_bindings, then modules.
    // Manifest itself is written separately by `pack` before this list.
    let mut ordered: Vec<CollectedEntry> = Vec::with_capacity(entries.len());
    if let Some(root_entry) = &manifest.root_entry {
        if let Some(e) = entries.iter().find(|e| &e.virtual_entry == root_entry) {
            ordered.push(e.clone());
        }
    }
    for binding in &manifest.idl_bindings {
        if let Some(e) = entries.iter().find(|e| e.virtual_entry == binding.entry) {
            ordered.push(e.clone());
        }
    }
    for module in &manifest.modules {
        if let Some(e) = entries.iter().find(|e| e.virtual_entry == module.entry) {
            ordered.push(e.clone());
        }
    }

    Ok((manifest, ordered))
}

fn ensure_unique(seen: &mut HashSet<String>, value: &str, kind: &str) -> anyhow::Result<()> {
    if !seen.insert(value.to_string()) {
        anyhow::bail!("duplicate {kind} {value:?} in script package input");
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

fn eq_relative(rel: &Path, root_entry: &str) -> bool {
    forward_slashes(rel) == forward_slashes(Path::new(root_entry))
}

fn is_idl_path(rel: &Path) -> bool {
    rel.components()
        .any(|c| matches!(c, std::path::Component::Normal(s) if s == "idl"))
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
    fn pack_module_bundle_without_root() {
        let scripts = tmp_dir("module_bundle_src");
        fs::write(scripts.join("foo.p7"), "// foo").unwrap();
        fs::create_dir_all(scripts.join("idl")).unwrap();
        fs::write(scripts.join("idl").join("bar.p7"), "// bar").unwrap();

        let out_dir = tmp_dir("module_bundle_out");
        let out = out_dir.join("bundle.ypk");

        pack(
            &PackInput {
                scripts_dir: &scripts,
                root_entry: None,
                root_name: None,
                extra_files: &[],
            },
            &out,
        )
        .unwrap();

        assert!(out.exists());

        // Spot-check the ypk produced is non-empty and starts with the
        // shared magic.
        let bytes = fs::read(&out).unwrap();
        assert_eq!(&bytes[..4], b"YPK\x01");
    }

    #[test]
    fn pack_with_root_validates_root_present() {
        let scripts = tmp_dir("root_missing_src");
        fs::write(scripts.join("foo.p7"), "// foo").unwrap();

        let out_dir = tmp_dir("root_missing_out");
        let out = out_dir.join("bundle.ypk");

        let err = pack(
            &PackInput {
                scripts_dir: &scripts,
                root_entry: Some("app.p7"),
                root_name: Some("app"),
                extra_files: &[],
            },
            &out,
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("app.p7"), "got {msg}");
    }

    #[test]
    fn duplicate_module_names_rejected() {
        let scripts = tmp_dir("dup_src");
        fs::write(scripts.join("foo.p7"), "// foo a").unwrap();
        fs::create_dir_all(scripts.join("idl")).unwrap();
        // Same module name (file stem "foo") from a different dir collides.
        fs::write(scripts.join("idl").join("foo.p7"), "// foo b").unwrap();

        let out_dir = tmp_dir("dup_out");
        let out = out_dir.join("bundle.ypk");

        let err = pack(
            &PackInput {
                scripts_dir: &scripts,
                root_entry: None,
                root_name: None,
                extra_files: &[],
            },
            &out,
        )
        .unwrap_err();
        assert!(format!("{err}").contains("duplicate module name"));
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
            root_entry: None,
            root_name: None,
            extra_files: &[],
        };
        pack(&mk_input(), &out1).unwrap();
        pack(&mk_input(), &out2).unwrap();

        let b1 = fs::read(&out1).unwrap();
        let b2 = fs::read(&out2).unwrap();
        assert_eq!(b1, b2, "ypk byte output must be deterministic");
    }

    #[test]
    fn manifest_classifies_root_idl_and_modules() {
        let scripts = tmp_dir("classify_src");
        fs::write(scripts.join("app.p7"), "// root").unwrap();
        fs::write(scripts.join("title.p7"), "// title module").unwrap();
        fs::create_dir_all(scripts.join("idl")).unwrap();
        fs::write(scripts.join("idl").join("svc.p7"), "// svc binding").unwrap();

        let collected = collect_entries(&PackInput {
            scripts_dir: &scripts,
            root_entry: Some("app.p7"),
            root_name: Some("app"),
            extra_files: &[],
        })
        .unwrap();

        let kinds: std::collections::HashMap<_, _> = collected
            .iter()
            .map(|e| (e.module_name.clone(), e.kind))
            .collect();
        assert_eq!(kinds["app"], ClassifiedKind::Root);
        assert_eq!(kinds["title"], ClassifiedKind::Module);
        assert_eq!(kinds["svc"], ClassifiedKind::IdlBinding);
    }
}
