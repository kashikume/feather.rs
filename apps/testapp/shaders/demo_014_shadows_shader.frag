#version 450

// A plain colour with Blinn-Phong lighting from the sun, darkened where the sun's light is
// blocked: the shadow map holds, for each place seen from the sun, the depth of the nearest
// thing there. A fragment farther from the sun than that is in shadow. 3x3 neighbouring
// samples are averaged (percentage-closer filtering) to soften the edges.

// The scene's shadow map (bound by the engine, see `shadow_map_sampler` in the scene file).
layout(binding = 1) uniform sampler2D shadowMap;

layout(binding = 2) uniform Lighting {
    // Direction towards the sun, set on the scene's root node every frame.
    vec3 light;
    // The object's colour, set on its node.
    vec3 color;
    // From the material: highlight size and strength.
    float shininess;
    float specularStrength;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec3 fragCamera;
layout(location = 3) in vec4 fragLightSpace;

layout(location = 0) out vec4 outColor;

const float AMBIENT = 0.22;

// 1 where the sun reaches the fragment, 0 in shadow, in between on soft edges.
float sunlight(vec3 normal, vec3 light) {
    vec3 p = fragLightSpace.xyz / fragLightSpace.w;
    if (p.z >= 1.0) {
        return 1.0;
    }
    vec2 uv = p.xy * 0.5 + 0.5;
    // Surfaces at a grazing angle to the light need more bias against shadow acne.
    float bias = 0.0008 + 0.002 * (1.0 - max(dot(normal, light), 0.0));
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
    vec3 normal = normalize(fragNormal);
    vec3 light = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);

    float diffuse = max(dot(normal, light), 0.0);
    float sun = diffuse > 0.0 ? sunlight(normal, light) : 0.0;
    vec3 halfway = normalize(light + toCamera);
    float specular = pow(max(dot(normal, halfway), 0.0), lighting.shininess)
        * lighting.specularStrength * sun;

    vec3 color = lighting.color * (AMBIENT + diffuse * sun) + vec3(specular);
    outColor = vec4(color, 1.0);
}
