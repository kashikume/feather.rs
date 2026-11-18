#version 450

// The simplest shadow receiver, vertex part: besides the usual position, it computes where the
// vertex is as seen from the shadow light, to look it up in the shadow map.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    // The shadow light's projection x view, filled by the engine (`shadow_matrix_uniform`).
    mat4 lightViewProj;
} camera;

layout(push_constant) uniform Object {
    mat4 model;
} object;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec3 fragNormal;
// The position in the shadow light's clip space.
layout(location = 1) out vec4 fragLightSpace;

void main() {
    vec4 world = object.model * vec4(inPosition, 1.0);
    gl_Position = camera.proj * camera.view * world;
    fragNormal = mat3(object.model) * inNormal;
    fragLightSpace = camera.lightViewProj * world;
}
