#version 450

// A flat plate, drawn where its node puts it.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} camera;

// Per plate: the node's global transform.
layout(push_constant) uniform Plate {
    mat4 model;
} plate;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec2 fragTexCoord;

void main() {
    gl_Position = camera.proj * camera.view * plate.model * vec4(inPosition, 1.0);
    fragTexCoord = inTexCoord;
}
