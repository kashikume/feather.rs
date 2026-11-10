#version 450

// A rectangle of the atlas (the material's texture), with Blinn-Phong lighting from one
// directional light. The texture's alpha makes see-through parts (the shadow).

layout(binding = 1) uniform sampler2D atlas;

// The rectangle of the material's texture in the atlas, filled by the engine.
layout(binding = 2) uniform TextureRect {
    float umin;
    float umax;
    float vmin;
    float vmax;
} rect;

layout(binding = 3) uniform Lighting {
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

const float AMBIENT = 0.3;

void main() {
    vec2 uv = mix(vec2(rect.umin, rect.vmin), vec2(rect.umax, rect.vmax), fragTexCoord);
    vec4 color = texture(atlas, uv);

    vec3 normal = normalize(fragNormal);
    vec3 light = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    float diffuse = max(dot(normal, light), 0.0);
    vec3 halfway = normalize(light + toCamera);
    float specular = diffuse > 0.0
        ? pow(max(dot(normal, halfway), 0.0), lighting.shininess) * lighting.specularStrength
        : 0.0;

    outColor = vec4(color.rgb * (AMBIENT + (1.0 - AMBIENT) * diffuse) + vec3(specular), color.a);
}
