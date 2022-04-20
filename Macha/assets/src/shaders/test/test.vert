#version 450

layout(location = 0) in vec3 v_Position;
layout(location = 1) in vec3 v_Normal;
layout(location = 2) in vec2 v_UV;

layout(push_constant) uniform CameraData
{
	mat4 viewProjection;
	vec4 worldPos;
}
pc_CameraData;

layout(set = 3, binding = 0) uniform ModelData { mat4 modelMatrix; }
u_ModelData;

layout(location = 0) out vec2 fs_UVPassThrough;

void main()
{
	mat4 transform   = pc_CameraData.viewProjection * u_ModelData.modelMatrix;
	gl_Position      = transform * vec4(v_Position, 1);
	fs_UVPassThrough = v_UV;
}
