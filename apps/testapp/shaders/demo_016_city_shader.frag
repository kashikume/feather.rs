#version 450

// Asset-backed city: atlas plate + tiled UVs, sunlight with soft shadows, sky/ground light,
// street darkening, reflections, glow and fog. Colours are linear (textures are sRGB).

layout(binding = 1) uniform sampler2D albedoMap;
layout(binding = 2) uniform sampler2D shadowMap;

// The material's rectangle in the packed atlas (filled by the engine).
layout(binding = 3) uniform TextureRect {
    float umin;
    float umax;
    float vmin;
    float vmax;
} rect;

layout(binding = 4) uniform Lighting {
    vec3 sunDirection;
    vec3 sunColor;
    vec3 skyColor;
    vec3 horizonColor;
    vec3 groundColor;
    float fogDensity;
    vec3 tint;
    vec3 emissive;
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

vec3 sky(vec3 d) {
    float h = d.z;
    vec3 c = h > 0.0
        ? mix(lighting.horizonColor, lighting.skyColor, pow(h, 0.45))
        : mix(lighting.horizonColor, lighting.groundColor, pow(min(-h * 4.0, 1.0), 0.7));
    float sun = max(dot(d, normalize(lighting.sunDirection)), 0.0);
    return c + lighting.sunColor * (pow(sun, 600.0) * 8.0 + pow(sun, 12.0) * 0.18);
}

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
    // Map into the atlas plate. World UVs may tile past 0..1 (grass, roads, facades); use
    // fract for the sample, but keep continuous derivatives so mips don't flicker.
    vec2 plateMin = vec2(rect.umin, rect.vmin);
    vec2 plateSize = vec2(rect.umax - rect.umin, rect.vmax - rect.vmin);
    vec2 local = fract(fragTexCoord);
    // Keep filtering inside the plate (avoid bleeding into neighbouring atlas entries).
    vec2 texelSize = 1.0 / vec2(textureSize(albedoMap, 0));
    vec2 pad = min(vec2(0.5), 0.5 * texelSize / max(plateSize, vec2(1e-6)));
    local = mix(pad, 1.0 - pad, local);
    vec2 uv = plateMin + local * plateSize;
    vec4 texel = textureGrad(albedoMap, uv, dFdx(fragTexCoord) * plateSize, dFdy(fragTexCoord) * plateSize);
    vec3 albedo = texel.rgb * mix(vec3(1.0), lighting.tint, texel.a);

    vec3 n = normalize(fragNormal);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    vec3 sunDir = normalize(lighting.sunDirection);

    float up = n.z * 0.5 + 0.5;
    vec3 ambient = mix(lighting.groundColor, lighting.skyColor, up) * 0.9;
    float street = mix(0.55, 1.0, clamp(fragPosition.z / 6.0, 0.0, 1.0));

    float diffuse = max(dot(n, sunDir), 0.0);
    float sun = diffuse > 0.0 ? sunlight(n, sunDir) : 0.0;
    vec3 halfway = normalize(sunDir + toCamera);
    float specular = pow(max(dot(n, halfway), 0.0), lighting.shininess) * lighting.specularStrength;

    vec3 color = albedo * (ambient * street + lighting.sunColor * diffuse * sun)
        + lighting.sunColor * specular * sun;

    vec3 r = reflect(-toCamera, n);
    float fresnel = mix(0.15, 1.0, pow(1.0 - max(dot(n, toCamera), 0.0), 4.0));
    vec3 reflection = sky(r) * (r.z < 0.0 ? sun * 0.6 + 0.4 : 1.0);
    color = mix(color, reflection, clamp(lighting.reflectivity * fresnel, 0.0, 1.0));

    color += lighting.emissive;

    float distance = length(fragCamera - fragPosition);
    float fog = 1.0 - exp(-pow(distance * lighting.fogDensity, 2.0));
    color = mix(color, sky(-toCamera), fog);

    color = vec3(1.0) - exp(-color * 1.35);
    outColor = vec4(color, 1.0);
}
