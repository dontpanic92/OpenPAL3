#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(set = 2, binding = 0) uniform sampler2D texSampler;
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref
    vec4 uv_xform;   // reserved
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 0) out vec4 outColor;

void main() {
    vec4 sampled = texture(texSampler, fragTexCoord);
    if (sampled.a < mat.misc.x) {
        discard;
    }
    outColor = vec4(sampled.rgb * mat.tint.rgb, 1.0);
}
