#version 450

layout(location = 0) in vec4 vs_Color;
layout(location = 1) in vec2 vs_UVPassthrough;

layout(set = 3, binding = 1) uniform sampler2D u_Texture;

layout(location = 0) out vec4 f_Color;

void main() { f_Color = vs_Color * texture(u_Texture, vs_UVPassthrough); }
