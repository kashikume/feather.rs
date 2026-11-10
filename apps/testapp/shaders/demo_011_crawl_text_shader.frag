#version 450

// The crawl's text in one colour, fading out as it moves away along -Z.

// The whole font atlas: white images with the glyphs' coverage in alpha.
layout(binding = 1) uniform sampler2D font;

layout(binding = 2) uniform Style {
    // Colour and opacity, set on the `Crawl` node.
    vec4 color;
    // World Z where the text starts fading out, and where it is gone (fadeEnd < fadeStart).
    float fadeStart;
    float fadeEnd;
} style;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec3 fragWorldPosition;

layout(location = 0) out vec4 outColor;

void main() {
    float coverage = texture(font, fragTexCoord).a;
    float fade = smoothstep(style.fadeEnd, style.fadeStart, fragWorldPosition.z);
    outColor = vec4(style.color.rgb, style.color.a * coverage * fade);
}
