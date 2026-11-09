#version 450

// Draws one texture of the atlas.

// The whole atlas.
layout(binding = 1) uniform sampler2D atlas;

// The rectangle of the plate's texture in the atlas, filled by the engine from the material's
// texture (`umin_uniform` ... `vmax_uniform` in the scene file).
layout(binding = 2) uniform TextureRect {
    float umin;
    float umax;
    float vmin;
    float vmax;
} rect;

layout(location = 0) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 uv = mix(vec2(rect.umin, rect.vmin), vec2(rect.umax, rect.vmax), fragTexCoord);
    outColor = texture(atlas, uv);
}
