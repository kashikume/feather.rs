#version 450

// The drive-in's waving canvas: the movie (a scene texture), as projected onto cloth. The
// projector stands behind the cars, so folds facing it are brighter and folds turned away from
// it darker; a faint glow of the projector's light shows where the movie is black.

layout(binding = 1) uniform sampler2D movie;

layout(binding = 2) uniform Screen {
    // How bright the image is (1 = as drawn).
    float brightness;
    // Where the projector is, in world space.
    vec3 projector;
} screen;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec2 fragTexCoord;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

void main() {
    vec3 n = normalize(fragNormal);
    vec3 toProjector = normalize(screen.projector - fragPosition);
    // A flat canvas faces the projector almost straight on: about 1. Folds tilt away.
    float facing = clamp(dot(n, toProjector), 0.0, 1.0);
    float fold = mix(0.25, 1.0, pow(facing, 6.0));

    vec2 centered = fragTexCoord * 2.0 - 1.0;
    float vignette = 1.0 - 0.18 * dot(centered, centered);
    vec3 glow = vec3(0.012, 0.012, 0.014);
    vec3 color = (texture(movie, fragTexCoord).rgb * screen.brightness + glow) * vignette * fold;
    outColor = vec4(color, 1.0);
}
