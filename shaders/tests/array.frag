#version 450

// Test shader for reflection: arrays in uniform blocks are not supported.

layout(set = 0, binding = 2) uniform Arrays {
    vec4 colors[4];
} arrays;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = arrays.colors[0];
}
