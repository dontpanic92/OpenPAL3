#version 450
#extension GL_ARB_separate_shader_objects : enable

// PAL5 grass-wind — fragment stage.
//
// Identical sampling/fog behaviour to `simple_triangle.frag`: a single texture
// with an alpha-test cutout, plus linear distance fog. The grass billboard
// texture is baked CPU-side as the real `cao###` ground-grass color masked by
// a blade-shaped alpha (see `openpal5::grass`), so the silhouette reads as a
// tuft while the color is authentic. The wind sway lives entirely in
// `grass.vert`; this stage is colour-only.
layout(constant_id = 0) const bool ALPHA_TEST = true;

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
    vec4 uv_xform;   // x = wind strength, y = wind speed (consumed in vert)
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in float fragCoverage;   // per-cell density coverage (0..1)
layout(location = 0) out vec4 outColor;

void main() {
    vec4 sampled = texture(texSampler, fragTexCoord);
    if (ALPHA_TEST && sampled.a < mat.misc.x) {
        discard;
    }
    // Per-cell coverage scales the overall alpha gain (`tint.a`). The grass
    // overlay loads `cao###` opaque (sampled.a == 1), so coverage is driven by
    // the density-derived `fragCoverage`; output is premultiplied.
    float cov = clamp(mat.tint.a * fragCoverage, 0.0, 1.0);
    outColor = vec4(sampled.rgb * mat.tint.rgb * cov, sampled.a * cov);

    if (perFrameUbo.fogParams.x > 0.5 && mat.misc.w < 0.5) {
        float d = -(vec4(fragWorldPos, 1.0) * perFrameUbo.view).z;
        float fStart = perFrameUbo.fogParams.y;
        float fEnd = perFrameUbo.fogParams.z;
        float vis = clamp((fEnd - d) / max(fEnd - fStart, 1e-4), 0.0, 1.0);
        outColor.rgb = mix(perFrameUbo.fogColor.rgb * outColor.a, outColor.rgb, vis);
    }
}
