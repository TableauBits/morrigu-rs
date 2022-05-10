#version 450

float inverseMix(float a, float b, float v) { return (v - a) / (b - a); }

layout(location = 0) in vec2 vs_UVPassthrough;

layout(set = 3, binding = 1) uniform LerpInfo {
  float tBegin;
  float tEnd;
}
u_LerpInfo;

layout(set = 3, binding = 2) uniform Colors {
  vec4 fromColor;
  vec4 toColor;
}
u_Colors;

layout(location = 0) out vec4 f_Color;

void main() {
  float t =
      clamp(inverseMix(u_LerpInfo.tBegin, u_LerpInfo.tEnd, vs_UVPassthrough.y),
            0.f, 1.f);

  f_Color = mix(u_Colors.fromColor, u_Colors.toColor, t);
}
