#version 450

// A text shader for the unit tests: samples the font atlas in a rectangle given by uniforms
// (`text::GLYPH_RECT_UNIFORMS`), in a colour.

layout(binding = 1) uniform sampler2D font;

layout(binding = 2) uniform Style {
    vec4 color;
    float umin;
    float umax;
    float vmin;
    float vmax;
} style;

layout(location = 1) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 uv = mix(vec2(style.umin, style.vmin), vec2(style.umax, style.vmax), fragTexCoord);
    outColor = vec4(style.color.rgb, style.color.a * texture(font, uv).a);
}
