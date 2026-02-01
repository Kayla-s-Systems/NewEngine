use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=shaders/tri.vert");
    println!("cargo:rerun-if-changed=shaders/tri.frag");
    println!("cargo:rerun-if-changed=shaders/text.vert");
    println!("cargo:rerun-if-changed=shaders/text.frag");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let compiler = shaderc::Compiler::new().unwrap();

    compile(
        &compiler,
        "shaders/tri.vert",
        shaderc::ShaderKind::Vertex,
        &out_dir,
        "tri.vert.spv",
    );
    compile(
        &compiler,
        "shaders/tri.frag",
        shaderc::ShaderKind::Fragment,
        &out_dir,
        "tri.frag.spv",
    );
    
    compile(
        &compiler,
        "shaders/text.vert",
        shaderc::ShaderKind::Vertex,
        &out_dir,
        "text.vert.spv",
    );
    compile(
        &compiler,
        "shaders/text.frag",
        shaderc::ShaderKind::Fragment,
        &out_dir,
        "text.frag.spv",
    );
}

fn compile(
    compiler: &shaderc::Compiler,
    path: &str,
    kind: shaderc::ShaderKind,
    out: &PathBuf,
    out_name: &str,
) {
    let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read shader '{path}': {e}"));

    let mut opts = shaderc::CompileOptions::new().expect("shaderc options");
    opts.set_optimization_level(shaderc::OptimizationLevel::Performance);

    let compiled = compiler
        .compile_into_spirv(&src, kind, path, "main", Some(&opts))
        .unwrap_or_else(|e| panic!("failed to compile shader '{path}': {e}"));

    fs::write(out.join(out_name), compiled.as_binary_u8())
        .unwrap_or_else(|e| panic!("failed to write '{out_name}': {e}"));
}