use crate::error::Error;

use ash::{vk, Device};
use spirv_reflect::types::variable;

use std::{
    fs::{self},
    path::Path,
};

pub struct Shader {
    vertex_module: vk::ShaderModule,
    fragment_module: vk::ShaderModule,

    pub reflection_entry_points: Vec<variable::ReflectEntryPoint>,
}

impl Shader {
    fn create_shader_module(
        device: &Device,
        source: &[u32],
    ) -> Result<vk::ShaderModule, vk::Result> {
        let module_info = vk::ShaderModuleCreateInfo::builder().code(source);

        unsafe { device.create_shader_module(&module_info, None) }
    }
}

impl Shader {
    /// This function expects a valid path for both **SPIR-V compiled** shader files.
    pub fn from_path(
        device: &Device,
        vertex_path: &Path,
        fragment_path: &Path,
    ) -> Result<Self, Error> {
        let vertex_spirv = fs::read(vertex_path)?;
        let fragment_spirv = fs::read(fragment_path)?;

        Self::from_spirv_u8(device, &vertex_spirv, &fragment_spirv)
    }

    /// This function expects **COMPILED SPIR-V**, not higher level languages like GLSL or HSLS source code.
    pub fn from_spirv_u8(
        device: &Device,
        vertex_spirv: &[u8],
        fragment_spirv: &[u8],
    ) -> Result<Self, Error> {
        let vertex_u32 = ash::util::read_spv(&mut std::io::Cursor::new(vertex_spirv))?;
        let fragment_u32 = ash::util::read_spv(&mut std::io::Cursor::new(fragment_spirv))?;

        Self::from_spirv_u32(device, &vertex_u32, &fragment_u32)
    }

    /// This function expects **COMPILED SPIR-V**, not higher level languages like GLSL or HSLS source code.
    pub fn from_spirv_u32(
        device: &Device,
        vertex_spirv: &[u32],
        fragment_spirv: &[u32],
    ) -> Result<Self, Error> {
        let vertex_module = Self::create_shader_module(device, vertex_spirv)?;
        let fragment_module = Self::create_shader_module(device, fragment_spirv)?;

        let reflection_module = spirv_reflect::ShaderModule::load_u32_data(vertex_spirv)?;
        let reflection_entry_points = reflection_module.enumerate_entry_points()?;

        Ok(Self {
            vertex_module,
            fragment_module,
            reflection_entry_points,
        })
    }

    pub fn destroy(self, device: &Device) {
        unsafe {
            device.destroy_shader_module(self.fragment_module, None);
            device.destroy_shader_module(self.vertex_module, None);
        }
    }
}
