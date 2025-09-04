// Vertex shader
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    return out;
}

// Fragment shader
@group(0) @binding(0)
var image_texture: texture_2d<u32>;
@group(0) @binding(1)
var image_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the 16-bit texture
    let raw_value = textureLoad(image_texture, vec2<i32>(in.tex_coords * vec2<f32>(textureDimensions(image_texture))), 0);
    
    // Extract 16-bit value from the red channel
    let value_16bit = f32(raw_value.r);
    
    // Convert from 16-bit (0-65535) to normalized float (0.0-1.0)
    // Apply gamma correction for better visualization
    let normalized = value_16bit / 65535.0;
    let gamma_corrected = pow(normalized, 1.0 / 2.2);
    
    // Apply a simple stretch to enhance contrast
    // You can adjust these values for better visualization
    let min_val = 0.0;
    let max_val = 1.0;
    let stretched = clamp((gamma_corrected - min_val) / (max_val - min_val), 0.0, 1.0);
    
    // Output as grayscale RGB
    return vec4<f32>(stretched, stretched, stretched, 1.0);
}