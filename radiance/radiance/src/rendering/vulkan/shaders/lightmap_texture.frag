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
                     // y = lightmap intensity (`_ltMap.cfg`)
    vec4 uv_xform;   // primary-UV xform (applied in vert)
} mat;

// UV channel assignment (matches `lightmap_texture.vert`):
//
//   fragTexCoord  ← primary UV (set 0)  → diffuse atlas tiling
//   fragTexCoord2 ← secondary UV (set 1) → lightmap atlas
//
// Texture binding (`LightMapMaterialDef::create_with_samplers` builds
// the vector as `[lightmap, diffuse]`):
//
//   texSampler[0] = lightmap atlas (must be sampled with fragTexCoord2)
//   texSampler[1] = diffuse texture (must be sampled with fragTexCoord)
//
// Sampling the lightmap with the diffuse (primary) UV — as a previous
// revision did — produced the well-known "black tile" artefact in PAL4
// BSPs, because the tiled diffuse UV indexes the atlas outside the
// charted regions.
layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec2 fragTexCoord2;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 color    = texture(texSampler[1], fragTexCoord);
    vec4 lightMap = texture(texSampler[0], fragTexCoord2);
    if (ALPHA_TEST && color.a < mat.misc.x) {
        discard;
    }

    // `color.rgb` is premultiplied when the diffuse has transparency; the
    // lightmap factor and tint must therefore also be multiplied by
    // `mat.tint.a` to preserve the premultiplied invariant of the output.
    //
    // The lightmap itself is remapped as `lightMap * 1.5 + 0.15` to match
    // the shipped PAL4 renderer (see `pal4/ltmap.rs` doc-comment): this
    // gives a 0.15 ambient floor so dark-corner lightmap samples don't
    // collapse to pure black, and a 1.5 gain so well-lit samples reach
    // full brightness. Without the `+ 0.15` floor, baked shadows
    // produced a hard black-to-color gradient across faces.
    // The lightmap itself is remapped as `lightMap * 1.5 + 0.3` to match
    // the shipped PAL4 renderer's ambient look (see `pal4/ltmap.rs`
    // doc-comment for the canonical formula). The shipped formula uses
    // a `+ 0.15` floor; we use `+ 0.3` because the canonical 0.15 leaves
    // many baked-dark or under-baked atlas texels producing pure-black
    // faces in cave/wall geometry (e.g. M01/1). The 1.5 gain so well-lit
    // samples reach full brightness remains unchanged. Without the
    // higher floor, baked shadows produced a hard black-to-color
    // gradient across affected faces.
    vec3 lm = lightMap.rgb * 1.5 + 0.3;
    vec3 rgb = lm * mat.misc.y * color.rgb * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, color.a * mat.tint.a);
}
