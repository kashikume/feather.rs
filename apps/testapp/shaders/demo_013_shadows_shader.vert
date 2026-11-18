#version 450

// Lit, shadowed objects: passes the world position and normal to the fragment shader, and the
// position seen from the shadow light, to look up the shadow map.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    // The scene's shadow light: projection x view (filled by the engine, see
    // `shadow_matrix_uniform` in the scene file).
    mat4 lightViewProj;
} camera;

layout(push_constant) uniform Object {
    mat4 model;
} object;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragPosition;
layout(location = 2) out vec3 fragCamera;
layout(location = 3) out vec4 fragLightSpace;

void main() {
    vec4 world = object.model * vec4(inPosition, 1.0);
    gl_Position = camera.proj * camera.view * world;
    // Normals stay perpendicular to surfaces under non-uniform scaling.
    fragNormal = transpose(inverse(mat3(object.model))) * inNormal;
    fragPosition = world.xyz;
    fragCamera = inverse(camera.view)[3].xyz;
    fragLightSpace = camera.lightViewProj * world;
}
