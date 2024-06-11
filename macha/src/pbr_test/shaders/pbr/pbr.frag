#version 450

#define M_PI 3.1415926535897932384626433832795

layout(set = 2, binding = 0) uniform LightData {
    vec4 cameraPos;
    vec4 lightPos ;
} u_LightData;

layout(set = 3, binding = 1) uniform PBRParams {
    vec4 albedo;
    vec4 mra   ; // Packed [metallic, roughness, ao, _padding]
} u_PBRParams;

layout(location = 0) in vec3 vs_fragPos;
layout(location = 1) in vec3 vs_normal;
layout(location = 2) in vec2 vs_uv;

layout(location = 0) out vec4 f_color;

struct ShadingData {
    vec3  V        ; // Normalized vector from shading location to eye
    vec3  L        ; // Normalized vector from shading location to light
    vec3  N        ; // Surface normal

    vec3  H        ; // Half vector ( normalize(L + V) )
    float VdotH    ; // Hopefully self-explanatory, stored for caching

    vec3  albedo   ;
    float metallic ;
    float roughness;
    float ao       ;
};

vec3 phong(ShadingData data) {
    vec3 reflectDir = reflect(-data.L, data.N);

    float ambient  = 0.1;
    float diffuse  = max(dot(data.N, data.L), 0.0);
    float specular = 0.5 * pow(max(dot(data.V, reflectDir), 0.0), 32);
    vec3 result = (ambient + diffuse + specular) * data.albedo;

    return result;
}

vec3 fresnelSchlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

float distributionGGX(vec3 N, vec3 H, float roughness) {
    float a      = roughness * roughness;
    float a2     = a * a;
    float NdotH  = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;

    float num   = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = M_PI * denom * denom;

    return num / denom;
}

float geometrySchlickGGX(float NdotV, float roughness) {
    float r = (roughness + 1.0);
    float k = (r * r) / 8.0;

    float num   = NdotV;
    float denom = NdotV * (1.0 - k) + k;

    return num / denom;
}

float geometrySmith(vec3 N, vec3 V, vec3 L, float roughness) {
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2  = geometrySchlickGGX(NdotV, roughness);
    float ggx1  = geometrySchlickGGX(NdotL, roughness);

    return ggx1 * ggx2;
}

void main() {
    vec3 V = normalize(u_LightData.cameraPos.xyz - vs_fragPos);
    vec3 L = normalize(u_LightData.lightPos.xyz  - vs_fragPos);
    vec3 H = normalize(L + V);
    ShadingData data = ShadingData(
        V,
        L,
        normalize(vs_normal),

        H,
        dot(V, H),

        u_PBRParams.albedo.xyz,
        u_PBRParams.mra.x,
        u_PBRParams.mra.y,
        u_PBRParams.mra.z
    );

    vec3 F0 = vec3(0.04);
    F0 = mix(F0, data.albedo, data.metallic);

    vec3 Lo = vec3(0.0);

    float distance = length(u_LightData.lightPos.xyz - vs_fragPos);
    float attenuation = 1.0 / (distance * distance);
    vec3 radiance = vec3(1.0) * attenuation;

    float ndf = distributionGGX(data.N, data.H, data.roughness);
    float g = geometrySmith(data.N, data.V, data.L, data.roughness);
    vec3 f = fresnelSchlick(max(dot(data.H, data.V), 0.0), F0);

    vec3 ks = f;
    vec3 kd = vec3(1.0) - ks;
    kd *= 1.0 - data.metallic;

    vec3 numerator = ndf * g * f;
    float denum = 4.0 * max(dot(data.N, data.V), 0.0) * max(dot(data.N, data.L), 0.0) + 0.0001;
    vec3 specular = numerator / denum;

    float NdotL = max(dot(data.N, data.L), 0.0);
    Lo += (kd * data.albedo / M_PI + specular) * radiance * NdotL;

    vec3 ambient = vec3(0.03) * data.albedo * data.ao;
    vec3 color = ambient + Lo;

    // color = color / (color + vec3(1.0));
    // color = pow(color, vec3(1.0/2.2));

    // vec3 color = phong(data);
    // vec3 color = vec3(data.metallic, data.roughness, data.ao);

    f_color = vec4(color, 1.0);
}

