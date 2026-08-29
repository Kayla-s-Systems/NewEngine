#![forbid(unsafe_op_in_unsafe_fn)]

use libloading::Library;
use newengine_platform_api::{
    PlatformRuntimeDescriptorV1, PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL,
};
use newengine_plugin_api::{
    bootstrap_phase_to_u8, plugin_kind_to_u8, PluginDescriptorV2, PluginDiscoveryDescriptorV1,
    PluginDiscoveryManifestV2, PluginDiscoveryPlatformRuntimeV1, PluginDiscoverySignatureV1,
    PluginRootV1Ref, PluginSignatureV1, LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL,
    PLUGIN_DESCRIPTOR_V2_SYMBOL_BYTES_NUL, PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE,
    PLUGIN_DISCOVERY_EMBEDDED_MAGIC, PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
    PLUGIN_ROOT_SYMBOL_BYTES_NUL,
};
use std::{
    env, fs,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1\0";
const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";
const MAX_EMBEDDED_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

fn main() {
    if let Err(error) = run() {
        eprintln!("[plugin-manifest] {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let first = args.next().ok_or(
        "usage: newengine-plugin-manifest-tool --embed <dll> | --verify-embedded <dll> | --extract <dll> <output> | <dll> <output> | --verify <dll> <manifest>",
    )?;

    if first == "--embed" {
        let dll = single_path_arg(&mut args, "--embed <dll>")?;
        return embed(&dll);
    }
    if first == "--verify-embedded" {
        let dll = single_path_arg(&mut args, "--verify-embedded <dll>")?;
        return verify_embedded(&dll);
    }
    if first == "--extract" {
        let dll = next_path(&mut args, "--extract <dll> <output>")?;
        let out = next_path(&mut args, "--extract <dll> <output>")?;
        ensure_no_more(args)?;
        return extract(&dll, &out);
    }
    if first == "--verify" {
        let dll = next_path(&mut args, "--verify <dll> <manifest>")?;
        let manifest = next_path(&mut args, "--verify <dll> <manifest>")?;
        ensure_no_more(args)?;
        return verify_external(&dll, &manifest);
    }

    let dll = PathBuf::from(first);
    let out = next_path(&mut args, "<dll> <output>")?;
    ensure_no_more(args)?;
    emit_external(&dll, &out)
}

fn single_path_arg(
    args: &mut impl Iterator<Item = std::ffi::OsString>,
    usage: &str,
) -> Result<PathBuf, String> {
    let path = next_path(args, usage)?;
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    Ok(path)
}

fn next_path(
    args: &mut impl Iterator<Item = std::ffi::OsString>,
    usage: &str,
) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("usage: newengine-plugin-manifest-tool {usage}"))
}

fn ensure_no_more(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<(), String> {
    if args.next().is_some() {
        Err("too many arguments".to_owned())
    } else {
        Ok(())
    }
}

fn inspect(dll: &Path) -> Result<PluginDiscoveryManifestV2, String> {
    let artifact_file = dll
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("DLL file name is not UTF-8")?
        .to_owned();

    let lib = unsafe { Library::new(dll) }.map_err(|e| format!("open '{}': {e}", dll.display()))?;
    let has_canonical_root = unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL_BYTES_NUL)
    }
    .is_ok();
    let has_legacy_root = unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL)
    }
    .is_ok();
    let is_platform_runtime =
        unsafe { lib.get::<unsafe extern "C" fn()>(PLATFORM_RUNTIME_SYMBOL) }.is_ok();

    let signature =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(SIGNATURE_SYMBOL) }
            .ok()
            .map(|sym| unsafe { sym() })
            .map(|s| PluginDiscoverySignatureV1 {
                id: s.id.to_string(),
                name: s.name.to_string(),
                version: s.version.to_string(),
                kind: plugin_kind_to_u8(s.kind),
                bootstrap_phase: bootstrap_phase_to_u8(s.bootstrap_phase),
            });

    let typed = unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginDescriptorV2>(
            PLUGIN_DESCRIPTOR_V2_SYMBOL_BYTES_NUL,
        )
    }
    .ok()
    .map(|sym| unsafe { sym() });
    let typed = match typed {
        Some(v) => Some(v),
        None if has_canonical_root => {
            let sym = unsafe {
                lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL_BYTES_NUL)
            }
            .map_err(|e| format!("canonical root disappeared: {e}"))?;
            let root = unsafe { sym() };
            let module = (root.create())();
            let descriptor = module.descriptor();
            drop(module);
            Some(PluginDescriptorV2::from_legacy(&descriptor))
        }
        None => None,
    };
    let descriptor = typed
        .as_ref()
        .map(PluginDiscoveryDescriptorV1::from_descriptor_v2);

    let platform_runtime = if is_platform_runtime {
        unsafe {
            lib.get::<extern "C" fn() -> PlatformRuntimeDescriptorV1>(
                PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL,
            )
        }
        .ok()
        .map(|sym| sym())
        .map(|d| PluginDiscoveryPlatformRuntimeV1 {
            id: d.id.to_string(),
            name: d.name.to_string(),
            version: d.version.to_string(),
            backend_priority: d.backend_priority,
            system_tags: d.system_tags.iter().map(|v| v.to_string()).collect(),
        })
        .or_else(|| {
            signature
                .as_ref()
                .map(|s| PluginDiscoveryPlatformRuntimeV1 {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    version: s.version.clone(),
                    backend_priority: 0,
                    system_tags: Vec::new(),
                })
        })
    } else {
        None
    };

    if signature.is_none() && descriptor.is_none() && platform_runtime.is_none() {
        return Err("binary exposes no supported NorthStar discovery metadata".to_owned());
    }

    Ok(PluginDiscoveryManifestV2 {
        schema_version: PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
        artifact_file,
        signature,
        descriptor,
        platform_runtime,
        has_canonical_root,
        has_legacy_root,
    })
}

fn embedded_manifest(dll: &Path) -> Result<Option<(PluginDiscoveryManifestV2, u64)>, String> {
    let mut file = fs::File::open(dll).map_err(|e| format!("open '{}': {e}", dll.display()))?;
    let file_len = file
        .metadata()
        .map_err(|e| format!("metadata '{}': {e}", dll.display()))?
        .len();
    if file_len < PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE {
        return Ok(None);
    }

    file.seek(SeekFrom::End(
        -(PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE as i64),
    ))
    .map_err(|e| format!("seek embedded footer '{}': {e}", dll.display()))?;
    let mut len_bytes = [0u8; 8];
    let mut magic = [0u8; 16];
    file.read_exact(&mut len_bytes)
        .map_err(|e| format!("read embedded length '{}': {e}", dll.display()))?;
    file.read_exact(&mut magic)
        .map_err(|e| format!("read embedded magic '{}': {e}", dll.display()))?;
    if &magic != PLUGIN_DISCOVERY_EMBEDDED_MAGIC {
        return Ok(None);
    }

    let payload_len = u64::from_le_bytes(len_bytes);
    if payload_len == 0 || payload_len > MAX_EMBEDDED_MANIFEST_BYTES {
        return Err(format!(
            "invalid embedded manifest length={} path='{}'",
            payload_len,
            dll.display()
        ));
    }
    let trailer_len = payload_len
        .checked_add(PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE)
        .ok_or_else(|| "embedded manifest length overflow".to_owned())?;
    if trailer_len > file_len {
        return Err(format!(
            "embedded manifest exceeds artifact length path='{}'",
            dll.display()
        ));
    }
    let payload_start = file_len - trailer_len;
    file.seek(SeekFrom::Start(payload_start))
        .map_err(|e| format!("seek embedded manifest '{}': {e}", dll.display()))?;
    let mut payload = vec![0u8; payload_len as usize];
    file.read_exact(&mut payload)
        .map_err(|e| format!("read embedded manifest '{}': {e}", dll.display()))?;
    let manifest: PluginDiscoveryManifestV2 = serde_json::from_slice(&payload)
        .map_err(|e| format!("invalid embedded manifest '{}': {e}", dll.display()))?;
    Ok(Some((manifest, payload_start)))
}

fn embed(dll: &Path) -> Result<(), String> {
    let manifest = inspect(dll)?;
    let json = serde_json::to_vec(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    if json.is_empty() || json.len() as u64 > MAX_EMBEDDED_MANIFEST_BYTES {
        return Err(format!("embedded manifest size {} is invalid", json.len()));
    }

    let base_len = embedded_manifest(dll)?.map_or_else(
        || {
            fs::metadata(dll)
                .map(|m| m.len())
                .map_err(|e| format!("metadata '{}': {e}", dll.display()))
        },
        |(_, payload_start)| Ok(payload_start),
    )?;

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(dll)
        .map_err(|e| format!("open '{}' for embed: {e}", dll.display()))?;
    file.set_len(base_len).map_err(|e| {
        format!(
            "truncate existing embedded metadata '{}': {e}",
            dll.display()
        )
    })?;
    file.seek(SeekFrom::End(0))
        .map_err(|e| format!("seek '{}' for embed: {e}", dll.display()))?;
    file.write_all(&json)
        .map_err(|e| format!("write embedded manifest '{}': {e}", dll.display()))?;
    file.write_all(&(json.len() as u64).to_le_bytes())
        .map_err(|e| format!("write embedded length '{}': {e}", dll.display()))?;
    file.write_all(PLUGIN_DISCOVERY_EMBEDDED_MAGIC)
        .map_err(|e| format!("write embedded magic '{}': {e}", dll.display()))?;
    file.sync_all()
        .map_err(|e| format!("sync embedded manifest '{}': {e}", dll.display()))?;
    drop(file);

    verify_embedded(dll)?;
    println!(
        "[plugin-manifest] embedded {} bytes into {}",
        json.len(),
        dll.display()
    );
    Ok(())
}

fn verify_embedded(dll: &Path) -> Result<(), String> {
    let (embedded, _) = embedded_manifest(dll)?.ok_or_else(|| {
        format!(
            "embedded discovery manifest missing '{}': no trailer",
            dll.display()
        )
    })?;
    let actual = inspect(dll)?;
    if embedded != actual {
        return Err(format!(
            "embedded metadata does not match binary exports: artifact='{}'",
            dll.display()
        ));
    }
    println!(
        "[plugin-manifest] verified embedded metadata {}",
        dll.display()
    );
    Ok(())
}

fn extract(dll: &Path, out: &Path) -> Result<(), String> {
    let (manifest, _) = embedded_manifest(dll)?.ok_or_else(|| {
        format!(
            "embedded discovery manifest missing '{}': no trailer",
            dll.display()
        )
    })?;
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create '{}': {e}", parent.display()))?;
    }
    fs::write(out, format!("{json}\n")).map_err(|e| format!("write '{}': {e}", out.display()))?;
    println!(
        "[plugin-manifest] extracted {} -> {}",
        dll.display(),
        out.display()
    );
    Ok(())
}

// Legacy migration helpers. Release packaging no longer relies on external files.
fn emit_external(dll: &Path, out: &Path) -> Result<(), String> {
    let manifest = inspect(dll)?;
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create '{}': {e}", parent.display()))?;
    }
    fs::write(out, format!("{json}\n")).map_err(|e| format!("write '{}': {e}", out.display()))?;
    println!(
        "[plugin-manifest] emitted migration metadata {} -> {}",
        dll.display(),
        out.display()
    );
    Ok(())
}

fn verify_external(dll: &Path, manifest_path: &Path) -> Result<(), String> {
    let actual = inspect(dll)?;
    let bytes = fs::read(manifest_path)
        .map_err(|e| format!("read manifest '{}': {e}", manifest_path.display()))?;
    let expected: PluginDiscoveryManifestV2 = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid manifest '{}': {e}", manifest_path.display()))?;
    if expected != actual {
        return Err(format!(
            "external metadata does not match binary exports: manifest='{}' artifact='{}'",
            manifest_path.display(),
            dll.display()
        ));
    }
    println!(
        "[plugin-manifest] verified migration metadata {} <-> {}",
        manifest_path.display(),
        dll.display()
    );
    Ok(())
}
