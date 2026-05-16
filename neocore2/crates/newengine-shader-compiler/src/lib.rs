#![forbid(unsafe_code)]
#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_render_api::ShaderStage;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};


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

    let cache_key = shader_cache_key(stage, logical_name, entry, source);
    let cache_path = shader_cache_path(stage, logical_name, entry, cache_key);
    if shader_runtime_cache_enabled() {
        match read_cached_spirv(&cache_path) {
            Ok(Some(words)) => return Ok(words),
            Ok(None) => {}
            Err(e) => {
                eprintln!(
                    "newengine-shader-compiler: ignoring corrupt shader cache path='{}' err='{e}'",
                    cache_path.display()
                );
                let _ = std::fs::remove_file(&cache_path);
            }
        }
    }

    let glslc = resolve_glslc();
    let stem = unique_stem(logical_name, stage);
    let temp_dir = shader_compile_temp_dir(&stem);
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        ShaderCompileError::new(format!(
            "shader temp dir create failed dir='{}' err='{e}'",
            temp_dir.display()
        ))
    })?;

    let source_path = temp_dir.join(format!("source.{}", stage_extension(stage)));
    let spv_path = temp_dir.join("output.spv");

    if let Err(e) = std::fs::write(&source_path, source) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(ShaderCompileError::new(format!(
            "shader temp source write failed path='{}' err='{e}'",
            source_path.display()
        )));
    }

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
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(ShaderCompileError::new(format!(
                "failed to execute glslc='{}' shader='{logical_name}' err='{e}'. Set NEWENGINE_GLSLC to a valid glslc executable or install Vulkan SDK.",
                display_command(&glslc)
            )));
        }
    };

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(ShaderCompileError::new(format!(
            "glslc failed shader='{logical_name}' status='{}' stdout='{}' stderr='{}'",
            output.status,
            stdout.trim(),
            stderr.trim()
        )));
    }

    let bytes = match std::fs::read(&spv_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(ShaderCompileError::new(format!(
                "shader SPIR-V read failed path='{}' err='{e}'",
                spv_path.display()
            )));
        }
    };

    let _ = std::fs::remove_dir_all(&temp_dir);

    let words = spirv_bytes_to_words(&bytes).map_err(|e| {
        ShaderCompileError::new(format!("shader='{logical_name}' invalid SPIR-V: {e}"))
    })?;

    if shader_runtime_cache_enabled() {
        if let Err(e) = write_cached_spirv(&cache_path, &words) {
            eprintln!(
                "newengine-shader-compiler: shader cache write failed path='{}' err='{e}'",
                cache_path.display()
            );
        }
    }

    Ok(words)
}

fn shader_runtime_cache_enabled() -> bool {
    !matches!(
        std::env::var("NEWENGINE_SHADER_RUNTIME_CACHE"),
        Ok(v) if v.eq_ignore_ascii_case("0")
            || v.eq_ignore_ascii_case("false")
            || v.eq_ignore_ascii_case("off")
            || v.eq_ignore_ascii_case("disabled")
    )
}

fn shader_cache_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("NEWENGINE_SHADER_CACHE_DIR") {
        return PathBuf::from(path);
    }
    cache_files_root().join("shaders").join("runtime")
}

fn cache_files_root() -> PathBuf {
    std::env::var_os("NEWENGINE_CACHE_FILES")
        .or_else(|| std::env::var_os("CACHE_FILES"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cache"))
}

fn shader_compile_temp_dir(stem: &str) -> PathBuf {
    // Keep transient GLSL sources out of the project cache. The compiler writes
    // into a unique OS-temp directory and removes it after each invocation, so
    // project-local temporary shader folders are not created during runtime.
    std::env::temp_dir().join("newengine-shader-compiler").join(stem)
}

fn shader_cache_path(stage: ShaderStage, logical_name: &str, entry: &str, key: u64) -> PathBuf {
    let filename = format!(
        "{}_{}_{}_{}.spv",
        sanitize_cache_component(logical_name, 80),
        stage_extension(stage),
        sanitize_cache_component(entry, 32),
        key
    );
    shader_cache_dir().join(filename)
}

fn sanitize_cache_component(value: &str, limit: usize) -> String {
    let mut clean = String::with_capacity(value.len().min(limit));
    for ch in value.chars().take(limit) {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            clean.push(ch);
        } else {
            clean.push('_');
        }
    }
    if clean.is_empty() {
        clean.push_str("shader");
    }
    clean
}

fn shader_cache_key(stage: ShaderStage, logical_name: &str, entry: &str, source: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    fn feed(hash: &mut u64, bytes: &[u8]) {
        for byte in bytes {
            *hash ^= u64::from(*byte);
            *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        *hash ^= 0xff;
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    feed(&mut hash, stage_extension(stage).as_bytes());
    feed(&mut hash, logical_name.as_bytes());
    feed(&mut hash, entry.as_bytes());
    feed(&mut hash, source.as_bytes());
    hash
}

fn read_cached_spirv(path: &std::path::Path) -> std::io::Result<Option<Vec<u32>>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    match spirv_bytes_to_words(&bytes) {
        Ok(words) => Ok(Some(words)),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
    }
}

fn write_cached_spirv(path: &std::path::Path, words: &[u32]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("spv.tmp");
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp, path).map_err(|_| e)
        }
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

    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if words.first().copied() != Some(0x0723_0203) {
        return Err("SPIR-V magic mismatch");
    }
    Ok(words)
}

fn display_command(cmd: &OsString) -> String {
    PathBuf::from(cmd).display().to_string()
}
