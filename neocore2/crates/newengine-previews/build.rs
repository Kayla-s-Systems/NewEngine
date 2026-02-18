use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=shaders/primitive_preview.vert");
    println!("cargo:rerun-if-changed=shaders/primitive_preview.frag");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let compiler = shaderc::Compiler::new().expect("shaderc compiler");

    compile(
        &compiler,
        "shaders/primitive_preview.vert",
        shaderc::ShaderKind::Vertex,
        &out_dir,
        "primitive_preview.vert.spv",
    );
    compile(
        &compiler,
        "shaders/primitive_preview.frag",
        shaderc::ShaderKind::Fragment,
        &out_dir,
        "primitive_preview.frag.spv",
    );
}

fn compile(
    compiler: &shaderc::Compiler,
    path: &str,
    kind: shaderc::ShaderKind,
    out_dir: &Path,
    out_name: &str,
) {
    let src = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read shader '{path}': {e}"));

    let mut opts = shaderc::CompileOptions::new().expect("shaderc options");
    opts.set_optimization_level(shaderc::OptimizationLevel::Performance);

    let compiled = compiler
        .compile_into_spirv(&src, kind, path, "main", Some(&opts))
        .unwrap_or_else(|e| panic!("failed to compile shader '{path}': {e}"));

    fs::write(out_dir.join(out_name), compiled.as_binary_u8())
        .unwrap_or_else(|e| panic!("failed to write '{out_name}': {e}"));
}
