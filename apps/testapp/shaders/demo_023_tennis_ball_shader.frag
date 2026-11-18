#version 450

// A tennis ball lit by a directional light: the felt colour from the material's texture slot 0
// (colorMap) and its relief from a tangent-space normal map in slot 1 (normalMap, a linear
// atlas). The felt is matte, with a soft wrap-around light, a weak broad highlight and a little
// sheen at the edge.

layout(binding = 1) uniform sampler2D colorMap;
layout(binding = 2) uniform sampler2D normalMap;

layout(binding = 3) uniform Lighting {
    // Direction towards the light, in world space (need not be normalized), and its strength.
    // Set on the scene's root node, so every node inherits them.
    vec3 light;
    float lightIntensity;
    // From the material: how much the normal map bends the normals (0 = a smooth ball) ...
    float normalStrength;
    // ... how sharp the highlight is (higher = smaller, sharper) and how bright.
    float shininess;
    float specularStrength;
} lighting;

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec2 fragTexCoord;
layout(location = 2) in vec3 fragPosition;
layout(location = 3) in vec3 fragCamera;

layout(location = 0) out vec4 outColor;

const float AMBIENT = 0.06;
// How far the light wraps around the felt past the terminator.
const float WRAP = 0.2;

// The tangent frame at this pixel, from the screen-space derivatives of the position and of the
// texture coordinates (C. Schueler, "Normal Mapping Without Precomputed Tangents"): columns are
// the directions of growing u and growing v, and the normal. Meshes need no tangents.
mat3 tangentFrame(vec3 normal, vec3 position, vec2 uv) {
    vec3 dp1 = dFdx(position);
    vec3 dp2 = dFdy(position);
    vec2 duv1 = dFdx(uv);
    vec2 duv2 = dFdy(uv);
    vec3 dp2perp = cross(dp2, normal);
    vec3 dp1perp = cross(normal, dp1);
    vec3 t = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 b = dp2perp * duv1.y + dp1perp * duv2.y;
    // The result is scaled by dot(normal, cross(dp1, dp2)), whose sign depends on which way the
    // framebuffer's y axis goes (down in Vulkan): undo it, keep the directions.
    float side = dot(normal, cross(dp1, dp2)) < 0.0 ? -1.0 : 1.0;
    float scale = side * inversesqrt(max(dot(t, t), dot(b, b)));
    return mat3(t * scale, b * scale, normal);
}

void main() {
    vec3 geometric = normalize(fragNormal);
    vec3 light = normalize(lighting.light);
    vec3 toCamera = normalize(fragCamera - fragPosition);
    vec3 color = texture(colorMap, fragTexCoord).rgb;

    // The normal map: x towards +u, y up in the image (towards -v), z out of the surface.
    vec3 bump = texture(normalMap, fragTexCoord).xyz * 2.0 - 1.0;
    bump.xy *= lighting.normalStrength;
    mat3 frame = tangentFrame(geometric, fragPosition, fragTexCoord);
    vec3 normal = normalize(frame[0] * bump.x - frame[1] * bump.y + frame[2] * bump.z);

    // Wrap-around diffuse light from the bent normal, faded out where the ball itself turns
    // away from the light, so bumps on the dark side stay dark.
    float facing = dot(normal, light);
    float diffuse = max((facing + WRAP) / (1.0 + WRAP), 0.0);
    diffuse *= smoothstep(-0.25, 0.15, dot(geometric, light));
    diffuse *= lighting.lightIntensity;

    vec3 halfway = normalize(light + toCamera);
    float specular = pow(max(dot(normal, halfway), 0.0), lighting.shininess)
        * lighting.specularStrength * lighting.lightIntensity
        * smoothstep(0.0, 0.2, dot(geometric, light));

    // Felt sheen: fibres catch the light at grazing angles, at the lit edge of the ball.
    float rim = pow(1.0 - max(dot(geometric, toCamera), 0.0), 3.0);
    float sheen = rim * 0.25 * smoothstep(-0.1, 0.4, dot(geometric, light)) * lighting.lightIntensity;

    vec3 lit = color * (AMBIENT + diffuse) + vec3(specular) + color * sheen;
    outColor = vec4(lit, 1.0);
}
