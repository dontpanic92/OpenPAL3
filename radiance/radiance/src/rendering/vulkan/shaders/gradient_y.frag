#version 450
#extension GL_ARB_separate_shader_objects : enable

// `GradientYMaterialDef` encodes its parameters into the existing
// `MaterialParams` UBO via field-reuse so we don't have to add a second
// per-material UBO. The mapping is:
//
//   tint.rgb     = high color (vertex with Y == y_max)
//   tint.a       = y_max
//   misc.x       = unused (would be alpha_ref for other shaders)
//   misc.y       = y_min
//   misc.zw      = unused
//   uv_xform.xy  = low color (R, G channels)
//   uv_xform.zw  = (low color.b, unused)
//
// The dummy `texSampler[0]` slot exists only to satisfy the renderer's
// per-material descriptor layout (which is keyed on textures.len()) —
// it is intentionally not sampled in `main`.
layout(set = 2, binding = 0) uniform sampler2D texSampler[1];
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;
    vec4 uv_xform;
} mat;

layout(location = 0) in float worldY;

layout(location = 0) out vec4 outColor;

void main() {
    float y_max = mat.tint.a;
    float y_min = mat.misc.y;
    vec3  high  = mat.tint.rgb;
    vec3  low   = vec3(mat.uv_xform.x, mat.uv_xform.y, mat.uv_xform.z);

    float t = clamp((worldY - y_min) / max(y_max - y_min, 1e-6), 0.0, 1.0);
    outColor = vec4(mix(low, high, t), 1.0);
}
