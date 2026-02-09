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

    let shader_path = path.to_str().unwrap();
    let output = match Command::new("glslc")
        .arg(shader_path)
        .arg("-o")
        .arg(&shader_out_dir)
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            match Command::new("glslangValidator")
                .arg("-V")
                .arg(shader_path)
                .arg("-o")
                .arg(&shader_out_dir)
                .output()
            {
                Ok(output) => output,
                Err(err) => panic!(
                    "Failed to compile shader {} with glslangValidator: {}",
                    shader_name, err
                ),
            }
        }
        Err(err) => panic!("Failed to compile shader {}: {}", shader_name, err),
    };

    if !output.status.success() {
        panic!(
            "Failed to compile shader {}: {}",
            shader_name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
