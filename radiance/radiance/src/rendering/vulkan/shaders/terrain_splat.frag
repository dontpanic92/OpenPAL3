#version 450
#extension GL_ARB_separate_shader_objects : enable

// PAL5 terrain splat — fragment stage.
//
// Blends up to four terrain textures per-texel by a weight atlas, then
// applies the same dynamic Lambert lighting as `actor_lit.frag`.
//
// Texture bindings (set 2, `texSampler[5]`):
//   texSampler[0..3] = terrain-texture layers for slots 0..3 (unused slots
//                      are padded with slot 0's texture)
//   texSampler[4]    = per-block weight atlas; RGBA = slots 0,1,2,3 weights
//
// The weight atlas is sampled with the per-block UV (`fragAtlasUV`); the
// terrain textures tile in world space (UV derived from `fragWorldPos`).
// All terrain textures are loaded straight/opaque (no premultiply), so the
// blended color is correct ground color.

layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;            // rgb = ambient color, w = light count
    vec4 lightPos[16];       // xyz = world position, w = outer radius
    vec4 lightColor[16];     // rgb = color, w = inner radius
    vec4 sunDir;             // xyz = dir toward sun, w = enabled (1/0)
    vec4 sunColor;           // rgb = sun color
} perFrameUbo;

layout(set = 2, binding = 0) uniform sampler2D texSampler[5];
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;               // x = active layer count (1..4)
    vec4 uv_xform;           // xy = world->tile scale, zw = unused
} mat;

layout(location = 0) in vec2 fragAtlasUV;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec3 fragNormal;

layout(location = 0) out vec4 outColor;

void main() {
    // World-tiled UV for the ground textures (one repeat per `uv_xform.xy`
    // world units).
    vec2 tileUV = fragWorldPos.xz * mat.uv_xform.xy;

    // Per-texel layer weights (already normalized so active layers sum to 1).
    vec4 w = texture(texSampler[4], fragAtlasUV);
    int layers = int(mat.misc.x + 0.5);

    vec3 base = texture(texSampler[0], tileUV).rgb;
    vec3 col = base * w.r;
    if (layers > 1) col += texture(texSampler[1], tileUV).rgb * w.g;
    if (layers > 2) col += texture(texSampler[2], tileUV).rgb * w.b;
    if (layers > 3) col += texture(texSampler[3], tileUV).rgb * w.a;

    // Guard against an all-zero weight texel (e.g. atlas seam): fall back to
    // the base layer so terrain never shows black.
    float wsum = w.r + (layers > 1 ? w.g : 0.0)
                     + (layers > 2 ? w.b : 0.0)
                     + (layers > 3 ? w.a : 0.0);
    if (wsum < 0.001) col = base;

    // Dynamic Lambert lighting (mirrors actor_lit.frag).
    vec3 N = normalize(fragNormal);
    vec3 lit = perFrameUbo.ambient.rgb;
    int count = int(perFrameUbo.ambient.w);
    for (int i = 0; i < count; i++) {
        vec3 d = perFrameUbo.lightPos[i].xyz - fragWorldPos;
        float dist = length(d);
        vec3 L = dist > 0.0 ? d / dist : vec3(0.0, 1.0, 0.0);
        float ndl = max(dot(N, L), 0.2);

        float outer = perFrameUbo.lightPos[i].w;
        float inner = perFrameUbo.lightColor[i].w;
        float atten = 1.0;
        if (outer < 1.0e18) {
            float edge0 = max(inner, outer * 0.85);
            atten = 1.0 - smoothstep(edge0, outer, dist);
        }
        lit += perFrameUbo.lightColor[i].rgb * ndl * atten;
    }

    // Directional sun (parallel, no attenuation); mirrors actor_lit.frag.
    if (perFrameUbo.sunDir.w > 0.5) {
        vec3 Ls = normalize(perFrameUbo.sunDir.xyz);
        float ndlS = max(dot(N, Ls), 0.2);
        lit += perFrameUbo.sunColor.rgb * ndlS;
    }

    outColor = vec4(col * lit * mat.tint.rgb, 1.0);
}
