#version 450

// Demo 9's textured spider, with real shadows instead of the flat disc under the body. The
// texture's alpha is still used, and the light is darkened where the shadow map says something
// nearer the light is in the way. 3x3 samples soften the edges.

layout(binding = 1) uniform sampler2D atlas;

// The scene's shadow map (bound by the engine, see `shadow_map_sampler` in the scene file).
layout(binding = 2) uniform sampler2D shadowMap;

// The rectangle of the material's texture in the atlas, filled by the engine.
layout(binding = 3) uniform TextureRect {
    float umin;
    float umax;
    float vmin;
    float vmax;
} rect;

layout(binding = 4) uniform Lighting {
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
layout(location = 4) in vec4 fragLightSpace;

layout(location = 0) out vec4 outColor;

const float AMBIENT = 0.3;

// 1 where the light reaches the fragment, 0 in shadow, in between on soft edges.
float sunlight(vec3 normal, vec3 lightDir) {
    vec3 p = fragLightSpace.xyz / fragLightSpace.w;
    if (p.z >= 1.0) {
        return 1.0;
    }
    vec2 uv = p.xy * 0.5 + 0.5;
    // Surfaces at a grazing angle to the light need more bias against shadow acne.
    float bias = 0.0008 + 0.002 * (1.0 - max(dot(normal, lightDir), 0.0));
    vec2 texel = 1.0 / vec2(textureSize(shadowMap, 0));
    float lit = 0.0;
    for (int x = -1; x <= 1; x++) {
        for (int y = -1; y <= 1; y++) {
            float nearest = texture(shadowMap, uv + vec2(x, y) * texel).r;
            lit += p.z - bias <= nearest ? 1.0 : 0.0;
        }
    }
    return lit / 9.0;
}

void main() {
    vec2 uv = mix(vec2(rect.umin, rect.vmin), vec2(rect.umax, rect.vmax), fragTexCoord);
    vec4 color = texture(atlas, uv);

    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    float diffuse = max(dot(normal, lightDir), 0.0);
    float sun = diffuse > 0.0 ? sunlight(normal, lightDir) : 0.0;
    vec3 halfway = normalize(lightDir + toCamera);
    float specular = pow(max(dot(normal, halfway), 0.0), lighting.shininess)
        * lighting.specularStrength * sun;

    outColor = vec4(
        color.rgb * (AMBIENT + (1.0 - AMBIENT) * diffuse * sun) + vec3(specular), color.a);
}
