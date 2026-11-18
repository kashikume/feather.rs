#version 450

// The drive-in's ground, posts and frame: a plain colour lit by a dim moon and by the screen.
// The screen is an area light whose colour is the movie's average: the scene texture's
// smallest mip level (the engine makes its mipmaps every frame).

layout(binding = 1) uniform sampler2D movie;

layout(binding = 2) uniform Lighting {
    // Towards the moon, in world space, and its colour (linear).
    vec3 moonDirection;
    vec3 moonColor;
    // The screen: its centre, half its width (along X) and height (along Z); it faces -Y.
    vec3 screenCenter;
    vec2 screenHalfSize;
    // How much light the screen gives, and the distance at which it is halved.
    float screenStrength;
    float screenReach;
    // From the material or the node: the surface's colour (linear).
    vec3 color;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec2 fragTexCoord;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

// Light from the screen reaching a surface at `position` with `normal`: from the screen's point
// nearest to it, falling off with distance, only in front of the screen.
vec3 screenLight(vec3 position, vec3 normal) {
    vec3 local = position - lighting.screenCenter;
    vec3 nearest = lighting.screenCenter + vec3(
        clamp(local.x, -lighting.screenHalfSize.x, lighting.screenHalfSize.x),
        0.0,
        clamp(local.z, -lighting.screenHalfSize.y, lighting.screenHalfSize.y));
    vec3 toScreen = nearest - position;
    float distance = length(toScreen);
    vec3 direction = toScreen / max(distance, 1e-4);
    float front = clamp(direction.y * 4.0, 0.0, 1.0);
    float falloff = 1.0 / (1.0 + pow(distance / lighting.screenReach, 2.0));
    vec3 movie = textureLod(movie, vec2(0.5), 16.0).rgb;
    return movie * lighting.screenStrength * front * falloff * max(dot(normal, direction), 0.0);
}

void main() {
    vec3 n = normalize(fragNormal);
    vec3 moon = lighting.moonColor * max(dot(n, normalize(lighting.moonDirection)), 0.0);
    vec3 ambient = vec3(0.012, 0.014, 0.025);
    vec3 light = ambient + moon + screenLight(fragPosition, n);
    // Far away, everything fades into the night.
    float distance = length(fragCamera - fragPosition);
    float haze = exp(-distance * 0.025);
    vec3 color = lighting.color * light * haze;
    outColor = vec4(color, 1.0);
}
