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
    mat4 lightViewProj[3];   // per-cascade world -> shadow clip space
    vec4 cascadeSplits;      // xyz = view-space far depth of cascades 0..2
    vec4 shadowParams;       // x = enabled, y = bias, z = 1/size, w = pcf radius
    vec4 fogColor;           // rgb = linear fog color
    vec4 fogParams;          // x = enabled, y = start depth, z = end depth
} perFrameUbo;

layout(set = 0, binding = 1) uniform sampler2DArray shadowMap;
layout(set = 2, binding = 0) uniform sampler2D texSampler[5];
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;               // x = active layer count (1..4), w = fog_exempt
    vec4 uv_xform;           // xy = world->tile scale, zw = unused
} mat;

layout(location = 0) in vec2 fragAtlasUV;
layout(location = 1) in vec3 fragWorldPos;
layout(location = 2) in vec3 fragNormal;

layout(location = 0) out vec4 outColor;

// Pick the cascade for a fragment by its camera view-space depth (positive
// forward): the first cascade whose split covers it, else the last.
int selectCascade(vec3 worldPos) {
    float viewZ = -(vec4(worldPos, 1.0) * perFrameUbo.view).z;
    if (viewZ <= perFrameUbo.cascadeSplits.x) return 0;
    if (viewZ <= perFrameUbo.cascadeSplits.y) return 1;
    return 2;
}

// Directional-shadow visibility for a world-space point (mirrors
// actor_lit.frag): `1.0` = lit, `0.0` = shadowed; 3×3 PCF with a small bias on
// the selected cascade; off-map / disabled lookups read as lit.
float sunVisibility(vec3 worldPos) {
    if (perFrameUbo.shadowParams.x < 0.5) {
        return 1.0;
    }
    int cascade = selectCascade(worldPos);
    vec4 lp = vec4(worldPos, 1.0) * perFrameUbo.lightViewProj[cascade];
    vec3 proj = lp.xyz / lp.w;
    vec2 uv = proj.xy * 0.5 + 0.5;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || proj.z > 1.0) {
        return 1.0;
    }
    float currentDepth = proj.z - perFrameUbo.shadowParams.y;
    float texel = perFrameUbo.shadowParams.z;
    int r = int(perFrameUbo.shadowParams.w + 0.5);
    float visible = 0.0;
    float count = 0.0;
    for (int x = -r; x <= r; x++) {
        for (int y = -r; y <= r; y++) {
            float closest = texture(shadowMap, vec3(uv + vec2(x, y) * texel, float(cascade))).r;
            visible += currentDepth <= closest ? 1.0 : 0.0;
            count += 1.0;
        }
    }
    return visible / count;
}

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
        lit += perFrameUbo.sunColor.rgb * ndlS * sunVisibility(fragWorldPos);
    }

    outColor = vec4(col * lit * mat.tint.rgb, 1.0);

    // Linear distance fog (gated). Terrain is opaque (alpha 1), so this is a
    // plain blend toward the fog color by view-space depth. Mirrors
    // `actor_lit.frag`.
    if (perFrameUbo.fogParams.x > 0.5 && mat.misc.w < 0.5) {
        float d = -(vec4(fragWorldPos, 1.0) * perFrameUbo.view).z;
        float fStart = perFrameUbo.fogParams.y;
        float fEnd = perFrameUbo.fogParams.z;
        float vis = clamp((fEnd - d) / max(fEnd - fStart, 1e-4), 0.0, 1.0);
        outColor.rgb = mix(perFrameUbo.fogColor.rgb, outColor.rgb, vis);
    }
}
