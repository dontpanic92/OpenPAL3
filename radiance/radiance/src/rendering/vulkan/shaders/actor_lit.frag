#version 450
#extension GL_ARB_separate_shader_objects : enable

// Dynamically-lit actor shader: per-pixel Lambert diffuse summed over the
// scene's omni point lights plus a flat ambient term. Used by PAL3 roles,
// whose MV3 meshes ship per-frame vertex normals. Static scenery keeps its
// own (unlit / lightmap / baked vertex-color) path.

layout(constant_id = 0) const bool ALPHA_TEST = true;

layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;            // rgb = ambient color, w = light count
    vec4 lightPos[16];       // xyz = world position
    vec4 lightColor[16];     // rgb = color
    vec4 sunDir;             // xyz = dir toward sun, w = enabled (1/0)
    vec4 sunColor;           // rgb = sun color
} perFrameUbo;

layout(set = 2, binding = 0) uniform sampler2D texSampler;
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;               // x = alpha_ref (only when ALPHA_TEST)
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
    vec3 lit = perFrameUbo.ambient.rgb;

    int count = int(perFrameUbo.ambient.w);
    for (int i = 0; i < count; i++) {
        vec3 d = perFrameUbo.lightPos[i].xyz - fragWorldPos;
        float dist = length(d);
        vec3 L = dist > 0.0 ? d / dist : vec3(0.0, 1.0, 0.0);
        float ndl = max(dot(N, L), 0.2);

        // Distance attenuation. The `w` lanes carry the light's [inner, outer]
        // radii (outer in lightPos.w, inner in lightColor.w). PAL3 omni lights
        // ship FLT_MAX radii (≈3.4e38) meaning "no attenuation"; treat any very
        // large outer radius as infinite.
        //
        // PAL3's interior key lights (e.g. the YN09 candle, range 30→200)
        // behave like Direct3D point lights: roughly *constant* within their
        // range with a cutoff at the far edge — NOT a gradual dimming across
        // the whole radius. So the light stays at full strength out to
        // `inner` (and most of the way to `outer`), then falls off only over
        // the outer shell. A whole-range ramp here left actors that sit well
        // inside the candle's reach far too dark vs. the original.
        float outer = perFrameUbo.lightPos[i].w;
        float inner = perFrameUbo.lightColor[i].w;
        float atten = 1.0;
        if (outer < 1.0e18) {
            // Fully lit until `edge0`, smooth cutoff from there to `outer`.
            float edge0 = max(inner, outer * 0.85);
            atten = 1.0 - smoothstep(edge0, outer, dist);
        }

        lit += perFrameUbo.lightColor[i].rgb * ndl * atten;
    }

    // Directional sun: parallel light, no attenuation. `sunDir.w` is the
    // enabled flag. Uses the same 0.2 wrap floor as the point lights so
    // back-facing surfaces still pick up a little fill rather than going
    // fully black.
    if (perFrameUbo.sunDir.w > 0.5) {
        vec3 Ls = normalize(perFrameUbo.sunDir.xyz);
        float ndlS = max(dot(N, Ls), 0.2);
        lit += perFrameUbo.sunColor.rgb * ndlS;
    }

    // Premultiplied-alpha invariant matches `simple_triangle.frag`: scale RGB
    // and alpha by tint.a; the lighting term replaces the flat white the unlit
    // shader would otherwise use.
    vec3 rgb = sampled.rgb * lit * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, sampled.a * mat.tint.a);
}
