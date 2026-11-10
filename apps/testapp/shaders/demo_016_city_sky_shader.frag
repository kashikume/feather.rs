#version 450

// The sky's colour by direction: deep blue up, pale at the horizon, the sun and its glow.
// The city shader's fog uses the same colours, so far buildings fade into it.

layout(binding = 1) uniform Sky {
    vec3 sunDirection;
    vec3 sunColor;
    vec3 skyColor;
    vec3 horizonColor;
    vec3 groundColor;
} sky;

layout(location = 0) in vec3 fragDirection;

layout(location = 0) out vec4 outColor;

void main() {
    vec3 d = normalize(fragDirection);
    float h = d.z;
    vec3 c = h > 0.0
        ? mix(sky.horizonColor, sky.skyColor, pow(h, 0.45))
        : mix(sky.horizonColor, sky.groundColor, pow(min(-h * 4.0, 1.0), 0.7));
    float sun = max(dot(d, normalize(sky.sunDirection)), 0.0);
    c += sky.sunColor * (pow(sun, 600.0) * 8.0 + pow(sun, 12.0) * 0.18);
    c = vec3(1.0) - exp(-c * 1.35);
    outColor = vec4(c, 1.0);
}
