use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let shader_dir = PathBuf::from("shaders");

    compile(
        &shader_dir.join("tri.vert"),
        shaderc::ShaderKind::Vertex,
        &out_dir.join("tri.vert.spv"),
    );
    compile(
        &shader_dir.join("tri.frag"),
        shaderc::ShaderKind::Fragment,
        &out_dir.join("tri.frag.spv"),
    );

    println!("cargo:rerun-if-changed=shaders/tri.vert");
    println!("cargo:rerun-if-changed=shaders/tri.frag");
}

fn compile(src: &PathBuf, kind: shaderc::ShaderKind, dst: &PathBuf) {
    let source = fs::read_to_string(src).unwrap();

    let mut compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);

    let artifact = compiler
        .compile_into_spirv(&source, kind, src.to_str().unwrap(), "main", Some(&options))
        .unwrap();

    fs::write(dst, artifact.as_binary_u8()).unwrap();
}