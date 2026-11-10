#version 450

// Text in one colour: the font atlas's images are white, with the glyphs' coverage in alpha.
// Text meshes carry the characters' rectangles of the atlas in their texture coordinates.

layout(binding = 1) uniform sampler2D font;

layout(binding = 2) uniform Style {
    // Colour and opacity, set on the text's node.
    vec4 color;
} style;

layout(location = 0) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    float coverage = texture(font, fragTexCoord).a;
    outColor = vec4(style.color.rgb, style.color.a * coverage);
}
