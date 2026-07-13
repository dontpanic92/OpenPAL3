//! End-to-end test exercising the full asset-project workflow: record
//! changes in a `ProjectManifest`, store their payloads in a
//! `PayloadStore`, publish a `.yapatch` (atomically), read it back
//! (verifying hashes along the way), and record the install in a
//! journal.

use std::path::PathBuf;

use asset_project::manifest::{AssetChangeKey, PackagePath, TargetPackage};
use asset_project::patch::PackageFingerprint;
use asset_project::{
    AssetChange, AssetChangeKind, AssetProjectError, ContentHash, InstallationJournal,
    PayloadStore, ProjectManifest, YapatchReader, YapatchWriter, publish,
};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn full_project_to_patch_to_install_workflow() {
    let dir = scratch_dir("e2e");

    // --- Editor side: record changes + store converted payloads. ---
    let payload_store = PayloadStore::new(dir.join("objects"));

    let texture_bytes = b"pretend-this-is-a-converted-dds-texture".to_vec();
    let model_bytes = b"pretend-this-is-a-converted-dff-model".to_vec();
    payload_store.put(&texture_bytes).unwrap();
    payload_store.put(&model_bytes).unwrap();

    let scene_cpk = TargetPackage::new("scene.cpk").unwrap();

    let mut manifest = ProjectManifest::new("demo-project", "pal3", dir.join("base-assets"));
    manifest.upsert_change(AssetChange::from_payload(
        AssetChangeKind::Add,
        scene_cpk.clone(),
        PackagePath::new("texture/hero.dds").unwrap(),
        &texture_bytes,
        None,
        None,
        None,
    ));
    manifest.upsert_change(AssetChange::from_payload(
        AssetChangeKind::Replace,
        scene_cpk.clone(),
        PackagePath::new("model/hero.dff").unwrap(),
        &model_bytes,
        Some(ContentHash::of(b"previous hero.dff contents")),
        None,
        None,
    ));

    let manifest_path = dir.join("project.json");
    manifest.save(&manifest_path).unwrap();

    // --- Pack a .yapatch from the (re-loaded, to prove persistence
    // round-trips) project manifest. ---
    let reloaded = ProjectManifest::load(&manifest_path).unwrap();
    assert_eq!(reloaded.target_game, "pal3");
    assert_eq!(reloaded.base_asset_root, dir.join("base-assets"));

    let patch_path = dir.join("update.yapatch");
    let mut writer =
        YapatchWriter::create(&patch_path, reloaded.target_game.clone(), reloaded.version).unwrap();
    writer.add_package_fingerprint(PackageFingerprint {
        target_package: scene_cpk.clone(),
        base_hash: ContentHash::of(b"scene.cpk base state fingerprint"),
    });
    for change in reloaded.changes() {
        let payload = payload_store.get(change.payload.content_hash).unwrap();
        writer.add_change(change.clone(), &payload).unwrap();
    }
    let patch_manifest = writer.finish().unwrap();
    assert_eq!(patch_manifest.changes.len(), 2);
    assert_eq!(patch_manifest.target_game, "pal3");
    assert_eq!(patch_manifest.package_fingerprints.len(), 1);
    assert!(patch_manifest.fingerprint_for(&scene_cpk).is_some());

    // Only the verified, finished file should exist at the destination
    // -- no leftover temp file from the atomic publish sequence.
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    assert!(patch_path.exists());

    // --- Patcher side: open + verify the patch. ---
    let mut reader = YapatchReader::open(&patch_path).unwrap();
    assert_eq!(reader.manifest().changes.len(), 2);
    reader.verify_all().unwrap();

    let hero_dds = PackagePath::new("texture/hero.dds").unwrap();
    let change = reader
        .manifest()
        .changes
        .iter()
        .find(|c| c.package_internal_path == hero_dds)
        .cloned()
        .unwrap();
    let payload = reader.read_payload(&change).unwrap();
    assert_eq!(payload, texture_bytes);

    // --- Installation journal bookkeeping. ---
    let journal_path = dir.join("journal.json");
    let mut journal = InstallationJournal::load_or_default(&journal_path).unwrap();

    let manifest_hash_for_journal = ContentHash::of(b"opaque .yapatch manifest bytes marker");
    journal
        .begin(
            patch_manifest.patch_id,
            &patch_path,
            manifest_hash_for_journal,
            patch_manifest.base_project_version,
        )
        .unwrap();
    assert!(!journal.is_applied(patch_manifest.patch_id));

    let applied_paths: Vec<String> = reader
        .manifest()
        .changes
        .iter()
        .map(|c| c.package_internal_path.as_str().to_string())
        .collect();
    journal
        .complete(patch_manifest.patch_id, applied_paths)
        .unwrap();
    journal.save(&journal_path).unwrap();

    let reloaded_journal = InstallationJournal::load_or_default(&journal_path).unwrap();
    assert!(reloaded_journal.is_applied(patch_manifest.patch_id));
    assert_eq!(reloaded_journal.entries()[0].changes_applied.len(), 2);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn yapatch_rejects_a_tampered_payload() {
    let dir = scratch_dir("e2e-tamper");
    let payload_store = PayloadStore::new(dir.join("objects"));

    let payload = b"authentic payload".to_vec();
    payload_store.put(&payload).unwrap();

    let change = AssetChange::from_payload(
        AssetChangeKind::Add,
        TargetPackage::new("scene.cpk").unwrap(),
        PackagePath::new("data/file.bin").unwrap(),
        &payload,
        None,
        None,
        None,
    );

    let patch_path = dir.join("tampered.yapatch");
    let mut writer = YapatchWriter::create(&patch_path, "pal3", 1).unwrap();
    writer.add_change(change.clone(), &payload).unwrap();
    writer.finish().unwrap();

    // Simulate the change record drifting from the payload (e.g. a
    // hand-edited manifest, or a bug upstream) by asking the reader to
    // verify a change whose declared hash doesn't match what's stored.
    let mut tampered_change = change;
    tampered_change.payload.content_hash = ContentHash::of(b"not the real payload");

    let mut reader = YapatchReader::open(&patch_path).unwrap();
    let err = reader.read_payload(&tampered_change).unwrap_err();
    assert!(matches!(err, AssetProjectError::HashMismatch { .. }));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn yapatch_writer_rejects_change_with_wrong_declared_hash() {
    let dir = scratch_dir("e2e-writer-mismatch");
    let patch_path = dir.join("bad.yapatch");
    let mut writer = YapatchWriter::create(&patch_path, "pal3", 1).unwrap();

    let mut change = AssetChange::from_payload(
        AssetChangeKind::Add,
        TargetPackage::new("scene.cpk").unwrap(),
        PackagePath::new("data/file.bin").unwrap(),
        b"real payload",
        None,
        None,
        None,
    );
    change.payload.content_hash = ContentHash::of(b"wrong hash source");

    let err = writer.add_change(change, b"real payload").unwrap_err();
    assert!(matches!(err, AssetProjectError::HashMismatch { .. }));

    // The destination must not exist: a failed `add_change` happens
    // before `finish`, so no temp-file/rename dance has even started,
    // but this also guards against any future refactor that might
    // publish eagerly.
    assert!(!patch_path.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn yapatch_publish_leaves_destination_untouched_on_verification_failure() {
    // There's no direct way to make `finish()`'s re-verification fail
    // without corrupting the temp file mid-flight (which races the
    // writer itself), so instead this exercises the documented
    // guarantee at the level available through the public API: a
    // pre-existing destination file is left completely alone until a
    // `finish()` call actually succeeds.
    let dir = scratch_dir("e2e-publish-untouched");
    let patch_path = dir.join("existing.yapatch");
    std::fs::write(&patch_path, b"pre-existing, unrelated content").unwrap();

    let writer = YapatchWriter::create(&patch_path, "pal3", 1).unwrap();
    // Dropping the writer without calling `finish()` must never touch
    // `patch_path` -- only a successful `finish()` may rename over it.
    drop(writer);

    assert_eq!(
        std::fs::read(&patch_path).unwrap(),
        b"pre-existing, unrelated content"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn publish_convenience_api_matches_incremental_writer() {
    let dir = scratch_dir("e2e-publish-convenience");
    let payload = b"convenience payload".to_vec();

    let change = AssetChange::from_payload(
        AssetChangeKind::Add,
        TargetPackage::new("scene.cpk").unwrap(),
        PackagePath::new("data/file.bin").unwrap(),
        &payload,
        None,
        None,
        None,
    );

    let patch_path = dir.join("convenience.yapatch");
    let manifest = publish(
        &patch_path,
        "pal3",
        1,
        vec![],
        vec![(change.clone(), payload.clone())],
    )
    .unwrap();
    assert_eq!(manifest.changes.len(), 1);

    let mut reader = YapatchReader::open(&patch_path).unwrap();
    reader.verify_all().unwrap();
    assert_eq!(reader.read_payload(&change).unwrap(), payload);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn package_path_rejects_traversal_end_to_end() {
    assert!(matches!(
        PackagePath::new("../../etc/passwd").unwrap_err(),
        AssetProjectError::InvalidPath { .. }
    ));
    assert!(matches!(
        TargetPackage::new("/etc/passwd").unwrap_err(),
        AssetProjectError::InvalidPath { .. }
    ));
}

#[test]
fn asset_change_key_disambiguates_same_internal_path_across_packages() {
    let key_a = AssetChangeKey::new(
        TargetPackage::new("scene.cpk").unwrap(),
        PackagePath::new("shared/name.txt").unwrap(),
    );
    let key_b = AssetChangeKey::new(
        TargetPackage::new("basedata.cpk").unwrap(),
        PackagePath::new("shared/name.txt").unwrap(),
    );
    assert_ne!(key_a, key_b);
}
