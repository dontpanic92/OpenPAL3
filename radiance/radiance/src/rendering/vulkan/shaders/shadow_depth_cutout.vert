#version 450
#extension GL_ARB_separate_shader_objects : enable

// Alpha-tested variant of the directional shadow-map depth pass.
//
// Same as `shadow_depth.vert` but forwards the vertex TEXCOORD so the
// fragment stage can sample the caster's opacity and `discard` cutout
// texels. Used for cutout (`AlphaTest`) casters — tree-leaf cards, grass —
// so their shadows take the textured silhouette instead of a solid quad.
// POSITION stays at location 0 / offset 0 like every caster; TEXCOORD is at
// location 1 (the pipeline supplies its per-program byte offset).
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
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

layout(push_constant) uniform PushConsts {
    uint cascade;
} pc;

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 texcoord;

layout(location = 0) out vec2 fragTexCoord;

void main() {
    fragTexCoord = texcoord;
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;
    gl_Position = world * perFrameUbo.lightViewProj[pc.cascade];
}
