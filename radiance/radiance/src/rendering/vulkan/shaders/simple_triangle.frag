#version 450
#extension GL_ARB_separate_shader_objects : enable

// Alpha convention: non-opaque textures are stored **premultiplied** by
// the texture loader (`texture::premultiply_alpha`). The Vulkan pipeline
// uses `ONE / 1-SRC_ALPHA` blend factors to match. The cutout / opaque
// path is unaffected because those textures bypass premultiplication
// (alpha is identically 255).
//
// ALPHA_TEST is selected at pipeline creation via a Vulkan specialization
// constant (constant_id = 0). It is `true` for the `BlendMode::AlphaTest`
// pipeline (cutout) and `false` for every other mode, so opaque draws keep
// early-Z (no `discard`).
layout(constant_id = 0) const bool ALPHA_TEST = true;

layout(set = 2, binding = 0) uniform sampler2D texSampler;
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref (only consulted when ALPHA_TEST is true)
    vec4 uv_xform;   // reserved
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 0) out vec4 outColor;

void main() {
    vec4 sampled = texture(texSampler, fragTexCoord);
    if (ALPHA_TEST && sampled.a < mat.misc.x) {
        discard;
    }
    // Premultiplied invariant: scale RGB *and* alpha by tint.a so the
    // resulting fragment is still premultiplied. Tinting the RGB by
    // tint.rgb (default white) is applied on top.
    outColor = vec4(sampled.rgb * mat.tint.rgb * mat.tint.a, sampled.a * mat.tint.a);
}
