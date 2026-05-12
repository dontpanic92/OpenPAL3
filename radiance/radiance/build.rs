use std::path::PathBuf;
use std::process::Command;

mod features;

fn main() {
    features::enable_features();
    generate_comdef("radiance.idl", "radiance_comdef.rs");

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

fn generate_comdef(idl_file: &str, out_file: &str) {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let idl_path = workspace_root.join("crosscom").join("idl").join(idl_file);
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(out_file);
    let dependencies = crosscom_ccidl::generate_to_file(&idl_path, &out_path)
        .unwrap_or_else(|err| panic!("Failed to generate {}: {}", out_file, err));

    for dependency in dependencies {
        println!("cargo:rerun-if-changed={}", dependency.display());
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

    let shader_path = path.to_str().unwrap();
    let output = Command::new("glslc")
        .arg(shader_path)
        .arg("-o")
        .arg(&shader_out_dir)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "Failed to find or execute glslc for shader {}: {}. Ensure the Vulkan SDK is installed and glslc is in PATH.",
                shader_name, err
            )
        });

    if !output.status.success() {
        panic!(
            "Failed to compile shader {} with glslc: stderr: {} stdout: {}",
            shader_name,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    if !output.stdout.is_empty() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }
}
