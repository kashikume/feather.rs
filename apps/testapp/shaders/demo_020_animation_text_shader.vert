#version 450

// Text: a quad per character, either all in one mesh or each on its own node (see the
// fragment shader).

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

void main() {
    gl_Position = camera.proj * camera.view * text.model * vec4(inPosition, 1.0);
    fragTexCoord = inTexCoord;
}
