#version 450

// Demo 16 city: textured, shadowed objects from OBJ meshes and a PNG atlas.

layout(binding = 0) uniform Camera {
    mat4 view;
    mat4 proj;
    // The sun's projection x view, for the shadow map (filled by the engine).
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
layout(location = 2) out vec2 fragTexCoord;
layout(location = 3) out vec4 fragLightSpace;
layout(location = 4) out vec3 fragCamera;

void main() {
    vec4 world = object.model * vec4(inPosition, 1.0);
    gl_Position = camera.proj * camera.view * world;
    fragNormal = transpose(inverse(mat3(object.model))) * inNormal;
    fragPosition = world.xyz;
    fragTexCoord = inTexCoord;
    fragLightSpace = camera.lightViewProj * world;
    fragCamera = inverse(camera.view)[3].xyz;
}
