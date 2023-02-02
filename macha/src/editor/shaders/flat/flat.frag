#version 450

layout(set = 3, binding = 1) uniform ColorData { vec3 color; }
u_ColorData;

layout(location = 0) out vec4 f_Color;

void main() {
  f_Color = vec4(u_ColorData.color, 1);
}

