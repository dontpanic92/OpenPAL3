#version 450
#extension GL_ARB_separate_shader_objects : enable

// PAL3 static prop shader (CVD items) — ambient-only. Reproduces the original
// PAL3 `gfxscript/geom.gbf` (no-light pass): the prop is simply the texture
// modulated by the scene ambient, with no per-vertex directional term. CVD
// props in the original are evenly dim, not split bright/dark, so they must
// NOT pick up the nearest scene lights the way POL/role geometry does.

layout(constant_id = 0) const bool ALPHA_TEST = true;

layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;            // rgb = ambient color
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
    vec4 misc;               // x = alpha_ref (when ALPHA_TEST), w = fog_exempt
    vec4 uv_xform;
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec3 fragNormal;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 sampled = texture(texSampler, fragTexCoord);
    if (ALPHA_TEST && sampled.a < mat.misc.x) {
        discard;
    }

    // CVD props are evenly dim in the original but still clearly readable, not
    // near-black. The scene floor (~0.1) is too dark, so lift to a modest floor.
    vec3 ambient = max(perFrameUbo.ambient.rgb, vec3(0.35));
    vec3 rgb = sampled.rgb * ambient * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, sampled.a * mat.tint.a);

    if (perFrameUbo.fogParams.x > 0.5 && mat.misc.w < 0.5) {
        float dist = -(vec4(fragWorldPos, 1.0) * perFrameUbo.view).z;
        float fStart = perFrameUbo.fogParams.y;
        float fEnd = perFrameUbo.fogParams.z;
        float vis = clamp((fEnd - dist) / max(fEnd - fStart, 1e-4), 0.0, 1.0);
        outColor.rgb = mix(perFrameUbo.fogColor.rgb * outColor.a, outColor.rgb, vis);
    }
}
