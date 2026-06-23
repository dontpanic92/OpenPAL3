#version 450
#extension GL_ARB_separate_shader_objects : enable

// PAL5 terrain splat — vertex stage. Mirrors `actor_lit.vert`'s world/normal
// setup but forwards the per-block weight-atlas UV (vertex texcoord) and the
// world position (the fragment stage derives terrain-texture tiling UV from
// it).
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

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 inTexCoord;   // per-block weight-atlas UV [0,1]

layout(location = 0) out vec2 fragAtlasUV;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out vec3 fragNormal;

mat4 clip = mat4(vec4(1.0, 0.0, 0.0, 0.0),
                 vec4(0.0, -1.0, 0.0, 0.0),
                 vec4(0.0, 0.0, 0.5, 0.5),
                 vec4(0.0, 0.0, 0, 1.0));

void main() {
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;
    gl_Position = world * perFrameUbo.view * perFrameUbo.proj * clip;

    fragAtlasUV = inTexCoord;
    fragWorldPos = world.xyz;
    fragNormal = normalize(normal * mat3(perInstanceUbo.model));
}
