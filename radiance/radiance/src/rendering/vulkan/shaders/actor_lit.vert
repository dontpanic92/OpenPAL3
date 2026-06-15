#version 450
#extension GL_ARB_separate_shader_objects : enable

// Per-frame UBO. Mirrors `actor_lit.frag` and the GPU-side
// `PerFrameUniformBuffer`: `view`/`proj` plus the scene lighting environment
// (`ambient.w` = active light count). Existing unlit shaders declare only the
// `view`/`proj` prefix of this block, which is layout-compatible (the extra
// trailing members live in the same backing buffer).
layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;            // rgb = ambient color, w = light count
    vec4 lightPos[16];       // xyz = world position
    vec4 lightColor[16];     // rgb = color
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;
    vec4 uv_xform;
} mat;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec2 fragTexCoord;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out vec3 fragNormal;

mat4 clip = mat4(vec4(1.0, 0.0, 0.0, 0.0),
                 vec4(0.0, -1.0, 0.0, 0.0),
                 vec4(0.0, 0.0, 0.5, 0.5),
                 vec4(0.0, 0.0, 0, 1.0));

void main() {
    // Radiance uses the row-vector convention (`v * M`), so the world
    // position is `position * model` and direction vectors transform by the
    // upper-left 3x3 as `n * mat3(model)`.
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;
    gl_Position = world * perFrameUbo.view * perFrameUbo.proj * clip;

    fragWorldPos = world.xyz;
    fragNormal = normalize(normal * mat3(perInstanceUbo.model));
    fragTexCoord = inTexCoord * mat.uv_xform.xy + mat.uv_xform.zw;
}
