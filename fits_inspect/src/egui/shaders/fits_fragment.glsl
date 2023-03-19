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
    uint intensity = texture( mono_fits, vec2(UV.x, 1.0-UV.y) ).r;

    float x = intensity;
    x = x / 65535.0;

    

    float h_low = histogram_low;
    float h_high = histogram_high;
    float h_mtf = histogram_mtf;

    // 0.5 into [0.25 -> .75] = 0.5
    // 0.5 - 0.25 = 0.25
    // 5  = 0.5

    if (x >= clip_high) {
            color.r = 0.5f;
            color.g = 0.25f;
            color.b = 0.25f;
    } else if (x <= clip_low) {
        color.r = 0.25f;
        color.g = 0.5f;
        color.b = 0.25f;
    } else {

            x = (x - h_low) / (h_high - h_low);
            x =            ((h_mtf - 1.0)* x) /
                ((2*h_mtf - 1.0) * x - h_mtf);

        color = vec4(x, x, x, 1.0);
    }
}