#version 450

// A planet or a moon lit by a strong directional light (a sun): a bright day side, a dark night
// side, a highlight that only water reflects, and an atmosphere glowing at the lit edge.

layout(binding = 1) uniform sampler2D texSampler;

layout(binding = 2) uniform Lighting {
    // Direction towards the light, in world space (need not be normalized), and its strength.
    // Set on the scene's root node, so every node inherits them.
    vec3 light;
    float lightIntensity;
    // From the material: how sharp the highlight is (higher = smaller, sharper) ...
    float shininess;
    // ... and how bright it is (0 = no highlight).
    float specularStrength;
    // From the material: how bright the atmosphere glows (0 = no atmosphere).
    float atmosphereStrength;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec2 fragTexCoord;
layout(location = 2) in vec3 fragPosition;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

// Light reaching the night side.
const float AMBIENT = 0.02;
const vec3 ATMOSPHERE = vec3(0.35, 0.6, 1.0);

void main() {
    vec3 normal = normalize(fragNormal);
    vec3 light = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    vec4 color = texture(texSampler, fragTexCoord);

    // A soft day/night edge instead of a hard cut-off at the terminator.
    float facing = dot(normal, light);
    float diffuse = smoothstep(-0.05, 0.25, facing) * max(facing, 0.0) * 0.6
        + smoothstep(-0.05, 0.25, facing) * 0.4;
    diffuse *= lighting.lightIntensity;

    // Water is blue where the map has more blue than red and green; only water shines.
    float water = clamp((color.b - max(color.r, color.g)) * 4.0, 0.0, 1.0);
    vec3 halfway = normalize(light + toCamera);
    float specular = facing > 0.0
        ? pow(max(dot(normal, halfway), 0.0), lighting.shininess) * lighting.specularStrength
        : 0.0;
    specular *= water * lighting.lightIntensity;

    // Atmosphere: brighter towards the edge of the disc, and only on the day side.
    float rim = pow(1.0 - max(dot(normal, toCamera), 0.0), 3.0);
    float day = smoothstep(-0.2, 0.4, facing);
    vec3 atmosphere = ATMOSPHERE * rim * day * lighting.atmosphereStrength * lighting.lightIntensity;

    vec3 lit = color.rgb * (AMBIENT + diffuse) + vec3(1.0, 0.95, 0.85) * specular + atmosphere;
    outColor = vec4(lit, 1.0);
}
