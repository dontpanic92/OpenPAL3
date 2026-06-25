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

// Per-frame UBO prefix (shared backing buffer with the lit shaders); declared
// through `fogParams` so the trailing fog fields land at the std140 offset
// matching `PerFrameUniformBuffer`.
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
    vec4 fogColor;           // rgb = linear fog color
    vec4 fogParams;          // x = enabled, y = start depth, z = end depth
} perFrameUbo;

layout(set = 2, binding = 0) uniform sampler2D texSampler[2];
layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;       // x = alpha_ref (only consulted when ALPHA_TEST is true)
                     // y = lightmap intensity (`_ltMap.cfg`)
                     // z = ambient floor (additive term; 0.3 default, 0 for PAL3)
                     // w = fog_exempt
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
layout(location = 2) in vec3 fragWorldPos;

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
    // The lightmap is remapped as `lightMap * 1.5 * intensity + 0.3` —
    // i.e. the baked-light contribution (`lightMap * 1.5`) is scaled by
    // the per-scene `_ltMap.cfg` `intensity` (UBO `misc.y`), but the
    // ambient floor (`+ 0.3`) is *not*. The shipped formula doc-comment
    // (`pal4/ltmap.rs`) writes the canonical form as
    // `(lightMap * 1.5 + 0.15) * intensity`, but in practice the
    // PAL4 corpus ships intensities in the low end (most blocks are
    // `[0.05, 0.5]`, Q04 caves `[0.04, 0.51]`, m01/2 = `0.04`), and
    // multiplying the ambient floor by such a small `intensity`
    // collapses the visible-but-dim ambient down to a pure-black
    // baseline. Pulling the floor outside the multiply keeps the
    // intensity term doing what its name promises — dimming the *light*
    // — while guaranteeing every surface retains a visible diffuse
    // contribution. The floor itself is `0.3` (vs the canonical `0.15`)
    // for the cave/wall dark-corner case from M01/1; the gain (`1.5`)
    // is unchanged.
    //
    // For non-PAL4 callers `mat.misc.y` is the `MaterialParams::default`
    // `intensity = 1.0` (set in `radiance/.../material.rs`), so this
    // shader degenerates to `lightMap * 1.5 + 0.3` as before for any
    // path that doesn't stamp a per-scene `_ltMap.cfg` intensity.
    // Baked-lightmap modulation: `lightMap * 1.5 * intensity + ambient_floor`.
    // The gain (`1.5`) and the per-scene `intensity` (`misc.y`) scale the baked
    // contribution; the additive ambient floor is now `misc.z` (previously a
    // hard-coded `0.3`). PAL4 keeps the `0.3` floor via the `MaterialParams`
    // default, which is what its dark caves were tuned against. PAL3 sets the
    // floor to `0.0` so its baked lightmaps keep their dark, high-contrast
    // shadows instead of being lifted and desaturated toward grey.
    vec3 lm = lightMap.rgb * 1.5 * mat.misc.y + mat.misc.z;
    vec3 rgb = lm * color.rgb * mat.tint.rgb * mat.tint.a;
    outColor = vec4(rgb, color.a * mat.tint.a);

    // Linear distance fog (gated). Inert for PAL3/PAL4 (no scene fog); present
    // so any future fogged lightmap scene fades correctly. Premultiplied blend.
    if (perFrameUbo.fogParams.x > 0.5 && mat.misc.w < 0.5) {
        float d = -(vec4(fragWorldPos, 1.0) * perFrameUbo.view).z;
        float fStart = perFrameUbo.fogParams.y;
        float fEnd = perFrameUbo.fogParams.z;
        float vis = clamp((fEnd - d) / max(fEnd - fStart, 1e-4), 0.0, 1.0);
        outColor.rgb = mix(perFrameUbo.fogColor.rgb * outColor.a, outColor.rgb, vis);
    }
}
