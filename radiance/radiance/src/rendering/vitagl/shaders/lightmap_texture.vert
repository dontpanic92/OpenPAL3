uniform float4x4 modelMatrix;
uniform float4x4 viewMatrix;
uniform float4x4 projectionMatrix;

void main(
	float3 position,
	float2 texcoord,
    float2 texcoord2,
	float2 out out_texcoord : TEXCOORD0,
	float2 out out_texcoord2 : TEXCOORD1,
	float4 out gl_Position : POSITION
) {
	float4 mvPosition = mul(mul(float4(position, 1.0f), modelMatrix), viewMatrix);
	gl_Position = mul(mvPosition, projectionMatrix);
	out_texcoord = texcoord;
	out_texcoord2 = texcoord2;
}
