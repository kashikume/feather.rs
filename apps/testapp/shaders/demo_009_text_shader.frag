#version 450

// Text in one colour: the font atlas's images are white, with the glyphs' coverage in alpha.
//
// The same shader draws both kinds of text (`TextMethod`): one-mesh text has the characters'
// rectangles of the atlas in its texture coordinates, and its material sets the rectangle
// below to the whole atlas; glyph nodes draw the same square, each with its character's
// rectangle in its own uniforms. The scene file doesn't name these uniforms for the engine
// (`umin_uniform` ...), so the engine leaves them to the material and the nodes.

// The whole font atlas.
layout(binding = 1) uniform sampler2D font;

layout(binding = 2) uniform Style {
    // The text's colour and opacity, set on its node (or inherited from an ancestor).
    vec4 color;
    // The rectangle of the atlas the texture coordinates span.
    float umin;
    float umax;
    float vmin;
    float vmax;
} style;

layout(location = 0) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 uv = mix(vec2(style.umin, style.vmin), vec2(style.umax, style.vmax), fragTexCoord);
    float coverage = texture(font, uv).a;
    outColor = vec4(style.color.rgb, style.color.a * coverage);
}
