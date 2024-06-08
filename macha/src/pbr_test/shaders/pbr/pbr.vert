#version 450

layout(location = 0) in vec3 v_Position;
layout(location = 1) in vec3 v_Normal;
layout(location = 2) in vec2 v_UV;

layout(push_constant) uniform CameraData {
  mat4 viewProjection;
  vec4 worldPos;
}
pc_CameraData;

layout(set = 3, binding = 0) uniform ModelData { mat4 modelMatrix; }
u_ModelData;

layout(location = 0) out vec3 fs_fragPos;
layout(location = 1) out vec3 fs_normal;
layout(location = 2) out vec2 fs_uv;

void main() {
    mat4 transform = pc_CameraData.viewProjection * u_ModelData.modelMatrix;
    gl_Position = transform * vec4(v_Position, 1);

    fs_fragPos = (u_ModelData.modelMatrix * vec4(v_Position, 1)).xyz;
    fs_normal = v_Normal;
    fs_uv = v_UV;
}

