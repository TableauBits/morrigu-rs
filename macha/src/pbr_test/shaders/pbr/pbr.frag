#version 450

#define M_PI 3.1415926535897932384626433832795

layout(set = 2, binding = 0) uniform LightData {
    vec4 cameraPos;
    vec4 lightPos ;
} u_LightData;

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
    vec3 V = normalize(u_LightData.cameraPos.xyz - vs_fragPos);
    vec3 L = normalize(u_LightData.lightPos.xyz  - vs_fragPos);
    vec3 H = normalize(L + V);
    ShadingData data = ShadingData(V, L, vs_normal, H, dot(V, H));

    vec3 result = (vec3(0.1) + max(dot(data.N, data.L), 0.0)) * vec3(0.8, 0.8, 0.2);
    f_Color = vec4(result, 1.0);
}

