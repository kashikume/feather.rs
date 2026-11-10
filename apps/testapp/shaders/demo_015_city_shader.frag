#version 450

// The city's textured, shadowed objects: sunlight with soft shadows, light from the sky and
// the ground, darker street level, sky reflections on glossy surfaces, glow, and fog into the
// sky's colours. Colours are linear (textures are sRGB and decoded when sampled).

layout(binding = 1) uniform sampler2D albedoMap;
// The sun's shadow map (bound by the engine).
layout(binding = 2) uniform sampler2D shadowMap;

layout(binding = 3) uniform Lighting {
    // Set on the `City` node: the sun and the sky.
    vec3 sunDirection;
    vec3 sunColor;
    vec3 skyColor;
    vec3 horizonColor;
    vec3 groundColor;
    float fogDensity;
    // Set on nodes: colours the texture where its alpha is 1 (walls, car paint, shirts).
    vec3 tint;
    // Set on nodes: light given off (traffic lights' lamps).
    vec3 emissive;
    // From the materials.
    float shininess;
    float specularStrength;
    float reflectivity;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec2 fragTexCoord;
layout(location = 3) in vec4 fragLightSpace;
layout(location = 4) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

// The sky's colour in direction `d`, as the sky dome draws it.
vec3 sky(vec3 d) {
    float h = d.z;
    vec3 c = h > 0.0
        ? mix(lighting.horizonColor, lighting.skyColor, pow(h, 0.45))
        : mix(lighting.horizonColor, lighting.groundColor, pow(min(-h * 4.0, 1.0), 0.7));
    float sun = max(dot(d, normalize(lighting.sunDirection)), 0.0);
    return c + lighting.sunColor * (pow(sun, 600.0) * 8.0 + pow(sun, 12.0) * 0.18);
}

// 1 in sunlight, 0 in shadow, in between on the edges (3x3 samples).
float sunlight(vec3 normal, vec3 sunDir) {
    vec3 p = fragLightSpace.xyz / fragLightSpace.w;
    vec2 uv = p.xy * 0.5 + 0.5;
    if (p.z >= 1.0 || any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) {
        return 1.0;
    }
    float slope = 1.0 - max(dot(normal, sunDir), 0.0);
    float bias = 0.00025 + 0.0012 * slope;
    vec2 texel = 1.0 / vec2(textureSize(shadowMap, 0));
    float lit = 0.0;
    for (int x = -1; x <= 1; x++) {
        for (int y = -1; y <= 1; y++) {
            lit += p.z - bias <= texture(shadowMap, uv + vec2(x, y) * texel).r ? 1.0 : 0.0;
        }
    }
    return lit / 9.0;
}

void main() {
    vec4 texel = texture(albedoMap, fragTexCoord);
    vec3 albedo = texel.rgb * mix(vec3(1.0), lighting.tint, texel.a);

    vec3 n = normalize(fragNormal);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    vec3 sunDir = normalize(lighting.sunDirection);

    // Sky light from above, ground light from below; less of it down in the street.
    float up = n.z * 0.5 + 0.5;
    vec3 ambient = mix(lighting.groundColor, lighting.skyColor, up) * 0.9;
    float street = mix(0.55, 1.0, clamp(fragPosition.z / 6.0, 0.0, 1.0));

    float diffuse = max(dot(n, sunDir), 0.0);
    float sun = diffuse > 0.0 ? sunlight(n, sunDir) : 0.0;
    vec3 halfway = normalize(sunDir + toCamera);
    float specular = pow(max(dot(n, halfway), 0.0), lighting.shininess) * lighting.specularStrength;

    vec3 color = albedo * (ambient * street + lighting.sunColor * diffuse * sun)
        + lighting.sunColor * specular * sun;

    // Glossy surfaces reflect the sky, more at grazing angles; reflections of the ground side
    // are darker in shadow.
    vec3 r = reflect(-toCamera, n);
    float fresnel = mix(0.15, 1.0, pow(1.0 - max(dot(n, toCamera), 0.0), 4.0));
    vec3 reflection = sky(r) * (r.z < 0.0 ? sun * 0.6 + 0.4 : 1.0);
    color = mix(color, reflection, clamp(lighting.reflectivity * fresnel, 0.0, 1.0));

    color += lighting.emissive;

    // Fog: far things fade into the sky's colour behind them.
    float distance = length(fragCamera - fragPosition);
    float fog = 1.0 - exp(-pow(distance * lighting.fogDensity, 2.0));
    color = mix(color, sky(-toCamera), fog);

    // Gentle tone mapping for the bright sun and lamps.
    color = vec3(1.0) - exp(-color * 1.35);
    outColor = vec4(color, 1.0);
}
