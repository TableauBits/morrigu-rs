use std::{fs::DirEntry, io::Write, path::Path};

use shaderc::{CompileOptions, EnvVersion, TargetEnv};

fn compile_shader(entry: DirEntry) {
    if !entry.file_type().unwrap().is_file() {
        return;
    }

    let input_file_path = entry.path();
    let input_file_name = input_file_path.file_name().expect("Invalid file name");
    let output_file_path = {
        let mut temp = input_file_path.clone();
        // remove:
        // -	filename
        temp.pop();
        // -	src
        temp.pop();
        // add:
        // -	gen
        temp.push("gen");
        // -	filename
        temp.push(
            input_file_name
                .to_str()
                .unwrap()
                .replace(".glsl.", ".spirv."),
        );

        temp
    };

    println!(
        "cargo:warning=Compiling {} -> {}",
        input_file_path.to_str().unwrap(),
        output_file_path.to_str().unwrap()
    );

    let source = std::fs::read_to_string(&input_file_path).expect("Failed to read file");

    let shader_type = match input_file_path
        .extension()
        .expect("No valid extension")
        .to_str()
        .expect("Invalid filename encoding")
    {
        "vert" => Ok(shaderc::ShaderKind::Vertex),
        "frag" => Ok(shaderc::ShaderKind::Fragment),
        _ => Err("Invalid extension"),
    }
    .expect("Failed to parse shader type");

    let mut compile_options = CompileOptions::new().unwrap();
    compile_options.set_target_env(TargetEnv::Vulkan, EnvVersion::Vulkan1_1 as u32);

    let compiler = shaderc::Compiler::new().expect("Failed to create shaderc compiler");
    let compiled_spirv = compiler
        .compile_into_spirv(
            source.as_str(),
            shader_type,
            input_file_name.to_str().expect("Invalid file name"),
            "main",
            Some(&compile_options),
        )
        .expect("Failed to compile shader");

    std::fs::create_dir_all(output_file_path.parent().unwrap())
        .expect("Failed to create necessary shader folder");
    let mut output_file =
        std::fs::File::create(output_file_path).expect("Failed to create shader output file");
    output_file
        .write_all(compiled_spirv.as_binary_u8())
        .expect("Failed to write in output file");
}

fn compile_shaders_in_dir(parent_dir: &Path) {
    let mut input_dir = parent_dir.to_owned();
    input_dir.push("src");

    let mut output_dir = parent_dir.to_owned();
    output_dir.push("gen");
    if output_dir.exists() {
        if let Err(error) = std::fs::remove_dir_all(&output_dir) {
            println!("cargo:warning=Failed to clean shader output folder {error}");
        }
    }

    for entry in std::fs::read_dir(&input_dir)
        .unwrap_or_else(|_| panic!("Directory {} should exist", input_dir.display()))
    {
        let entry = entry
            .unwrap_or_else(|_| panic!("Failed to iterate over directory {}", input_dir.display()));

        compile_shader(entry);
    }
}

fn main() {
    let shader_dirs = [Path::new("src/egui_integration/shaders")];

    for dir in shader_dirs {
        println!("cargo:rerun-if-changed={}", dir.display());
        compile_shaders_in_dir(dir);
    }
}
