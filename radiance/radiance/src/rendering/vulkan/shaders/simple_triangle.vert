#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

// Mirror of the per-material UBO from `simple_triangle.frag`. The vertex
// shader consumes `uv_xform` to apply a per-frame UV affine
// (`xy = scale`, `zw = offset`) so PAL4 water materials can animate their
// surface UVs without per-vertex data. Default `MaterialParams` values
// (`uv_scale = (1,1)`, `uv_offset = (0,0)`) make this an identity
// transform — every existing non-animated material is bit-identical.
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;
    vec4 uv_xform;
} mat;

layout(location = 0) in vec3 position;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec2 fragTexCoord;

mat4 clip = mat4(vec4(1.0, 0.0, 0.0, 0.0),
                 vec4(0.0, -1.0, 0.0, 0.0),
                 vec4(0.0, 0.0, 0.5, 0.5),
                 vec4(0.0, 0.0, 0, 1.0));

void main() {
    gl_Position = vec4(position, 1.0) * perInstanceUbo.model * perFrameUbo.view * perFrameUbo.proj * clip;
    fragTexCoord = inTexCoord * mat.uv_xform.xy + mat.uv_xform.zw;
}
