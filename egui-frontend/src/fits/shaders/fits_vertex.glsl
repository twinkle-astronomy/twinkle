
in vec2 vertex;
out vec2 UV;

uniform mat4 M;
uniform mat4 V;

void main() {
    gl_Position = V*M*vec4(vertex, 0.0,  1.0);
    UV = clamp(vertex, 0.0, 1.0);
}