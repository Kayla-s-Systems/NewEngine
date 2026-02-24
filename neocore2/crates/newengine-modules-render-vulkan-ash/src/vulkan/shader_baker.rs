#![forbid(unsafe_op_in_unsafe_fn)]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{VkRenderError, VkResult};
use blake3::Hasher;
use newengine_assets::{wait_ready, AssetAccess, AssetService, AssetServiceClient};
use newengine_core::plugins::default_host_api;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
}

impl ShaderStage {
    #[inline]
    fn to_shaderc(self) -> shaderc::ShaderKind {
        match self {
            ShaderStage::Vertex => shaderc::ShaderKind::Vertex,
            ShaderStage::Fragment => shaderc::ShaderKind::Fragment,
        }
    }

    #[inline]
    fn suffix(self) -> &'static str {
        match self {
            ShaderStage::Vertex => "vert",
            ShaderStage::Fragment => "frag",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShaderPack {
    pub tri_vert: Vec<u32>,
    pub tri_frag: Vec<u32>,

    pub text_vert: Vec<u32>,
    pub text_frag: Vec<u32>,

    pub ui_vert: Vec<u32>,
    pub ui_frag: Vec<u32>,
}

/// Runtime GLSL -> SPIR-V baker with on-disk caching.
///
/// Rules:
/// - Source is always loaded through AssetManager (`asset.manager`).
/// - Baked results are stored on disk so shaders can be edited / created at runtime.
/// - Cache key includes shader source bytes + stage + entry point + compiler options.
pub struct ShaderBaker {
    assets: AssetServiceClient,
    cache_dir: PathBuf,
    compiler: shaderc::Compiler,
}

impl ShaderBaker {
    pub fn new() -> VkResult<Self> {
        let assets = AssetServiceClient::new(default_host_api());

        let cache_dir = shader_cache_dir();
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            return Err(VkRenderError::Shader(format!(
                "shader cache: create_dir_all failed dir='{}' err='{e}'",
                cache_dir.display()
            )));
        }

        let compiler = shaderc::Compiler::new()
            .map_err(|_| VkRenderError::Shader("shaderc: failed to create compiler".to_string()))?;

        Ok(Self {
            assets,
            cache_dir,
            compiler,
        })
    }

    pub fn bake_pack(&self) -> VkResult<ShaderPack> {
        // The core "tri" pipeline must be independent from editor shaders.
        // It uses no descriptor sets and no vertex buffers.
        Ok(ShaderPack {
            tri_vert: self.compile_inline_words(BUILTIN_TRI_VERT, "builtin_tri.vert", ShaderStage::Vertex, "main")?,
            tri_frag: self.compile_inline_words(BUILTIN_TRI_FRAG, "builtin_tri.frag", ShaderStage::Fragment, "main")?,

            text_vert: self.load_or_compile_words("shaders/ui/text.vert", ShaderStage::Vertex)?,
            text_frag: self.load_or_compile_words("shaders/ui/text.frag", ShaderStage::Fragment)?,

            ui_vert: self.load_or_compile_words("shaders/ui/ui.vert", ShaderStage::Vertex)?,
            ui_frag: self.load_or_compile_words("shaders/ui/ui.frag", ShaderStage::Fragment)?,
        })
    }

    fn compile_inline_words(
        &self,
        src: &str,
        logical_name: &str,
        stage: ShaderStage,
        entry: &str,
    ) -> VkResult<Vec<u32>> {
        let opt = shaderc::OptimizationLevel::Performance;
        let key = shader_cache_key(src, stage, entry, opt);
        let out_path = self.cache_dir.join(shader_cache_filename(logical_name, stage, &key));

        if let Ok(words) = read_spv_words(&out_path) {
            return Ok(words);
        }

        let words = self.compile_glsl_words(src, logical_name, stage, entry)?;
        let _ = write_spv_words(&out_path, &words);
        Ok(words)
    }
    pub fn load_or_compile_words(&self, logical_path: &str, stage: ShaderStage) -> VkResult<Vec<u32>> {
        let src = self.load_text_asset(logical_path)?;

        let key = shader_cache_key(&src, stage, "main", shaderc::OptimizationLevel::Performance);
        let out_path = self.cache_dir.join(shader_cache_filename(logical_path, stage, &key));

        if let Ok(words) = read_spv_words(&out_path) {
            return Ok(words);
        }

        let words = self.compile_glsl_words(&src, logical_path, stage, "main")?;
        if let Err(e) = write_spv_words(&out_path, &words) {
            log::warn!(
                "shader cache: write failed path='{}' err='{e}'",
                out_path.display()
            );
        }
        Ok(words)
    }

    fn load_text_asset(&self, logical_path: &str) -> VkResult<String> {
        // Some legacy/broken shader packs were built with an "assets/" prefix in NEPAK keys.
        // Keep strict VFS access (no raw fs), but tolerate alternate logical paths.
        let mut candidates: Vec<String> = Vec::with_capacity(2);
        candidates.push(logical_path.to_owned());

        if let Some(rest) = logical_path.strip_prefix("shaders/") {
            candidates.push(format!("assets/{rest}"));
        }

        let mut first_err: Option<VkRenderError> = None;

        for (i, cand) in candidates.iter().enumerate() {
            match self.load_text_asset_once(cand) {
                Ok(s) => {
                    if i != 0 {
                        log::warn!(
                            "shader: path remap '{}' -> '{}' (fix your pak to use 'shaders/' keys)",
                            logical_path,
                            cand
                        );
                    }
                    return Ok(s);
                }
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
        }

        Err(first_err.unwrap_or_else(|| {
            VkRenderError::Shader(format!("shader: no candidates for path='{logical_path}'"))
        }))
    }

    fn load_text_asset_once(&self, logical_path: &str) -> VkResult<String> {
        let id = self.assets.load(logical_path).map_err(|e| {
            VkRenderError::Shader(format!("asset.load failed path='{logical_path}' err='{e}'"))
        })?;

        log::debug!("shader: requesting '{}'", logical_path);

        if let Err(e) = wait_ready(&self.assets, &id, Duration::from_secs(5)) {
            let trace = self
                .assets
                .resolve_trace_json(logical_path)
                .map(|v| v.to_string())
                .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));

            return Err(VkRenderError::Shader(format!(
                "asset not ready path='{logical_path}' id='{id}' err='{e:?}' trace={trace}"
            )));
        }

        let (_meta, payload) = self.assets.blob_wire_v1(&id).map_err(|e| {
            VkRenderError::Shader(format!(
                "asset.blob_wire_v1 failed path='{logical_path}' id='{id}' err='{e}'"
            ))
        })?;

        let s = std::str::from_utf8(&payload).map_err(|_| {
            VkRenderError::Shader(format!("shader source is not utf8 path='{logical_path}'"))
        })?;

        Ok(s.to_string())
    }

    fn compile_glsl_words(
        &self,
        src: &str,
        logical_path: &str,
        stage: ShaderStage,
        entry: &str,
    ) -> VkResult<Vec<u32>> {
        let mut opts = shaderc::CompileOptions::new()
            .map_err(|_e| VkRenderError::Shader("shaderc: failed to create options".to_string()))?;
        opts.set_optimization_level(shaderc::OptimizationLevel::Performance);

        let artifact = self
            .compiler
            .compile_into_spirv(src, stage.to_shaderc(), logical_path, entry, Some(&opts))
            .map_err(|e| VkRenderError::Shader(format!("shaderc: compile failed path='{logical_path}' err='{e}'")))?;

        Ok(artifact.as_binary().to_vec())
    }
}

const BUILTIN_TRI_VERT: &str = r#"#version 450
vec2 POS[3] = vec2[](
    vec2(-1.0, -1.0),
    vec2( 3.0, -1.0),
    vec2(-1.0,  3.0)
);
void main() {
    gl_Position = vec4(POS[gl_VertexIndex], 0.0, 1.0);
}
"#;

const BUILTIN_TRI_FRAG: &str = r#"#version 450
layout(location = 0) out vec4 outColor;
void main() {
    outColor = vec4(0.05, 0.10, 0.20, 1.0);
}
"#;

fn shader_cache_dir() -> PathBuf {
    // Opt-in override for editor / CI.
    if let Some(v) = std::env::var_os("NEWENGINE_SHADER_CACHE_DIR") {
        return PathBuf::from(v);
    }

    // Keep it deterministic and local to the process working dir.
    PathBuf::from("cache").join("shaders")
}

fn shader_cache_key(
    src: &str,
    stage: ShaderStage,
    entry: &str,
    opt: shaderc::OptimizationLevel,
) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(stage.suffix().as_bytes());
    h.update(b"\0");
    h.update(entry.as_bytes());
    h.update(b"\0");
    h.update(format!("{opt:?}").as_bytes());
    h.update(b"\0");
    h.update(src.as_bytes());
    *h.finalize().as_bytes()
}

fn shader_cache_filename(logical_path: &str, stage: ShaderStage, key: &[u8; 32]) -> String {
    let stem = Path::new(logical_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("shader");

    let hash16 = hex16(key);
    format!("{stem}.{}.{}.spv", stage.suffix(), hash16)
}

fn hex16(key: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 32];
    for (i, b) in key[..16].iter().copied().enumerate() {
        out[i * 2] = HEX[(b >> 4) as usize];
        out[i * 2 + 1] = HEX[(b & 0x0F) as usize];
    }
    String::from_utf8_lossy(&out).to_string()
}

fn read_spv_words(path: &Path) -> Result<Vec<u32>, std::io::Error> {
    let mut f = fs::File::open(path)?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    if bytes.len() < 4 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "spv: too small",
        ));
    }

    if bytes.len() % 4 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "spv: unaligned",
        ));
    }

    // Magic number 0x07230203 (little endian).
    if bytes[0] != 0x03 || bytes[1] != 0x02 || bytes[2] != 0x23 || bytes[3] != 0x07 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "spv: bad magic",
        ));
    }

    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        out.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

fn write_spv_words(path: &Path, words: &[u32]) -> Result<(), std::io::Error> {
    let tmp = path.with_extension("spv.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        for &w in words {
            f.write_all(&w.to_le_bytes())?;
        }
        f.flush()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}
