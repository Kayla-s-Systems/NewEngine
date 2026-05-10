#![forbid(unsafe_code)]
#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_render_api::ShaderStage;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod prebaked;

#[derive(Debug)]
pub struct ShaderCompileError {
    message: String,
}

impl ShaderCompileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ShaderCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ShaderCompileError {}

pub type ShaderCompileResult<T> = Result<T, ShaderCompileError>;

/// Compile GLSL/HLSL-compatible source to SPIR-V by invoking `glslc`.
///
/// Tool resolution order:
/// 1. `NEWENGINE_GLSLC`
/// 2. `VULKAN_SDK/Bin/glslc.exe` on Windows, `VULKAN_SDK/bin/glslc` elsewhere
/// 3. `glslc` from PATH
pub fn compile_glsl_to_spirv(
    stage: ShaderStage,
    logical_name: &str,
    entry: &str,
    source: &str,
) -> ShaderCompileResult<Vec<u32>> {
    if entry != "main" {
        return Err(ShaderCompileError::new(format!(
            "glslc adapter currently supports only entry='main', got entry='{entry}' shader='{logical_name}'"
        )));
    }

    if shader_bake_mode_allows_prebaked() {
        if let Some(words) = prebaked::lookup(stage, logical_name, entry) {
            return Ok(words);
        }
    }

    let glslc = resolve_glslc();
    let temp_dir = std::env::temp_dir().join("newengine-shaders");
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        ShaderCompileError::new(format!(
            "shader temp dir create failed dir='{}' err='{e}'",
            temp_dir.display()
        ))
    })?;

    let stem = unique_stem(logical_name, stage);
    let source_path = temp_dir.join(format!("{stem}.{}", stage_extension(stage)));
    let spv_path = temp_dir.join(format!("{stem}.spv"));

    std::fs::write(&source_path, source).map_err(|e| {
        ShaderCompileError::new(format!(
            "shader temp source write failed path='{}' err='{e}'",
            source_path.display()
        ))
    })?;

    let output = match Command::new(&glslc)
        .arg("-O")
        .arg(format!("-fshader-stage={}", glslc_stage(stage)))
        .arg(&source_path)
        .arg("-o")
        .arg(&spv_path)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            if shader_bake_mode_allows_error_fallback() {
                if let Some(words) = prebaked::lookup(stage, logical_name, entry) {
                    let _ = std::fs::remove_file(&source_path);
                    let _ = std::fs::remove_file(&spv_path);
                    return Ok(words);
                }
            }
            return Err(ShaderCompileError::new(format!(
                "failed to execute glslc='{}' shader='{logical_name}' err='{e}'. Set NEWENGINE_GLSLC to a valid glslc executable or install Vulkan SDK.",
                display_command(&glslc)
            )));
        }
    };

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&source_path);
        let _ = std::fs::remove_file(&spv_path);
        if shader_bake_mode_allows_error_fallback() {
            if let Some(words) = prebaked::lookup(stage, logical_name, entry) {
                return Ok(words);
            }
        }
        return Err(ShaderCompileError::new(format!(
            "glslc failed shader='{logical_name}' status='{}' stdout='{}' stderr='{}'",
            output.status,
            stdout.trim(),
            stderr.trim()
        )));
    }

    let bytes = std::fs::read(&spv_path).map_err(|e| {
        ShaderCompileError::new(format!(
            "shader SPIR-V read failed path='{}' err='{e}'",
            spv_path.display()
        ))
    })?;

    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&spv_path);

    spirv_bytes_to_words(&bytes).map_err(|e| {
        ShaderCompileError::new(format!("shader='{logical_name}' invalid SPIR-V: {e}"))
    })
}

#[inline]
fn shader_bake_mode_allows_prebaked() -> bool {
    match std::env::var("NEWENGINE_SHADER_BAKE_MODE") {
        Ok(v) if v.eq_ignore_ascii_case("runtime") || v.eq_ignore_ascii_case("glslc") => false,
        _ => true,
    }
}

fn shader_bake_mode_allows_error_fallback() -> bool {
    match std::env::var("NEWENGINE_SHADER_BAKE_MODE") {
        Ok(v) if v.eq_ignore_ascii_case("strict-runtime") || v.eq_ignore_ascii_case("strict-glslc") => false,
        _ => true,
    }
}

fn resolve_glslc() -> OsString {
    if let Some(path) = std::env::var_os("NEWENGINE_GLSLC") {
        return path;
    }

    if let Some(sdk) = std::env::var_os("VULKAN_SDK") {
        let mut path = PathBuf::from(sdk);
        if cfg!(windows) {
            path.push("Bin");
            path.push("glslc.exe");
        } else {
            path.push("bin");
            path.push("glslc");
        }
        return path.into_os_string();
    }

    OsString::from("glslc")
}

fn unique_stem(logical_name: &str, stage: ShaderStage) -> String {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();

    let mut clean = String::with_capacity(logical_name.len().min(48));
    for ch in logical_name.chars().take(48) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            clean.push(ch);
        } else {
            clean.push('_');
        }
    }

    format!("{clean}_{}_{}_{}", stage_extension(stage), pid, nanos)
}

fn glslc_stage(stage: ShaderStage) -> &'static str {
    match stage {
        ShaderStage::Vertex => "vertex",
        ShaderStage::Fragment => "fragment",
        ShaderStage::Compute => "compute",
    }
}

fn stage_extension(stage: ShaderStage) -> &'static str {
    match stage {
        ShaderStage::Vertex => "vert",
        ShaderStage::Fragment => "frag",
        ShaderStage::Compute => "comp",
    }
}

fn spirv_bytes_to_words(bytes: &[u8]) -> Result<Vec<u32>, &'static str> {
    if bytes.len() % 4 != 0 {
        return Err("byte length is not divisible by 4");
    }

    Ok(bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn display_command(cmd: &OsString) -> String {
    PathBuf::from(cmd).display().to_string()
}
