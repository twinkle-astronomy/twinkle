 const vec2 verts[6] = vec2[6](
                        vec2(-1.0, 1.0),
                        vec2(1.0, 1.0),
                        vec2(1.0, -1.0),

                        vec2(-1.0, 1.0),
                        vec2(-1.0, -1.0),
                        vec2(1.0, -1.0)
                    );

out vec2 UV;
uniform float center_x;
uniform float center_y;

uniform float min_x;
uniform float min_y;

uniform float max_x;
uniform float max_y;

vec2 texture_verts[6] = vec2[6](
    vec2(min_x, max_y),
    vec2(max_x, max_y),
    vec2(max_x, min_y),

    vec2(min_x, max_y),
    vec2(min_x, min_y),
    vec2(max_x, min_y)
);

void main() {
    gl_Position = vec4(verts[gl_VertexID], 0.0,  1.0);
    UV = texture_verts[gl_VertexID];
}