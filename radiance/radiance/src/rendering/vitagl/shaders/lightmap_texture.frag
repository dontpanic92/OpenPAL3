uniform sampler2D texSampler: TEXUNIT0;
uniform sampler2D texSampler2: TEXUNIT1;

float4 main(
	float2 texcoord : TEXCOORD0,
	float2 texcoord2 : TEXCOORD1
) {
	float4 lightMap = tex2D(texSampler, texcoord);
	float4 color = tex2D(texSampler2, texcoord2);
	if (color.a < 0.9) {
        discard;
    }

    float4 outColor = (lightMap * 1.5 + 0.15) * color;
	return outColor;
}
