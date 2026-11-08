#version 450

// Test shader for reflection: every supported uniform type, a block shared with the fragment
// stage and push constants used by both stages.

layout(set = 0, binding = 0) uniform Shared {
    mat4 view;
    mat4 proj;
    float time;
} shared_;

layout(set = 1, binding = 0) uniform Values {
    float f;
    int i;
    uint u;
    bool b;
    vec2 v2;
    vec3 v3;
    vec4 v4;
    ivec2 i2;
    ivec3 i3;
    ivec4 i4;
    uvec2 u2;
    uvec3 u3;
    uvec4 u4;
    mat2 m2;
    mat3 m3;
    mat4 m4;
} values;

layout(push_constant) uniform Push {
    mat4 model;
    vec4 tint;
} pc;

layout(location = 0) in vec3 inPosition;

void main() {
    float s = values.f + float(values.i) + float(values.u) + (values.b ? 1.0 : 0.0)
        + values.v2.x + values.v3.x + values.v4.x
        + float(values.i2.x + values.i3.x + values.i4.x)
        + float(values.u2.x + values.u3.x + values.u4.x)
        + values.m2[0][0] + values.m3[0][0] + values.m4[0][0];
    gl_Position = shared_.proj * shared_.view * values.m4 * pc.model * vec4(inPosition, s);
}
