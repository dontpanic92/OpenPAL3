#version 450
#extension GL_ARB_separate_shader_objects : enable

// Per-vertex Y-gradient shader (PAL4 nav-mesh debug). Used by entities
// whose materials are `GradientYMaterialDef` — currently the PAL4
// `_floor.dff` / `_wall.dff` debug geometry. The fragment shader
// interpolates between two colors based on the world-space Y coord of
// each fragment.
layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

layout(location = 0) in vec3 position;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out float worldY;

mat4 clip = mat4(vec4(1.0, 0.0, 0.0, 0.0),
                 vec4(0.0, -1.0, 0.0, 0.0),
                 vec4(0.0, 0.0, 0.5, 0.5),
                 vec4(0.0, 0.0, 0, 1.0));

void main() {
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;
    gl_Position = world * perFrameUbo.view * perFrameUbo.proj * clip;
    worldY = world.y;
}
