#version 450

layout(location = 0) in vec2 v_Position;
layout(location = 1) in vec2 v_UV;
layout(location = 2) in vec4 v_Color;

layout(push_constant) uniform ScreenData { vec2 size; }
pc_ScreenData;

layout(location = 0) out vec4 fs_Color;
layout(location = 1) out vec2 fs_UVPassThrough;

void main() {
  vec2 final_position = 2.0 * v_Position / pc_ScreenData.size - 1.0;

  gl_Position = vec4(final_position, 0.0, 1.0);
  fs_Color = v_Color;
  fs_UVPassThrough = v_UV;
}