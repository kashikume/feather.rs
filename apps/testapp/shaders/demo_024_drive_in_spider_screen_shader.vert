#version 450

// The drive-in's canvas screen, hanging from its top beam and waving in the wind (like demo 3's
// plates).
//
// The mesh is a grid in the XY plane facing +Z, from y = 0 (the top edge, pinned to the beam)
// down to y = -height; x goes from -width / 2 to width / 2. The wind pushes each vertex along
// Z, more the lower it hangs; the bottom edge is held loosely by ropes, so it moves less than
// the middle.

layout(binding = 0) uniform Frame {
    mat4 view;
    mat4 proj;
    // Seconds since the demo started (set on the screen's node).
    float time;
    // The canvas's size, and how far the wind pushes it (scene units).
    float width;
    float height;
    float strength;
} frame;

// Per draw: the node's global transform.
layout(push_constant) uniform PushConstants {
    mat4 model;
} pcs;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

// All in world space.
layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec3 fragPosition;
layout(location = 2) out vec2 fragTexCoord;
layout(location = 3) out vec3 fragCamera;

// How far the wind pushes the point `p` of the canvas along Z.
float displacement(vec2 p) {
    // 0 at the beam, 1 at the bottom edge; across, -1 to 1.
    float down = clamp(-p.y / frame.height, 0.0, 1.0);
    float across = p.x / (frame.width * 0.5);
    // Free most in the middle, held at the beam and a little at the bottom (ropes).
    float hold = sin(down * 3.14159 * 0.85);
    float t = frame.time;
    // Gusts: the wind slowly gets stronger and weaker.
    float gust = 0.6 + 0.4 * sin(t * 0.37);
    // The canvas billows as a whole...
    float billow = 0.6 * sin(t * 0.8) * (1.0 - 0.3 * across * across);
    // ... while waves run across it, faster than they run down.
    float ripple = 0.35 * sin(t * 1.7 + across * 8.0 + down * 2.5)
        + 0.15 * sin(t * 2.9 - across * 14.0 + down * 5.0);
    return frame.strength * hold * gust * (billow + ripple);
}

void main() {
    vec2 p = inPosition.xy;
    float d = displacement(p);
    vec3 position = vec3(p, inPosition.z + d);

    // The normal of the bent canvas, from its slope along X and Y.
    const float e = 0.02;
    vec3 along_x = vec3(e, 0.0, displacement(p + vec2(e, 0.0)) - d);
    vec3 along_y = vec3(0.0, e, displacement(p + vec2(0.0, e)) - d);
    vec3 normal = normalize(cross(along_x, along_y));

    vec4 world = pcs.model * vec4(position, 1.0);
    gl_Position = frame.proj * frame.view * world;
    fragNormal = transpose(inverse(mat3(pcs.model))) * normal;
    fragPosition = world.xyz;
    fragTexCoord = inTexCoord;
    fragCamera = inverse(frame.view)[3].xyz;
}
