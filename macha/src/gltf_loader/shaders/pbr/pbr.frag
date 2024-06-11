#version 450

layout(location = 0) in vec3 vs_PositionPassthrough;
layout(location = 1) in vec3 vs_NormalPassthrough;
layout(location = 2) in vec2 vs_UVPassthrough;

layout(set = 2, binding = 0) uniform LightData {
    vec4 lightDirection;
    vec4 lightColor;
    
    vec3 ambientLightColor;
    float ambientLightIntensity;

    vec4 cameraPosition;
} u_LightData;

layout(set = 2, binding = 1) uniform PBRData {
    vec4 baseColorFactor;
    float metallicFactor;
    float roughnessFactor;
    float alphaCutoff;
} u_PBRData;

layout(set = 2, binding = 2) uniform MapPresenceInfo {
    uint hasBaseColorMap;
    uint hasNormalMap;
    uint hasMetalRoughnessMap;
} u_MapPresenceInfo;

layout(set = 2, binding = 3) uniform sampler2D u_BaseColorSampler;
layout(set = 2, binding = 4) uniform sampler2D u_NormalSampler;
layout(set = 2, binding = 5) uniform sampler2D u_MetallicRoughnessSampler;

layout(location = 0) out vec4 f_Color;

// Encapsulate the various inputs used by the various functions in the shading equation
// We store values in this struct to simplify the integration of alternative implementations
// of the shading terms, outlined in the Readme.MD Appendix.
struct PBRInfo
{
    float NdotL;                  // cos angle between normal and light direction
    float NdotV;                  // cos angle between normal and view direction
    float NdotH;                  // cos angle between normal and half vector
    float LdotH;                  // cos angle between light direction and half vector
    float VdotH;                  // cos angle between view direction and half vector
    float perceptualRoughness;    // roughness value, as authored by the model creator (input to shader)
    float metalness;              // metallic value at the surface
    vec3 reflectance0;            // full reflectance color (normal incidence angle)
    vec3 reflectance90;           // reflectance color at grazing angle
    float alphaRoughness;         // roughness mapped to a more linear change in the roughness (proposed by [2])
    vec3 diffuseColor;            // color contribution from diffuse lighting
    vec3 specularColor;           // color contribution from specular lighting
};

const float M_PI = 3.141592653589793;
const float c_MinRoughness = 0.04;

// Find the normal for this fragment, pulling either from a predefined normal map
// or from the interpolated mesh normal and tangent attributes.
vec3 getNormal()
{
    // Retrieve the tangent space matrix
    vec3 pos_dx = dFdx(vs_PositionPassthrough);
    vec3 pos_dy = dFdy(vs_PositionPassthrough);
    vec3 tex_dx = dFdx(vec3(vs_UVPassthrough, 0.0));
    vec3 tex_dy = dFdy(vec3(vs_UVPassthrough, 0.0));
    vec3 t = (tex_dy.t * pos_dx - tex_dx.t * pos_dy) / (tex_dx.s * tex_dy.t - tex_dy.s * tex_dx.t);

    vec3 ng = normalize(vs_NormalPassthrough);

    t = normalize(t - ng * dot(ng, t));
    vec3 b = normalize(cross(ng, t));
    mat3 tbn = mat3(t, b, ng);

    vec3 n;
    if (u_MapPresenceInfo.hasNormalMap != 0) {
        n = texture(u_NormalSampler, vs_UVPassthrough).rgb;
        n = normalize(tbn * ((2.0 * n - 1.0)));
    } else {
        // The tbn matrix is linearly interpolated, so we need to re-normalize
        n = normalize(tbn[2].xyz);
    }

    // reverse backface normals
    // TODO!: correct/best place? -> https://github.com/KhronosGroup/glTF-WebGL-PBR/issues/51
    n *= (2.0 * float(gl_FrontFacing) - 1.0);

    return n;
}

// Basic Lambertian diffuse
// Implementation from Lambert's Photometria https://archive.org/details/lambertsphotome00lambgoog
// See also [1], Equation 1
vec3 diffuse(PBRInfo pbrInputs)
{
    return pbrInputs.diffuseColor / M_PI;
}

// The following equation models the Fresnel reflectance term of the spec equation (aka F())
// Implementation of fresnel from [4], Equation 15
vec3 specularReflection(PBRInfo pbrInputs)
{
    return pbrInputs.reflectance0 + (pbrInputs.reflectance90 - pbrInputs.reflectance0) * pow(clamp(1.0 - pbrInputs.VdotH, 0.0, 1.0), 5.0);
}

// This calculates the specular geometric attenuation (aka G()),
// where rougher material will reflect less light back to the viewer.
// This implementation is based on [1] Equation 4, and we adopt their modifications to
// alphaRoughness as input as originally proposed in [2].
float geometricOcclusion(PBRInfo pbrInputs)
{
    float NdotL = pbrInputs.NdotL;
    float NdotV = pbrInputs.NdotV;
    float r = pbrInputs.alphaRoughness;

    float attenuationL = 2.0 * NdotL / (NdotL + sqrt(r * r + (1.0 - r * r) * (NdotL * NdotL)));
    float attenuationV = 2.0 * NdotV / (NdotV + sqrt(r * r + (1.0 - r * r) * (NdotV * NdotV)));
    return attenuationL * attenuationV;
}

// The following equation(s) model the distribution of microfacet normals across the area being drawn (aka D())
// Implementation from "Average Irregularity Representation of a Roughened Surface for Ray Reflection" by T. S. Trowbridge, and K. P. Reitz
// Follows the distribution function recommended in the SIGGRAPH 2013 course notes from EPIC Games [1], Equation 3.
float microfacetDistribution(PBRInfo pbrInputs)
{
    float roughnessSq = pbrInputs.alphaRoughness * pbrInputs.alphaRoughness;
    float f = (pbrInputs.NdotH * roughnessSq - pbrInputs.NdotH) * pbrInputs.NdotH + 1.0;
    return roughnessSq / (M_PI * f * f);
}

void main()
{
    // Metallic and Roughness material properties are packed together
    // In glTF, these factors can be specified by fixed scalar values
    // or from a metallic-roughness map
    float perceptualRoughness = u_PBRData.roughnessFactor;
    float metallic = u_PBRData.metallicFactor;
    if (u_MapPresenceInfo.hasMetalRoughnessMap != 0) {
        // Roughness is stored in the 'g' channel, metallic is stored in the 'b' channel.
        // This layout intentionally reserves the 'r' channel for (optional) occlusion map data
        vec4 mrSample = texture(u_MetallicRoughnessSampler, vs_UVPassthrough);
        perceptualRoughness = mrSample.g * perceptualRoughness;
        metallic = mrSample.b * metallic;
    }
    perceptualRoughness = clamp(perceptualRoughness, c_MinRoughness, 1.0);
    metallic = clamp(metallic, 0.0, 1.0);
    // Roughness is authored as perceptual roughness; as is convention,
    // convert to material roughness by squaring the perceptual roughness [2].
    float alphaRoughness = perceptualRoughness * perceptualRoughness;

    // The albedo may be defined from a base texture or a flat color
    vec4 baseColor;
    if (u_MapPresenceInfo.hasBaseColorMap != 0) {
        vec4 texel = texture(u_BaseColorSampler, vs_UVPassthrough);
        baseColor = vec4(pow(texel.rgb, vec3(2.2)), texel.a) * u_PBRData.baseColorFactor;
    } else {
        baseColor = u_PBRData.baseColorFactor;
    }

    vec3 f0 = vec3(0.04);
    vec3 diffuseColor = baseColor.rgb * (vec3(1.0) - f0);
    diffuseColor *= 1.0 - metallic;
    vec3 specularColor = mix(f0, baseColor.rgb, metallic);

    // Compute reflectance.
    float reflectance = max(max(specularColor.r, specularColor.g), specularColor.b);

    // For typical incident reflectance range (between 4% to 100%) set the grazing reflectance to 100% for typical fresnel effect.
    // For very low reflectance range on highly diffuse objects (below 4%), incrementally reduce grazing reflecance to 0%.
    float reflectance90 = clamp(reflectance * 25.0, 0.0, 1.0);
    vec3 specularEnvironmentR0 = specularColor.rgb;
    vec3 specularEnvironmentR90 = vec3(1.0, 1.0, 1.0) * reflectance90;

    vec3 n = getNormal();                             // normal at surface point
    vec3 v = normalize(u_LightData.cameraPosition.xyz - vs_PositionPassthrough);        // Vector from surface point to camera
    vec3 l = normalize(u_LightData.lightDirection.xyz);             // Vector from surface point to light
    vec3 h = normalize(l+v);                          // Half vector between both l and v
    vec3 reflection = -normalize(reflect(v, n));

    float NdotL = clamp(dot(n, l), 0.001, 1.0);
    float NdotV = clamp(abs(dot(n, v)), 0.001, 1.0);
    float NdotH = clamp(dot(n, h), 0.0, 1.0);
    float LdotH = clamp(dot(l, h), 0.0, 1.0);
    float VdotH = clamp(dot(v, h), 0.0, 1.0);

    PBRInfo pbrInputs = PBRInfo(
        NdotL,
        NdotV,
        NdotH,
        LdotH,
        VdotH,
        perceptualRoughness,
        metallic,
        specularEnvironmentR0,
        specularEnvironmentR90,
        alphaRoughness,
        diffuseColor,
        specularColor
    );

    // Calculate the shading terms for the microfacet specular shading model
    vec3 F = specularReflection(pbrInputs);
    float G = geometricOcclusion(pbrInputs);
    float D = microfacetDistribution(pbrInputs);

    // Calculation of analytical lighting contribution
    vec3 diffuseContrib = (1.0 - F) * diffuse(pbrInputs);
    vec3 specContrib = F * G * D / (4.0 * NdotL * NdotV);
    vec3 color = NdotL * u_LightData.lightColor.rgb * (diffuseContrib + specContrib);

    // Add simple ambient light
    color += u_LightData.ambientLightColor * u_LightData.ambientLightColor * baseColor.xyz;

    // // This section uses mix to override final color for reference app visualization
    // // of various parameters in the lighting equation.
    // color = mix(color, F, u_ScaleFGDSpec.x);
    // color = mix(color, vec3(G), u_ScaleFGDSpec.y);
    // color = mix(color, vec3(D), u_ScaleFGDSpec.z);
    // color = mix(color, specContrib, u_ScaleFGDSpec.w);

    // color = mix(color, diffuseContrib, u_ScaleDiffBaseMR.x);
    // color = mix(color, baseColor.rgb, u_ScaleDiffBaseMR.y);
    // color = mix(color, vec3(metallic), u_ScaleDiffBaseMR.z);
    // color = mix(color, vec3(perceptualRoughness), u_ScaleDiffBaseMR.w);

    // NOTE: the spec mandates to ignore any alpha value in 'OPAQUE' mode
    float alpha = baseColor.a;
    if (u_PBRData.alphaCutoff > 0.0) {
        alpha = step(u_PBRData.alphaCutoff, baseColor.a);
    }

    if (alpha <= 0.0) {
        discard;
    }

    // TODO!: apply fix from reference shader:
    // https://github.com/KhronosGroup/glTF-WebGL-PBR/pull/55/files#diff-f7232333b020880432a925d5a59e075d
    f_Color = vec4(color, alpha);
}

