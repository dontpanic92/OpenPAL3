#version 450
#extension GL_ARB_separate_shader_objects : enable

// PAL5 grass-wind — vertex stage.
//
// The PAL5 `.ctr` grass is authored clump geometry textured (world-planar)
// with the real `cao###` ground-grass texture. This stage transforms the
// vertex and sways it horizontally over time so the grass reads as wind-blown.
//
// The sway is weighted by a per-vertex **height weight** carried in the second
// texcoord set (`inTexCoord2.x`: 0 at the clump root / ground, 1 at the tip),
// so roots stay pinned and tips move — blades bend rather than translate.
// Per-clump phase comes from world XZ, giving a travelling wave across the
// field instead of a lockstep wobble.
//
// Wind tunables ride the per-material UBO's `uv_xform.xy` (the grass material
// never animates its texture UVs, so the lane is free):
//   uv_xform.x = strength  (world units of max tip displacement)
//   uv_xform.y = speed     (radians/sec of the primary oscillation)
//
// The primary texcoord (`inTexCoord`) is the world-planar colour UV and is
// forwarded to the fragment stage untransformed.
//
// The full per-frame UBO prefix is declared so `timeParams` (appended after
// `fogParams`) lands at the correct std140 offset.
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
    vec4 fogColor;
    vec4 fogParams;
    vec4 timeParams;         // x = elapsed seconds
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;
    vec4 uv_xform;           // x = wind strength, y = wind speed
} mat;

layout(location = 0) in vec3 position;
layout(location = 2) in vec2 inTexCoord;    // world-planar colour UV
layout(location = 3) in vec2 inTexCoord2;   // x = wind weight (0 root..1 tip), y = per-cell coverage

layout(location = 0) out vec2 fragTexCoord;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out float fragCoverage;

mat4 clip = mat4(vec4(1.0, 0.0, 0.0, 0.0),
                 vec4(0.0, -1.0, 0.0, 0.0),
                 vec4(0.0, 0.0, 0.5, 0.5),
                 vec4(0.0, 0.0, 0, 1.0));

void main() {
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;

    float tipWeight = clamp(inTexCoord2.x, 0.0, 1.0);
    float strength = mat.uv_xform.x;
    float speed = mat.uv_xform.y;
    float t = perFrameUbo.timeParams.x;

    // Travelling wave: phase shifts with world position so clumps ripple. A
    // second, faster harmonic adds a subtle flutter.
    float phase = t * speed + (world.x + world.z) * 0.012;
    float sway = (sin(phase) + 0.3 * sin(phase * 2.7)) * strength * tipWeight;

    // Bend along a fixed diagonal wind direction (mostly +X, some +Z).
    world.x += sway;
    world.z += sway * 0.5;

    gl_Position = world * perFrameUbo.view * perFrameUbo.proj * clip;
    fragWorldPos = world.xyz;
    fragTexCoord = inTexCoord;
    fragCoverage = inTexCoord2.y;
}
