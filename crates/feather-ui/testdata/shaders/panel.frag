#version 450

// A flat colour for the unit tests' backgrounds, buttons and scrollbars.

layout(binding = 1) uniform Panel {
    vec4 color;
} panel;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = panel.color;
}
