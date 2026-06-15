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
        float ndl = max(dot(N, L), 0.0);
        lit += perFrameUbo.lightColor[i].rgb * ndl;
    }

    // Premultiplied-alpha invariant matches `simple_triangle.frag`: scale RGB
    // and alpha by tint.a; the lighting term replaces the flat white the unlit
    // shader would otherwise use.
    vec3 rgb = sampled.rgb * lit * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, sampled.a * mat.tint.a);
}
