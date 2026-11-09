#version 450

// Blinn-Phong lighting: ambient + diffuse + specular, from one directional light.

layout(binding = 1) uniform sampler2D texSampler;

layout(binding = 2) uniform Light {
    // Direction towards the light, in world space (need not be normalized). Set on the scene's
    // root node, so every node inherits it.
    vec3 light;
    // From the material: how sharp the highlight is (higher = smaller, sharper) ...
    float shininess;
    // ... and how bright it is (0 = no highlight).
    float specularStrength;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec2 fragTexCoord;
layout(location = 2) in vec3 fragPosition;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

// Light that reaches surfaces facing away from the light.
const float AMBIENT = 0.08;
// Brightness of a surface facing the light; above 1 to make the light stronger.
const float DIFFUSE = 1.3;

void main() {
    vec3 normal = normalize(fragNormal);
    vec3 light = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);

    float diffuse = max(dot(normal, light), 0.0);

    // Highlight where the normal is close to halfway between the light and the camera; none on
    // surfaces facing away from the light.
    vec3 halfway = normalize(light + toCamera);
    float specular = diffuse > 0.0
        ? pow(max(dot(normal, halfway), 0.0), lighting.shininess) * lighting.specularStrength
        : 0.0;

    vec4 color = texture(texSampler, fragTexCoord);
    vec3 lit = color.rgb * (AMBIENT + DIFFUSE * diffuse) + vec3(specular);
    outColor = vec4(lit, color.a);
}
