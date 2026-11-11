#version 450

// Demo 6's coloured objects, plus where each vertex sits in the shadow light's clip space.

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
    // The shadow light's projection x view, filled by the engine (`shadow_matrix_uniform`).
    mat4 lightViewProj;
} ubo;

// Per-draw data: the node's global transform.
layout(push_constant) uniform PushConstants {
    mat4 model;
} pcs;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

// All in world space, except `fragLightSpace` (the shadow light's clip space).
layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragPosition;
layout(location = 2) out vec3 fragCamera;
layout(location = 3) out vec4 fragLightSpace;

void main() {
    vec4 position = pcs.model * vec4(inPosition, 1.0);
    gl_Position = ubo.proj * ubo.view * position;
    fragNormal = transpose(inverse(mat3(pcs.model))) * inNormal;
    fragPosition = position.xyz;
    // The view matrix moves the camera to the origin, so its inverse moves the origin to the
    // camera.
    fragCamera = inverse(ubo.view)[3].xyz;
    fragLightSpace = ubo.lightViewProj * position;
}
