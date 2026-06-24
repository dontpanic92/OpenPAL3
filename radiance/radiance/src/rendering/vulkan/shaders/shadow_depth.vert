#version 450
#extension GL_ARB_separate_shader_objects : enable

// Directional shadow-map depth pass.
//
// Writes only depth from the sun's point of view. Shares set 0 (the
// per-frame UBO) and set 1 (the per-instance model matrix) with the main
// scene pipelines so the same descriptor sets stay bound, and reads only the
// vertex POSITION attribute (location 0, offset 0) — every caster program
// keeps POSITION first, so a single attribute layout works for all of them
// (only the binding stride differs, which the pipeline supplies per program).
//
// The UBO block must mirror `PerFrameUniformBuffer` up to `lightViewProj` so
// std140 offsets line up; trailing members the depth pass never reads are
// still declared to keep that layout exact. `lightViewProj` already folds in
// the Vulkan clip (Y-flip + [0,1] depth) remap, so no separate `clip`
// multiply is needed here.
layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;
    vec4 lightPos[16];
    vec4 lightColor[16];
    vec4 sunDir;
    vec4 sunColor;
    mat4 lightViewProj;
    vec4 shadowParams;
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

layout(location = 0) in vec3 position;

void main() {
    // Row-vector convention (`v * M`), matching the scene vertex shaders.
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;
    gl_Position = world * perFrameUbo.lightViewProj;
}
