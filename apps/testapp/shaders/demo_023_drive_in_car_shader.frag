#version 450

// The drive-in's cars: the city's vehicle texture (material slot 0, a rectangle of the city
// atlas), its paintable parts tinted per car, lit by a dim moon and by the screen, an area
// light in the movie's average colour (slot 1: the scene texture's smallest mip level), with a
// glossy highlight of each.

layout(binding = 1) uniform sampler2D albedoMap;
layout(binding = 2) uniform sampler2D movie;

// The vehicle texture's rectangle in the atlas (filled by the engine).
layout(binding = 3) uniform TextureRect {
    float umin;
    float umax;
    float vmin;
    float vmax;
} rect;

layout(binding = 4) uniform Lighting {
    vec3 moonDirection;
    vec3 moonColor;
    vec3 screenCenter;
    vec2 screenHalfSize;
    float screenStrength;
    float screenReach;
    // Per car: the paint (linear), where the texture's alpha marks paintable parts.
    vec3 tint;
    float shininess;
    float specularStrength;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec2 fragTexCoord;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

// See demo_023_drive_in_lit_shader.frag. Also returns the direction towards the screen.
vec3 screenLight(vec3 position, vec3 normal, out vec3 direction) {
    vec3 local = position - lighting.screenCenter;
    vec3 nearest = lighting.screenCenter + vec3(
        clamp(local.x, -lighting.screenHalfSize.x, lighting.screenHalfSize.x),
        0.0,
        clamp(local.z, -lighting.screenHalfSize.y, lighting.screenHalfSize.y));
    vec3 toScreen = nearest - position;
    float distance = length(toScreen);
    direction = toScreen / max(distance, 1e-4);
    float front = clamp(direction.y * 4.0, 0.0, 1.0);
    float falloff = 1.0 / (1.0 + pow(distance / lighting.screenReach, 2.0));
    vec3 movie = textureLod(movie, vec2(0.5), 16.0).rgb;
    return movie * lighting.screenStrength * front * falloff;
}

void main() {
    vec2 uv = mix(vec2(rect.umin, rect.vmin), vec2(rect.umax, rect.vmax), fract(fragTexCoord));
    vec4 texel = texture(albedoMap, uv);
    vec3 albedo = texel.rgb * mix(vec3(1.0), lighting.tint, texel.a);

    vec3 n = normalize(fragNormal);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    vec3 moonDir = normalize(lighting.moonDirection);
    vec3 screenDir;
    vec3 screen = screenLight(fragPosition, n, screenDir);

    vec3 ambient = vec3(0.012, 0.014, 0.025);
    vec3 diffuse = lighting.moonColor * max(dot(n, moonDir), 0.0)
        + screen * max(dot(n, screenDir), 0.0);
    float moonSpecular = pow(max(dot(n, normalize(moonDir + toCamera)), 0.0), lighting.shininess);
    float screenSpecular = pow(max(dot(n, normalize(screenDir + toCamera)), 0.0), lighting.shininess);
    vec3 specular = (lighting.moonColor * moonSpecular + screen * screenSpecular)
        * lighting.specularStrength;

    float distance = length(fragCamera - fragPosition);
    float haze = exp(-distance * 0.025);
    outColor = vec4((albedo * (ambient + diffuse) + specular) * haze, 1.0);
}
