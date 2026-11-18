#version 450

// An image: the material's texture, on a square whose texture coordinates span it, faded by
// the node's opacity.

layout(binding = 1) uniform sampler2D image;

layout(binding = 2) uniform Fade {
    // 0 (invisible) to 1 (as the texture is), set on the image's node.
    float opacity;
} fade;

layout(location = 0) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    vec4 color = texture(image, fragTexCoord);
    outColor = vec4(color.rgb, color.a * fade.opacity);
}
