#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use libloading::Library;
use newengine_core::host_events::{WindowHandles, WindowInitSize};
use newengine_core::render::{RenderApi, RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_plugin_api::{
    ConfigBlobV1, HostApiV1, RenderBackendDescriptorV1, RENDER_BACKEND_DESCRIBE_SYMBOL,
};
use serde_json::{Map, Value};

const RENDER_BACKEND_CREATE_SYMBOL: &[u8] = b"newengine_render_backend_create_v1\0";

pub const DEFAULT_RENDER_BACKEND_ID: &str = "newengine.renderer.vulkan";
pub const DEFAULT_RENDER_BACKEND_CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Debug, Clone)]
pub struct ResolvedRenderBackendConfig {
    pub backend_id: String,
    pub clear_color: [f32; 4],
    pub debug_text: String,
}

type RenderBackendCreateFn = unsafe fn(
    HostApiV1,
    raw_window_handle::RawDisplayHandle,
    raw_window_handle::RawWindowHandle,
    u32,
    u32,
    ConfigBlobV1,
) -> Result<Box<dyn RenderApi + 'static>, String>;

type RenderBackendDescribeFn = unsafe extern "C" fn() -> RenderBackendDescriptorV1;

#[derive(Debug, Clone)]
struct RenderBackendCandidate {
    path: PathBuf,
    id: String,
    name: String,
    version: String,
    aliases: Vec<String>,
    default_settings_json: String,
}

pub struct RenderBackendRuntimeModule {
    backend_spec: String,
    modules_dir: PathBuf,
    lib: Option<Library>,
    api: Option<RenderApiRef>,
    resolved_path: Option<PathBuf>,
}

impl RenderBackendRuntimeModule {
    #[inline]
    pub fn new(backend_spec: String, modules_dir: PathBuf) -> Self {
        Self {
            backend_spec,
            modules_dir,
            lib: None,
            api: None,
            resolved_path: None,
        }
    }
}

impl<E: Send + 'static> Module<E> for RenderBackendRuntimeModule {
    fn id(&self) -> &'static str {
        "render.runtime.loader"
    }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] {
        &[RENDER_API_PROVIDE]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let handles = *ctx
            .resources()
            .get::<WindowHandles>()
            .ok_or_else(|| EngineError::other("render backend: missing window handles"))?;
        let size = *ctx
            .resources()
            .get::<WindowInitSize>()
            .ok_or_else(|| EngineError::other("render backend: missing initial window size"))?;

        let candidate = detect_render_backend(&self.modules_dir, &self.backend_spec)?;
        let effective = resolve_render_backend_config(&candidate)?;

        log::info!(
            "render backend: loading id='{}' ver='{}' spec='{}' path='{}' size={}x{}",
            candidate.id,
            candidate.version,
            self.backend_spec,
            candidate.path.display(),
            size.width,
            size.height
        );
        log::info!(
            "render backend: effective config id='{}' clear_color={:.3},{:.3},{:.3},{:.3} debug_text='{}'",
            effective.backend_id,
            effective.clear_color[0],
            effective.clear_color[1],
            effective.clear_color[2],
            effective.clear_color[3],
            effective.debug_text
        );

        let lib = unsafe { Library::new(&candidate.path) }.map_err(|e| {
            EngineError::other(format!(
                "render backend load failed path='{}': {e}",
                candidate.path.display()
            ))
        })?;

        let api = {
            let create = unsafe { lib.get::<RenderBackendCreateFn>(RENDER_BACKEND_CREATE_SYMBOL) }
                .map_err(|e| {
                    EngineError::other(format!(
                        "render backend symbol '{}' missing in '{}': {e}",
                        String::from_utf8_lossy(RENDER_BACKEND_CREATE_SYMBOL)
                            .trim_end_matches('\0'),
                        candidate.path.display()
                    ))
                })?;

            let host = newengine_plugin_host::default_host_api();
            let effective_blob = config_blob_from_json_string(render_backend_effective_json(
                &candidate.default_settings_json,
                &newengine_plugin_host::get_plugin_overrides_with_env(&candidate.id),
            )?);

            unsafe {
                create(
                    host,
                    handles.display,
                    handles.window,
                    size.width,
                    size.height,
                    effective_blob,
                )
            }
                .map_err(|e| {
                    EngineError::other(format!(
                        "render backend create failed path='{}': {}",
                        candidate.path.display(),
                        e
                    ))
                })?
        };

        let api = RenderApiRef::from_box(api);
        ctx.resources_mut()
            .insert(effective.clone());
        ctx.resources_mut().register_api(RENDER_API_ID, api.clone())?;

        self.resolved_path = Some(candidate.path);
        self.api = Some(api);
        self.lib = Some(lib);
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx
            .resources_mut()
            .unregister_api::<RenderApiRef>(RENDER_API_ID);
        let _ = ctx.resources_mut().remove::<ResolvedRenderBackendConfig>();
        self.api = None;
        self.resolved_path = None;
        self.lib = None;
        Ok(())
    }
}

fn detect_render_backend(modules_dir: &Path, backend_spec: &str) -> EngineResult<RenderBackendCandidate> {
    let spec = backend_spec.trim();
    if spec.is_empty() {
        return Err(EngineError::other(
            "render backend spec is empty; expected alias, canonical id, or DLL path",
        ));
    }

    let direct = PathBuf::from(spec);
    if direct.is_file() {
        return inspect_render_backend(&direct)?.ok_or_else(|| {
            EngineError::other(format!(
                "render backend file '{}' is missing required symbol '{}'",
                direct.display(),
                String::from_utf8_lossy(RENDER_BACKEND_CREATE_SYMBOL).trim_end_matches('\0')
            ))
        });
    }

    let modules_path = resolve_modules_dir(modules_dir)?;
    let mut candidates: Vec<(i32, RenderBackendCandidate)> = Vec::new();

    let mut dynlibs: Vec<PathBuf> = std::fs::read_dir(&modules_path)
        .map_err(|e| {
            EngineError::other(format!(
                "render backend scan failed dir='{}': {e}",
                modules_path.display()
            ))
        })?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .filter(|path| is_dynamic_lib(path))
        .collect();

    dynlibs.sort();
    for path in dynlibs {
        let Some(candidate) = inspect_render_backend(&path)? else {
            continue;
        };

        let score = score_backend_candidate(&candidate, spec);
        if score > 0 {
            candidates.push((score, candidate));
        }
    }

    candidates.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.path.cmp(&b.path))
    });

    if let Some((_, candidate)) = candidates.into_iter().next() {
        return Ok(candidate);
    }

    Err(EngineError::other(format!(
        "render backend '{}' not found in '{}'",
        spec,
        modules_path.display()
    )))
}

fn resolve_render_backend_config(
    candidate: &RenderBackendCandidate,
) -> EngineResult<ResolvedRenderBackendConfig> {
    let effective = render_backend_effective_json(
        &candidate.default_settings_json,
        &newengine_plugin_host::get_plugin_overrides_with_env(&candidate.id),
    )?;

    Ok(ResolvedRenderBackendConfig {
        backend_id: candidate.id.clone(),
        clear_color: extract_clear_color(&effective),
        debug_text: extract_string_field(&effective, "debug_text")
            .unwrap_or_else(|| candidate.name.clone()),
    })
}

fn render_backend_effective_json(default_settings_json: &str, overrides: &Value) -> EngineResult<Value> {
    let mut effective = parse_json_object(default_settings_json, "render backend defaults")?;
    merge_json_replace(&mut effective, overrides);
    Ok(effective)
}

fn config_blob_from_json_string(value: Value) -> ConfigBlobV1 {
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    ConfigBlobV1 {
        content_type: "application/json".into(),
        bytes: bytes.into(),
        format_version: 1,
    }
}

fn parse_json_object(raw: &str, what: &str) -> EngineResult<Value> {
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|e| EngineError::other(format!("{what} parse failed: {e}")))?;
    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err(EngineError::other(format!("{what} must be a JSON object")))
    }
}

fn merge_json_replace(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(dst_map), Value::Object(src_map)) => {
            for (key, src_value) in src_map {
                match dst_map.get_mut(key) {
                    Some(dst_value) => merge_json_replace(dst_value, src_value),
                    None => {
                        dst_map.insert(key.clone(), src_value.clone());
                    }
                }
            }
        }
        (dst_value, src_value) => {
            *dst_value = src_value.clone();
        }
    }
}

fn extract_clear_color(value: &Value) -> [f32; 4] {
    let Some(arr) = value.get("clear_color").and_then(Value::as_array) else {
        return DEFAULT_RENDER_BACKEND_CLEAR_COLOR;
    };

    if arr.len() != 4 {
        return DEFAULT_RENDER_BACKEND_CLEAR_COLOR;
    }

    let mut out = DEFAULT_RENDER_BACKEND_CLEAR_COLOR;
    for (idx, item) in arr.iter().enumerate().take(4) {
        out[idx] = item.as_f64().unwrap_or(out[idx] as f64) as f32;
    }
    out
}

fn extract_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
}

fn inspect_render_backend(path: &Path) -> EngineResult<Option<RenderBackendCandidate>> {
    let lib = unsafe { Library::new(path) }.map_err(|e| {
        EngineError::other(format!(
            "render backend inspect failed path='{}': {e}",
            path.display()
        ))
    })?;

    if unsafe { lib.get::<RenderBackendCreateFn>(RENDER_BACKEND_CREATE_SYMBOL) }.is_err() {
        return Ok(None);
    }

    let descriptor = read_render_backend_descriptor(&lib)?;
    let fallback_id = path
        .file_stem()
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| DEFAULT_RENDER_BACKEND_ID.to_owned());

    Ok(Some(RenderBackendCandidate {
        path: path.to_path_buf(),
        id: descriptor
            .get("id")
            .and_then(Value::as_str)
            .filter(|it| !it.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or(fallback_id.clone()),
        name: descriptor
            .get("name")
            .and_then(Value::as_str)
            .filter(|it| !it.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| fallback_id.clone()),
        version: descriptor
            .get("version")
            .and_then(Value::as_str)
            .filter(|it| !it.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| "-".to_owned()),
        aliases: descriptor
            .get("aliases")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|it| !it.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        default_settings_json: descriptor
            .get("default_settings_json")
            .and_then(Value::as_str)
            .unwrap_or("{}")
            .to_owned(),
    }))
}

fn read_render_backend_descriptor(lib: &Library) -> EngineResult<Value> {
    let describe = unsafe { lib.get::<RenderBackendDescribeFn>(RENDER_BACKEND_DESCRIBE_SYMBOL) }
        .map_err(|e| {
            EngineError::other(format!(
                "render backend describe symbol '{}' missing: {e}",
                String::from_utf8_lossy(RENDER_BACKEND_DESCRIBE_SYMBOL).trim_end_matches('\0')
            ))
        })?;

    let raw = unsafe { describe() };
    let mut obj = Map::new();
    obj.insert(
        "id".to_owned(),
        Value::String(read_descriptor_field(raw.id_ptr, raw.id_len)),
    );
    obj.insert(
        "name".to_owned(),
        Value::String(read_descriptor_field(raw.name_ptr, raw.name_len)),
    );
    obj.insert(
        "version".to_owned(),
        Value::String(read_descriptor_field(raw.version_ptr, raw.version_len)),
    );
    obj.insert(
        "aliases".to_owned(),
        Value::Array(
            read_descriptor_field(raw.aliases_ptr, raw.aliases_len)
                .split(',')
                .map(str::trim)
                .filter(|it| !it.is_empty())
                .map(|it| Value::String(it.to_owned()))
                .collect(),
        ),
    );
    obj.insert(
        "default_settings_json".to_owned(),
        Value::String(read_descriptor_field(
            raw.default_settings_ptr,
            raw.default_settings_len,
        )),
    );
    Ok(Value::Object(obj))
}

fn read_descriptor_field(ptr: *const u8, len: usize) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8_lossy(bytes).trim().to_owned()
}

fn resolve_modules_dir(modules_dir: &Path) -> EngineResult<PathBuf> {
    let exe = std::env::current_exe()
        .map_err(|e| EngineError::other(format!("render backend: current_exe failed: {e}")))?;
    let exe_dir = exe.parent().ok_or_else(|| {
        EngineError::other("render backend: executable has no parent directory")
    })?;

    let path = if modules_dir.as_os_str().is_empty() || modules_dir == Path::new(".") {
        exe_dir.to_path_buf()
    } else if modules_dir.is_absolute() {
        modules_dir.to_path_buf()
    } else {
        exe_dir.join(modules_dir)
    };

    Ok(path)
}

#[inline]
fn is_dynamic_lib(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str).map(|v| v.to_ascii_lowercase()),
        Some(ext) if ext == "dll" || ext == "so" || ext == "dylib"
    )
}

fn score_backend_candidate(candidate: &RenderBackendCandidate, backend_spec: &str) -> i32 {
    let spec_norm = normalize_token_string(backend_spec);
    if spec_norm.is_empty() {
        return 0;
    }

    let id_norm = normalize_token_string(&candidate.id);
    let name_norm = normalize_token_string(&candidate.name);
    let path_norm = candidate
        .path
        .file_stem()
        .and_then(OsStr::to_str)
        .map(normalize_token_string)
        .unwrap_or_default();
    let alias_norms: Vec<String> = candidate
        .aliases
        .iter()
        .map(|it| normalize_token_string(it))
        .collect();

    if spec_norm == id_norm {
        return 10_000;
    }
    if alias_norms.iter().any(|it| *it == spec_norm) {
        return 9_000;
    }
    if spec_norm == path_norm {
        return 8_000;
    }

    let spec_tokens = split_tokens(&spec_norm);
    if spec_tokens.is_empty() {
        return 0;
    }

    let mut score = 0;
    for haystack in std::iter::once(&id_norm)
        .chain(std::iter::once(&name_norm))
        .chain(std::iter::once(&path_norm))
        .chain(alias_norms.iter())
    {
        let haystack_tokens = split_tokens(haystack);
        if spec_tokens
            .iter()
            .all(|token| haystack_tokens.iter().any(|it| it == token))
        {
            score = score.max(500 + haystack_tokens.len() as i32);
        }
    }

    score
}

fn normalize_token_string(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect()
}

fn split_tokens(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
        .collect()
}
