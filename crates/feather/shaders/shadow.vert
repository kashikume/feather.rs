#version 450

// The engine's shadow pass: draws shadow casters from the shadow light into the shadow map,
// depth only (there is no fragment shader).

layout(push_constant) uniform Shadow {
    // The scene's shadow light: projection x view.
    mat4 lightViewProj;
    // The caster's global transform.
    mat4 model;
} shadow;

layout(location = 0) in vec3 inPosition;

void main() {
    gl_Position = shadow.lightViewProj * shadow.model * vec4(inPosition, 1.0);
}
