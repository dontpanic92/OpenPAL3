#version 450
#extension GL_ARB_separate_shader_objects : enable

// Alpha convention: non-opaque diffuse textures are stored premultiplied
// (`texture::premultiply_alpha`). Lightmap textures sampled from
// `texSampler[0]` are bypassed by premultiplication because their alpha
// channel is conventionally a junk 255 and the classifier tags them as
// `AlphaKind::Opaque`. The cutout test runs on the diffuse texture's
// alpha (which `AlphaTest` materials always keep binary). See
// `simple_triangle.frag` for details on the specialization constant.
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

    // `color.rgb` is premultiplied when the diffuse has transparency; the
    // lightmap factor and tint must therefore also be multiplied by
    // `mat.tint.a` to preserve the premultiplied invariant of the output.
    vec3 rgb = (lightMap.rgb * 1.5 + 0.15) * color.rgb * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, color.a * mat.tint.a);
}
