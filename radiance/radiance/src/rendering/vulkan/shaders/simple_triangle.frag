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

// Per-frame UBO prefix (shared backing buffer with the lit shaders). Only the
// members this shader reads are declared up to `fogParams`; the std140 layout
// matches `PerFrameUniformBuffer` exactly so the trailing fog fields land at
// the right offset.
layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;
    vec4 lightPos[16];
    vec4 lightColor[16];
    vec4 sunDir;
    vec4 sunColor;
    mat4 lightViewProj[3];
    vec4 cascadeSplits;
    vec4 shadowParams;
    vec4 fogColor;           // rgb = linear fog color
    vec4 fogParams;          // x = enabled, y = start depth, z = end depth
} perFrameUbo;

layout(set = 2, binding = 0) uniform sampler2D texSampler;
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref (only consulted when ALPHA_TEST is true)
                     // w = fog_exempt
    vec4 uv_xform;   // reserved
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec3 fragWorldPos;
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

    // Linear distance fog (gated on scene fog + per-material exemption). The
    // skybox sets `fog_exempt` (misc.w) so it is never washed to fog color.
    if (perFrameUbo.fogParams.x > 0.5 && mat.misc.w < 0.5) {
        float d = -(vec4(fragWorldPos, 1.0) * perFrameUbo.view).z;
        float fStart = perFrameUbo.fogParams.y;
        float fEnd = perFrameUbo.fogParams.z;
        float vis = clamp((fEnd - d) / max(fEnd - fStart, 1e-4), 0.0, 1.0);
        outColor.rgb = mix(perFrameUbo.fogColor.rgb * outColor.a, outColor.rgb, vis);
    }
}
