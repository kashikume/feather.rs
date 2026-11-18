#version 450

// A shader for the unit tests with two texture samplers (material slots 0 and 1), each with
// its own rectangle uniforms, used with basic.vert.

layout(binding = 1) uniform sampler2D colorMap;
layout(binding = 2) uniform sampler2D normalMap;

layout(binding = 3) uniform Rects {
    float colorUmin;
    float colorUmax;
    float colorVmin;
    float colorVmax;
    float normalUmin;
    float normalUmax;
    float normalVmin;
    float normalVmax;
} rects;

layout(location = 0) in vec4 fragNormal;
layout(location = 1) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

void main() {
    vec2 colorUv = mix(vec2(rects.colorUmin, rects.colorVmin),
                       vec2(rects.colorUmax, rects.colorVmax), fragTexCoord);
    vec2 normalUv = mix(vec2(rects.normalUmin, rects.normalVmin),
                        vec2(rects.normalUmax, rects.normalVmax), fragTexCoord);
    vec3 normal = texture(normalMap, normalUv).xyz * 2.0 - 1.0;
    outColor = texture(colorMap, colorUv) * max(normal.z, 0.0);
}
