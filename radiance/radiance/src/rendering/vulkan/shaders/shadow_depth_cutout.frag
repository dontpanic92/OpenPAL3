#version 450
#extension GL_ARB_separate_shader_objects : enable

// Cutout depth pass: sample the caster's albedo alpha and discard texels
// below the material's alpha reference so leaf/grass cards cast a textured
// (leaf-shaped) shadow instead of a solid rectangle. Writes only depth.
//
// Set 2 (texture) + set 3 (material params) mirror the main color pipelines,
// so the same per-object texture + per-material params descriptors are bound.
layout(set = 2, binding = 0) uniform sampler2D texSampler;
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref
    vec4 uv_xform;
} mat;

layout(location = 0) in vec2 fragTexCoord;

void main() {
    float a = texture(texSampler, fragTexCoord).a;
    // Use a small floor so near-zero atlas texels always drop even when the
    // material's alpha_ref is ~0 (premultiplied cutout convention).
    float ref = max(mat.misc.x, 0.33);
    if (a < ref) {
        discard;
    }
}
