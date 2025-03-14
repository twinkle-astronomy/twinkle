precision highp float;
precision highp int;
precision highp usampler2D;

in vec2 UV;
out vec4 color;

uniform float clip_low;
uniform float clip_high;

uniform float histogram_low;
uniform float histogram_high;
uniform float histogram_mtf;

uniform usampler2D mono_fits;

void main() {
    // Explicitly convert to float after reading the texture
    float intensity = float(texture(mono_fits, vec2(UV.x, UV.y)).r);
    
    float x = intensity;
    x = x / 65535.0;

    float h_low = histogram_low;
    float h_high = histogram_high;
    float h_mtf = histogram_mtf;

    if (x >= clip_high) {
        color.r = 1.0;
        color.g = 0.9;
        color.b = 0.9;
    } else if (x <= clip_low) {
        color.r = 0.0;
        color.g = 0.2;
        color.b = 0.0;
    } else {
        x = (x - h_low) / (h_high - h_low);
        x = ((h_mtf - 1.0) * x) / 
            ((2.0 * h_mtf - 1.0) * x - h_mtf);
            
        color = vec4(x, x, x, 1.0);
    }
}