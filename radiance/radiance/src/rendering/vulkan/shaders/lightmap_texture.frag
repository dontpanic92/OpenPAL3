#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(set = 2, binding = 0) uniform sampler2D texSampler[2];
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref
    vec4 uv_xform;   // reserved
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec2 fragTexCoord2;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 lightMap = texture(texSampler[0], fragTexCoord);
    vec4 color = texture(texSampler[1], fragTexCoord2);
    if (color.a < mat.misc.x) {
        discard;
    }

    outColor = (lightMap * 1.5 + 0.15) * color * vec4(mat.tint.rgb, 1.0);
}
