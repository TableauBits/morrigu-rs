#version 450

layout(location = 0) in vec3 vs_TexCoords;

layout(set = 2, binding = 0) uniform samplerCube u_CubeMapTexture;

layout(location = 0) out vec4 f_Color;

void main() {
    f_Color = texture(u_CubeMapTexture, vs_TexCoords);
}
