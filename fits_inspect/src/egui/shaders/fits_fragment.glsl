precision highp float;

in vec2 UV;
out vec4 color;

uniform float clip_low;
uniform float clip_high;

uniform float histogram_low;
uniform float histogram_high;
uniform float histogram_mtf;

uniform usampler2D mono_fits;

void main() {
    uint intensity = texture( mono_fits, vec2(UV.x, UV.y) ).r;

    float x = intensity;
    x = x / 65535.0;

    float h_low = histogram_low;
    float h_high = histogram_high;
    float h_mtf = histogram_mtf;


    if (x >= clip_high) {
        color.r = 1.0f;
        color.g = 0.9f;
        color.b = 0.9f;
    } else if (x <= clip_low) {
        color.r = 0.0f;
        color.g = 0.2f;
        color.b = 0.0f;
    } else 
    {
        x = (x - h_low) / (h_high - h_low);
        x =            ((h_mtf - 1.0)* x) /
            ((2*h_mtf - 1.0) * x - h_mtf);

        color = vec4(x, x, x, 1.0);
    }
}