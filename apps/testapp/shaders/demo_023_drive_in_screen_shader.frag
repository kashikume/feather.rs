#version 450

// The drive-in's screen: the scene texture the movie (another scene) is drawn into, unlit and a
// little brighter than the movie, as a projected image glows in the dark, with a faint glow of
// the projector's light. Seen from behind, the screen's back shows nothing (its faces are
// culled).

layout(binding = 1) uniform sampler2D movie;

layout(binding = 2) uniform Screen {
    // How bright the image is (1 = as drawn).
    float brightness;
} screen;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec3 fragPosition;
layout(location = 2) in vec2 fragTexCoord;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 centered = fragTexCoord * 2.0 - 1.0;
    // The projector's light falls off towards the corners, and lights the white screen a
    // little even where the movie is black.
    float vignette = 1.0 - 0.18 * dot(centered, centered);
    vec3 glow = vec3(0.012, 0.012, 0.014);
    vec3 color = (texture(movie, fragTexCoord).rgb * screen.brightness + glow) * vignette;
    outColor = vec4(color, 1.0);
}
