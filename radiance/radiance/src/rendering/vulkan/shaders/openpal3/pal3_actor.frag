#version 450
#extension GL_ARB_separate_shader_objects : enable

// PAL3-specific actor (skin) shader — fragment stage. Faithfully reproduces
// the original PAL3 `gfxscript/skin_lit2.cg` lighting model:
//
//   color = texture * ( SUM_2lights( atten * max(N·L, 0) * diffuse ) + ambient )
//
// i.e. at most the two nearest omni point lights, a hard N·L clamp at 0 (no
// fill/wrap floor), and ambient added flat. Point-light attenuation is the
// Direct3D form 1/(a0 + a1·d + a2·d²); PAL3 ships FLT_MAX range so attenuation
// is effectively 1 (the lights act near-directional). Diffuse is white via
// the material tint. This is distinct from the shared `actor_lit` shader,
// which sums all scene lights with a 0.2 wrap floor.

layout(constant_id = 0) const bool ALPHA_TEST = true;

layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;            // rgb = ambient color, w = light count
    vec4 lightPos[16];       // xyz = world position, w = outer range
    vec4 lightColor[16];     // rgb = color, w = inner range
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

    vec3 N = normalize(fragNormal);
    int count = int(perFrameUbo.ambient.w);

    // PAL3 actors carry a near-full material ambient (skin.gbf shows the
    // character fully visible on ambient alone); the scene's global ambient
    // floor is only ~0.1 and is meant for scenery. Lift it so roles match the
    // original's bright, evenly-lit look.
    vec3 ambient = max(perFrameUbo.ambient.rgb, vec3(0.55));

    // Pick the two nearest enabled lights, matching the original engine's
    // 1-2 light skin shaders. The full table is at most 16 entries.
    int best0 = -1, best1 = -1;
    float d0 = 1e30, d1 = 1e30;
    for (int i = 0; i < count; i++) {
        float dist = distance(perFrameUbo.lightPos[i].xyz, fragWorldPos);
        if (dist < d0) {
            d1 = d0; best1 = best0;
            d0 = dist; best0 = i;
        } else if (dist < d1) {
            d1 = dist; best1 = i;
        }
    }

    vec3 lit = ambient;
    int idx[2] = int[2](best0, best1);
    for (int k = 0; k < 2; k++) {
        int i = idx[k];
        if (i < 0) continue;
        vec3 d = perFrameUbo.lightPos[i].xyz - fragWorldPos;
        float dist = length(d);
        vec3 L = dist > 0.0 ? d / dist : vec3(0.0, 1.0, 0.0);

        // PAL3 omni lights ship FLT_MAX range (no attenuation); treat any very
        // large outer radius as infinite, else use the D3D point-light cutoff.
        float outer = perFrameUbo.lightPos[i].w;
        float inner = perFrameUbo.lightColor[i].w;
        float atten = 1.0;
        if (outer < 1.0e18) {
            float edge0 = max(inner, outer * 0.85);
            atten = 1.0 - smoothstep(edge0, outer, dist);
        }

        lit += perFrameUbo.lightColor[i].rgb * max(dot(N, L), 0.0) * atten;
    }

    vec3 rgb = sampled.rgb * lit * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, sampled.a * mat.tint.a);

    // Linear distance fog, matching the shared shaders' eye-space fog.
    if (perFrameUbo.fogParams.x > 0.5 && mat.misc.w < 0.5) {
        float dist = -(vec4(fragWorldPos, 1.0) * perFrameUbo.view).z;
        float fStart = perFrameUbo.fogParams.y;
        float fEnd = perFrameUbo.fogParams.z;
        float vis = clamp((fEnd - dist) / max(fEnd - fStart, 1e-4), 0.0, 1.0);
        outColor.rgb = mix(perFrameUbo.fogColor.rgb * outColor.a, outColor.rgb, vis);
    }
}
