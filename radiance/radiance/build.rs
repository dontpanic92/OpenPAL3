use std::process::Command;

fn main() {
    build_shader("simple_triangle.vert");
    build_shader("simple_triangle.frag");
}

fn build_shader(shader_name: &str) {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::fs::canonicalize(
        std::path::PathBuf::from(manifest_dir)
            .join("src")
            .join("shaders")
            .join(shader_name),
    )
    .unwrap();
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
    let shader_out_dir = format!("{}/{}.spv", out_dir, shader_name);

    let output = Command::new("glslc")
        .args(&[
            path.to_str().unwrap().to_owned(),
            "-o".to_string(),
            shader_out_dir,
        ])
        .output()
        .expect(&format!("Failed to compile shader {}", shader_name));

    println!("{}", std::str::from_utf8(&output.stdout).unwrap());
}
