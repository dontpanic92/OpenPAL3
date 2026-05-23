uniform float4x4 modelMatrix;
uniform float4x4 viewMatrix;
uniform float4x4 projectionMatrix;

void main(
	float3 position,
	float2 texcoord,
	float out out_worldY : TEXCOORD0,
	float4 out gl_Position : POSITION
) {
	float4 world = mul(modelMatrix, float4(position, 1.0f));
	float4 mvPosition = mul(viewMatrix, world);
	gl_Position = mul(projectionMatrix, mvPosition);
	out_worldY = world.y;
}
