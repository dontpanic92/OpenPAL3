#version 450
#extension GL_ARB_separate_shader_objects : enable

// CVD Gouraud lighting: newgeom/geom_t without lights, geom_[t_]1L/2L
// with lights. Native light positions are transformed into model space.
layout(set = 0, binding = 0) uniform PerFrameUbo {
    mat4 view;
    mat4 proj;
    vec4 ambient;            // rgb = ambient color, w = light count
    vec4 lightPos[16];       // xyz = world position, w = outer range
    vec4 lightColor[16];     // rgb = color, w = inner range
} perFrameUbo;

layout(set = 1, binding = 0) uniform PerInstanceUbo {
    mat4 model;
} perInstanceUbo;

layout(set = 3, binding = 0) uniform MaterialParams {
    vec4 tint;
    vec4 misc;
    vec4 uv_xform;
    vec4 ambient;
    vec4 emissive;
    vec4 light_positions[2];
    vec4 light_colors[2];
    vec4 light_ambient[2];
    vec4 light_attenuation[2];
} mat;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec2 fragTexCoord;
layout(location = 1) out vec3 fragWorldPos;
layout(location = 2) out vec3 fragColor;

mat4 clip = mat4(vec4(1.0, 0.0, 0.0, 0.0),
                 vec4(0.0, -1.0, 0.0, 0.0),
                 vec4(0.0, 0.0, 0.5, 0.5),
                 vec4(0.0, 0.0, 0, 1.0));

void main() {
    vec4 world = vec4(position, 1.0) * perInstanceUbo.model;
    gl_Position = world * perFrameUbo.view * perFrameUbo.proj * clip;

    fragWorldPos = world.xyz;
    fragTexCoord = inTexCoord * mat.uv_xform.xy + mat.uv_xform.zw;

    vec3 base = perFrameUbo.ambient.rgb * mat.ambient.rgb
              + mat.emissive.rgb * mat.emissive.w;
    vec3 color = vec3(0.0);
    bool lit = false;
    mat4 inverseModel = inverse(perInstanceUbo.model);
    for (int i = 0; i < 2; i++) {
        if (mat.light_positions[i].w < 0.5) continue;
        lit = true;
        vec3 localLight = (vec4(mat.light_positions[i].xyz, 1.0) * inverseModel).xyz;
        vec3 delta = localLight - position;
        float distanceToLight = length(delta);
        float lambert = distanceToLight > 0.0
            ? max(dot(normal, delta / distanceToLight), 0.0) : 0.0;
        // Each selected light supplies ambient as well as diffuse. The native
        // two-light effect adds both complete contributions, including emission.
        vec3 coefficients = mat.light_attenuation[i].xyz;
        float attenuation = 1.0 / (coefficients.x + distanceToLight *
                                  (coefficients.y + distanceToLight * coefficients.z));
        color += base + mat.light_ambient[i].rgb * mat.ambient.rgb
                      + attenuation * lambert * mat.light_colors[i].rgb * mat.tint.rgb;
    }
    fragColor = clamp(lit ? color : base, 0.0, 1.0);
}
