use std::process::Command;

mod features;

fn main() {
    features::enable_features();

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "windows" | "linux" | "macos" | "android" => {
            build_vulkan_shader("simple_triangle.vert");
            build_vulkan_shader("simple_triangle.frag");
            build_vulkan_shader("lightmap_texture.vert");
            build_vulkan_shader("lightmap_texture.frag");
        }
        _ => {}
    }
}

#[allow(dead_code)]
fn build_vulkan_shader(shader_name: &str) {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::fs::canonicalize(
        std::path::PathBuf::from(manifest_dir)
            .join("src/rendering/vulkan/shaders")
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
