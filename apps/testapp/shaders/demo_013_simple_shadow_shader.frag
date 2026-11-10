#version 450

// The simplest shadow receiver, fragment part: diffuse light, except where the shadow map says
// something nearer to the light is in the way.

// The scene's shadow map, bound by the engine (`shadow_map_sampler`): for each place seen from
// the light, the depth (0 near .. 1 far) of the nearest shadow caster there.
layout(binding = 1) uniform sampler2D shadowMap;

layout(binding = 2) uniform Lighting {
    // Direction towards the light, set on the root node.
    vec3 light;
    // The object's colour, set on its node.
    vec3 color;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec4 fragLightSpace;

layout(location = 0) out vec4 outColor;

const float AMBIENT = 0.25;
// Keeps surfaces from shadowing themselves because of the map's limited precision.
const float BIAS = 0.002;

void main() {
    // Where the fragment is in the shadow map (0..1), and its depth seen from the light.
    vec3 p = fragLightSpace.xyz / fragLightSpace.w;
    vec2 place = p.xy * 0.5 + 0.5;
    float nearest = texture(shadowMap, place).r;
    // Lit when nothing nearer to the light was drawn at that place.
    float lit = p.z - BIAS <= nearest ? 1.0 : 0.0;

    float diffuse = max(dot(normalize(fragNormal), normalize(lighting.light)), 0.0);
    outColor = vec4(lighting.color * (AMBIENT + diffuse * lit), 1.0);
}
