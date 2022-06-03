#version 450

layout(location = 0) in vec2 vs_UVPassthrough;

layout(set = 0, binding = 0) uniform TimeData { vec4 scales; }
u_TimeData;

layout(set = 3, binding = 1) uniform sampler2D u_BaseTexture;
layout(set = 3, binding = 2) uniform sampler2D u_FlowMap;
layout(set = 3, binding = 3) uniform sampler2D u_GradientMap;
layout(set = 3, binding = 4) uniform FlowInfo {
  float speed;
  float intensity;
}
u_FlowInfo;

layout(location = 0) out vec4 f_Color;

void main() {
  // flow
  vec2 flow = texture(u_FlowMap, vs_UVPassthrough).rg;
  flow = (flow - 0.5) * 2.0;

  // phases
  float phase_1 = fract(u_TimeData.scales.y * u_FlowInfo.speed);
  float phase_2 = fract(phase_1 + 0.5);
  float flow_mix = abs((phase_1 - 0.5) * 2);

  // UVs
  vec2 phase_1_UV = vs_UVPassthrough + (flow * phase_1 * u_FlowInfo.intensity);
  vec2 phase_2_UV = vs_UVPassthrough + (flow * phase_2 * u_FlowInfo.intensity);

  // color
  vec3 phase_1_texel = texture(u_BaseTexture, phase_1_UV).xyz;
  vec3 phase_2_texel = texture(u_BaseTexture, phase_2_UV).xyz;
  vec2 gradient_UV = 1 - mix(phase_1_texel, phase_2_texel, flow_mix).xy;
  vec3 final_color = texture(u_GradientMap, gradient_UV).xyz;

  f_Color = vec4(final_color, 1);
}
