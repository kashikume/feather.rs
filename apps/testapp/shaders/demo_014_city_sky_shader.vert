#version 450

// The sky: a dome around the camera, coloured by direction.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} camera;

layout(push_constant) uniform Dome {
    mat4 model;
} dome;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

// The direction from the camera, in the world.
layout(location = 0) out vec3 fragDirection;

void main() {
    gl_Position = camera.proj * camera.view * dome.model * vec4(inPosition, 1.0);
    fragDirection = inPosition;
}
