uniform sampler2D texSampler: TEXUNIT0;

float4 main(
	float2 texcoord : TEXCOORD0
) {
	float4 color = tex2D(texSampler, texcoord);
	if (color.a < 0.4) {
        discard;
    } else {
		color.a = 1.0;
	}

	return color;
}
