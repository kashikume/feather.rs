#version 450

// The crawl's text: one mesh per text, lying on the XZ plane, with the characters' rectangles of
// the font atlas in the texture coordinates.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} camera;

// Per text: the node's global transform.
layout(push_constant) uniform Text {
    mat4 model;
} text;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec2 fragTexCoord;
layout(location = 1) out vec3 fragWorldPosition;

void main() {
    vec4 world = text.model * vec4(inPosition, 1.0);
    gl_Position = camera.proj * camera.view * world;
    fragTexCoord = inTexCoord;
    fragWorldPosition = world.xyz;
}
