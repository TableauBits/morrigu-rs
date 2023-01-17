use std::{
    fs::DirEntry,
    io::{self, Write},
};

fn recursive_compile(dir: &str) {
    for entry in std::fs::read_dir(dir).expect("Failed to read shader source folder") {
        let entry = entry.expect("Failed to iterate over files");

        if entry.file_type().unwrap().is_dir() {
            recursive_compile(entry.path().to_str().unwrap());
        } else {
            compile_shader(Ok(entry));
        }
    }
}

fn compile_shader(entry: Result<DirEntry, io::Error>) {
    let entry = entry.expect("Failed to iterate over files");

    if !entry.file_type().unwrap().is_file() {
        return;
    }

    let input_file_path = entry.path();
    let input_file_name = input_file_path.file_name().expect("Invalid file name");
    let output_file_path = std::path::Path::new("assets/gen").join(
        input_file_path
            .strip_prefix("assets/src")
            .expect("Invalid file structure")
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

    let compiler = shaderc::Compiler::new().expect("Failed to create shaderc compiler");
    let compiled_spirv = compiler
        .compile_into_spirv(
            source.as_str(),
            shader_type,
            input_file_name.to_str().expect("Invalid file name"),
            "main",
            None,
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
    println!("cargo:rerun-if-changed=assets/src/shaders/");
    println!("cargo:rerun-if-changed=assets/gen/shaders/");
    println!("Running build script");

    if let Err(error) = std::fs::remove_dir_all("assets/gen/shaders") {
        println!("cargo:warning=Failed to clean shader output folder {error}");
    }
    std::fs::create_dir_all("assets/gen/shaders").expect("Failed to create shader output folder");
    recursive_compile("assets/src/shaders");
}
