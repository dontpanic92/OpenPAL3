#version 450
#extension GL_ARB_separate_shader_objects : enable

// Alpha convention: textures are decoded as straight (non-premultiplied)
// RGBA. ALPHA_TEST is selected via specialization constant 0 at pipeline
// creation; see simple_triangle.frag for details.
layout(constant_id = 0) const bool ALPHA_TEST = true;

layout(set = 2, binding = 0) uniform sampler2D texSampler[2];
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref (only consulted when ALPHA_TEST is true)
    vec4 uv_xform;   // reserved
} mat;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec2 fragTexCoord2;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 lightMap = texture(texSampler[0], fragTexCoord);
    vec4 color = texture(texSampler[1], fragTexCoord2);
    if (ALPHA_TEST && color.a < mat.misc.x) {
        discard;
    }

    vec3 rgb = (lightMap.rgb * 1.5 + 0.15) * color.rgb * mat.tint.rgb;
    outColor = vec4(rgb, color.a * mat.tint.a);
}
