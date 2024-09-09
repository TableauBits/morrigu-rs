use std::{
    fs::DirEntry,
    io::Write,
    path::{Path, PathBuf},
};

use shaderc::{CompileOptions, EnvVersion, TargetEnv};

fn build_shaders(source_path: PathBuf) {
    let mut output_path = source_path.clone();
    output_path.push("gen");

    if output_path.exists() {
        if let Err(error) = std::fs::remove_dir_all(output_path.clone()) {
            println!("cargo:warning=Failed to clean shader output folder {error}");
        }
        std::fs::create_dir_all(output_path.clone())
            .expect("Failed to create shader output folder");
    }

    for entry in std::fs::read_dir(source_path).expect("Failed to read shader source folder") {
        let entry = entry.expect("Failed to iterate over files");

        if entry.path().ends_with("gen") {
            continue;
        }

        if entry.file_type().unwrap().is_dir() {
            recursive_compile(&entry.path(), &output_path);
        } else {
            compile_shader(entry, &output_path);
        }
    }
}

fn recursive_compile(source_path: &Path, output_path: &Path) {
    for entry in std::fs::read_dir(source_path).expect("Failed to read shader source folder") {
        let entry = entry.expect("Failed to iterate over files");

        if entry.file_type().unwrap().is_dir() {
            recursive_compile(&entry.path(), output_path);
        } else {
            compile_shader(entry, output_path);
        }
    }
}

fn compile_shader(entry: DirEntry, output_path: &Path) {
    if !entry.file_type().unwrap().is_file() {
        return;
    }

    let mut path_prefix = PathBuf::from(output_path);
    path_prefix.pop();

    let input_file_path = entry.path();
    let input_file_name = input_file_path.file_name().expect("Invalid file name");
    let output_file_path = PathBuf::from(output_path).join(
        input_file_path
            .strip_prefix(path_prefix)
            .unwrap()
            .with_file_name(
                input_file_name
                    .to_str()
                    .unwrap()
                    .replace(".glsl.", ".spirv."),
            ),
    );

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
        "comp" => Ok(shaderc::ShaderKind::Compute),
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

fn main() {
    println!("Running build script");

    let dirs = [
        "src/compute_shader_test/shaders",
        "src/editor/shaders",
        "src/gltf_loader/shaders",
        "src/pbr_test/shaders",
    ];

    for dir in dirs {
        build_shaders(PathBuf::from(dir));
    }
}
