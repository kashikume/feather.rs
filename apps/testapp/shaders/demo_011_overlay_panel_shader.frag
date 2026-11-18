#version 450

// A flat, possibly see-through colour behind text and images.

layout(binding = 1) uniform Panel {
    // Colour and opacity, set on the panel's node.
    vec4 color;
} panel;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = panel.color;
}
