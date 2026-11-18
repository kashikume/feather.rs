#version 450

// Stars twinkling: their brightness wavers around its own level, each at its own phase.

layout(binding = 1) uniform Sky {
    // Seconds since the demo started, set on the `Stars` node.
    float time;
} sky;

layout(location = 0) in vec2 fragStar;

layout(location = 0) out vec4 outColor;

void main() {
    float twinkle = 0.75 + 0.25 * sin(sky.time * 2.7 + fragStar.y * 6.2832);
    float brightness = fragStar.x * twinkle;
    // Slightly blue.
    outColor = vec4(brightness * vec3(0.9, 0.95, 1.0), 1.0);
}
