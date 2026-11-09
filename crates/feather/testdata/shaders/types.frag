#version 450

// Test shader for reflection, paired with types.vert.

layout(set = 0, binding = 0) uniform Shared {
    mat4 view;
    mat4 proj;
    float time;
} shared_;

layout(set = 1, binding = 1) uniform sampler2D albedo;

layout(push_constant) uniform Push {
    mat4 model;
    vec4 tint;
} pc;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = texture(albedo, vec2(shared_.time)) * pc.tint;
}
