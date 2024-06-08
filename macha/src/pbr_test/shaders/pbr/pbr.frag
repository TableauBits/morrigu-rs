#version 450

#define M_PI 3.1415926535897932384626433832795

layout(location = 0) in vec3 vs_fragPos;
layout(location = 1) in vec3 vs_normal;

layout(location = 0) out vec4 f_Color;

struct ShadingData {
    vec3  V    ; // Normalized vector from shading location to eye
    vec3  L    ; // Normalized vector from shading location to light
    vec3  N    ; // Surface normal

    vec3  H    ; // Half vector ( normalize(L + V) )
    float VdotH; // Hopefully self-explanatory, stored for caching
};

vec3 diffuse_brdf(vec3 color) {
  return (1 / M_PI) * color;
}

void main() {
    // ShadingData data = ShadingData(
    //     normalize(vs_fragPos)
    // );

    f_Color = vec4(0.8, 0.8, 0.2, 1);
}

