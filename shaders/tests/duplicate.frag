#version 450

// Test shader for reflection: `view` is also a member of a block in shader.vert.

layout(set = 0, binding = 2) uniform Other {
    mat4 view;
} other;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = other.view[0];
}
