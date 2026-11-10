#version 450

// An image: the material's texture, on a square whose texture coordinates span it.

layout(binding = 1) uniform sampler2D image;

layout(location = 0) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = texture(image, fragTexCoord);
}
