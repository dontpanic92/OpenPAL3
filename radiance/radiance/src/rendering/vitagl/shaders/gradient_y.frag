// vitagl gradient_y: the vitagl backend does not yet route per-material
// `MaterialParams` to the shader, so we fall back to a static built-in
// range and color pair. The PAL4 floor/wall debug visualization is
// primarily a desktop (Vulkan) feature; this shader exists so the
// crate keeps compiling for the Vita target and the same `GradientY`
// shader program enum has a fragment binary on both backends.
uniform sampler2D texSampler: TEXUNIT0;

static const float Y_MIN = -200.0;
static const float Y_MAX =  200.0;
static const float3 LOW  = float3(0.0, 0.0, 1.0);
static const float3 HIGH = float3(1.0, 0.0, 0.0);

float4 main(
	float worldY : TEXCOORD0
) {
	float t = saturate((worldY - Y_MIN) / max(Y_MAX - Y_MIN, 1e-6));
	return float4(lerp(LOW, HIGH, t), 1.0);
}
