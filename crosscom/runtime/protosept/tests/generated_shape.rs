//! Verifies the protosept emitter (which now also encodes the COM
//! dispatch contract via `@foreign` attributes) produces strings that
//! match the shape the host dispatcher expects.

#[test]
fn protosept_radiance_has_expected_foreign_shape() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let idl_dir = manifest_dir.join("..").join("..").join("idl");

    let radiance = crosscom_ccidl::generate_protosept(idl_dir.join("radiance.idl"))
        .unwrap()
        .source;

    // Each interface gets a `@foreign(...)` proto. The dispatcher and
    // finalizer names are constants the host registers under the same
    // labels; the type_tag is the canonical "<rust-module-dotted>.<I>".
    assert!(
        radiance.contains("@foreign(dispatcher=\"com.invoke\", finalizer=\"com.release\""),
        "missing @foreign attribute on emitted protos"
    );
    assert!(
        radiance.contains("type_tag=\"radiance.comdef.IDirector\""),
        "missing canonical type_tag for IDirector"
    );
    assert!(
        radiance.contains("pub proto IDirector"),
        "missing IDirector proto declaration"
    );
    assert!(
        radiance.contains("pub let IDirector_UUID: string ="),
        "missing IDirector UUID constant"
    );
    // Class Handle structs are no longer emitted.
    assert!(
        !radiance.contains("DirectorHandle"),
        "leftover Handle struct from old emitter"
    );
    assert!(
        !radiance.contains("@intrinsic"),
        "leftover @intrinsic annotations from old emitter"
    );
}

#[test]
fn protosept_editor_has_expected_foreign_shape() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let idl_dir = manifest_dir.join("..").join("..").join("idl");
    let editor = crosscom_ccidl::generate_protosept(idl_dir.join("editor.idl"))
        .unwrap()
        .source;
    assert!(
        editor.contains("type_tag=\"radiance_editor.comdef.IViewContent\""),
        "editor proto canonical type_tag"
    );
}

#[test]
fn protosept_skips_internal_methods() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let idl_dir = manifest_dir.join("..").join("..").join("idl");
    let radiance = crosscom_ccidl::generate_protosept(idl_dir.join("radiance.idl"))
        .unwrap()
        .source;
    // [internal(), rust()] methods don't appear at all.
    assert!(
        !radiance.contains("set_rendering_component"),
        "internal method leaked into protosept output"
    );
}
