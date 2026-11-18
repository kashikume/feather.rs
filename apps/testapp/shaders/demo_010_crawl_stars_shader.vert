#version 450

// Stars: tiny squares, all in one mesh. Each star's brightness and twinkle phase come in its
// vertices' texture coordinates.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
} camera;

layout(push_constant) uniform Stars {
    mat4 model;
} stars;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

// x: brightness (0..1), y: twinkle phase (0..1).
layout(location = 0) out vec2 fragStar;

void main() {
    gl_Position = camera.proj * camera.view * stars.model * vec4(inPosition, 1.0);
    fragStar = inTexCoord;
}
