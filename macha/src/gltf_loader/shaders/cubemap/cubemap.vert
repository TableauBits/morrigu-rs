#version 450

layout(location = 0) in vec3 v_Position;

layout(push_constant) uniform CameraData {
    mat4 viewProjection;
    vec4 worldPos;
}
pc_CameraData;

layout(set = 3, binding = 0) uniform ModelData { mat4 modelMatrix; }
u_ModelData;

layout(location = 0) out vec3 fs_TexCoords;

void main() {
    fs_TexCoords = v_Position;

    mat4 transform = pc_CameraData.viewProjection * u_ModelData.modelMatrix;
    vec4 pos = transform * vec4(v_Position, 1);
    gl_Position = pos.xyww;
}

