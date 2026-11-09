#version 450

// A plate hanging from its top edge and waving in the wind.
//
// The mesh is a grid in the XY plane facing +Z, from y = 0 (top edge, pinned) down to y = -1.
// The wind pushes each vertex along +Z, more the lower it hangs.

layout(binding = 0) uniform Frame {
    mat4 view;
    mat4 proj;
    // Seconds since the demo started.
    float time;
} frame;

// Per plate.
layout(push_constant) uniform Plate {
    mat4 model;
    // Where in its swing the plate starts, in radians.
    float phase;
    // How fast the plate flutters.
    float speed;
    // How far the wind pushes the plate.
    float strength;
} plate;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec2 fragTexCoord;

// How far the wind pushes the point `p` of the mesh along +Z.
float displacement(vec2 p) {
    float hang = clamp(-p.y, 0.0, 1.0);
    float t = frame.time * plate.speed + plate.phase;
    // Gusts: the wind slowly gets stronger and weaker, differently for every plate.
    float gust = 0.55 + 0.45 * sin(frame.time * 0.45 + plate.phase * 1.7);
    // The whole plate swings around its top edge...
    float swing = hang * sin(t * 0.7);
    // ... while ripples run down and across it.
    float ripple = hang * hang
        * (0.35 * sin(t * 1.9 + hang * 4.0 + p.x * 2.5) + 0.12 * sin(t * 3.1 - p.x * 4.0 + hang * 2.0));
    // A steady push away from the viewer.
    float push = -0.3 * hang;
    return plate.strength * (gust * (swing + ripple) + push);
}

void main() {
    vec2 p = inPosition.xy;
    float d = displacement(p);
    vec3 position = vec3(p, inPosition.z + d);

    // The normal of the bent surface, from its slope along X and Y.
    const float e = 0.01;
    vec3 along_x = vec3(e, 0.0, displacement(p + vec2(e, 0.0)) - d);
    vec3 along_y = vec3(0.0, e, displacement(p + vec2(0.0, e)) - d);
    vec3 normal = normalize(cross(along_x, along_y));

    gl_Position = frame.proj * frame.view * plate.model * vec4(position, 1.0);
    fragNormal = mat3(plate.model) * normal;
    fragTexCoord = inTexCoord;
}
