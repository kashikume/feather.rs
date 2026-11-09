#version 450

// A plain colour, no texture, with Blinn-Phong lighting from one directional light.

layout(binding = 1) uniform Lighting {
    // Direction towards the light, in world space (need not be normalized). Set on the scene's
    // root node, so every node inherits it.
    vec3 light;
    // The object's colour, set on its node.
    vec3 color;
    // From the material: how sharp the highlight is (higher = smaller, sharper) ...
    float shininess;
    // ... and how bright it is (0 = no highlight).
    float specularStrength;
    // From the material: 1 = solid, less shows what is behind (on transparent nodes only).
    float opacity;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

const float AMBIENT = 0.18;

void main() {
    vec3 normal = normalize(fragNormal);
    vec3 light = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);

    float diffuse = max(dot(normal, light), 0.0);
    vec3 halfway = normalize(light + toCamera);
    float specular = diffuse > 0.0
        ? pow(max(dot(normal, halfway), 0.0), lighting.shininess) * lighting.specularStrength
        : 0.0;

    outColor = vec4(lighting.color * (AMBIENT + diffuse) + vec3(specular), lighting.opacity);
}
