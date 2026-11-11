#version 450

// A flat, possibly see-through colour: the panel behind the text, and the buttons.

layout(binding = 1) uniform Panel {
    // Colour and opacity, set on the material.
    vec4 color;
} panel;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = panel.color;
}
