#![forbid(unsafe_op_in_unsafe_fn)]

use libloading::Library;
use newengine_platform_api::{
    PlatformRuntimeDescriptorV1, PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL,
};
use newengine_plugin_api::{
    bootstrap_phase_to_u8, plugin_kind_to_u8, PluginDescriptorV2,
    PluginDiscoveryDescriptorV1, PluginDiscoveryManifestV1, PluginDiscoveryPlatformRuntimeV1,
    PluginDiscoverySignatureV1, PluginRootV1Ref, PluginSignatureV1,
    LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL, PLUGIN_DESCRIPTOR_V2_SYMBOL_BYTES_NUL,
    PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION, PLUGIN_ROOT_SYMBOL_BYTES_NUL,
};
use sha2::{Digest, Sha256};
use std::{env, fs, path::{Path, PathBuf}};

const SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1\0";
const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";

fn main() {
    if let Err(error) = run() {
        eprintln!("[plugin-manifest] {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let dll = args.next().map(PathBuf::from).ok_or("usage: newengine-plugin-manifest-tool <dll> <output>")?;
    let out = args.next().map(PathBuf::from).ok_or("usage: newengine-plugin-manifest-tool <dll> <output>")?;
    if args.next().is_some() { return Err("too many arguments".to_owned()); }
    emit(&dll, &out)
}

fn emit(dll: &Path, out: &Path) -> Result<(), String> {
    let bytes = fs::read(dll).map_err(|e| format!("read '{}': {e}", dll.display()))?;
    let artifact_size = bytes.len() as u64;
    let artifact_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let artifact_file = dll.file_name().and_then(|v|v.to_str()).ok_or("DLL file name is not UTF-8")?.to_owned();

    let lib = unsafe { Library::new(dll) }.map_err(|e| format!("open '{}': {e}", dll.display()))?;
    let has_canonical_root = unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL_BYTES_NUL) }.is_ok();
    let has_legacy_root = unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL) }.is_ok();
    let is_platform_runtime = unsafe { lib.get::<unsafe extern "C" fn()>(PLATFORM_RUNTIME_SYMBOL) }.is_ok();

    let signature = unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(SIGNATURE_SYMBOL) }
        .ok().map(|sym| unsafe { sym() }).map(|s| PluginDiscoverySignatureV1 {
            id:s.id.to_string(), name:s.name.to_string(), version:s.version.to_string(),
            kind:plugin_kind_to_u8(s.kind), bootstrap_phase:bootstrap_phase_to_u8(s.bootstrap_phase),
        });

    let typed = unsafe { lib.get::<unsafe extern "C" fn() -> PluginDescriptorV2>(PLUGIN_DESCRIPTOR_V2_SYMBOL_BYTES_NUL) }
        .ok().map(|sym| unsafe { sym() });
    let typed = match typed {
        Some(v) => Some(v),
        None if has_canonical_root => {
            let sym = unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL_BYTES_NUL) }
                .map_err(|e| format!("canonical root disappeared: {e}"))?;
            let root = unsafe { sym() };
            let module = (root.create())();
            let descriptor = module.descriptor();
            drop(module);
            Some(PluginDescriptorV2::from_legacy(&descriptor))
        }
        None => None,
    };
    let descriptor = typed.as_ref().map(PluginDiscoveryDescriptorV1::from_descriptor_v2);

    let platform_runtime = if is_platform_runtime {
        unsafe { lib.get::<extern "C" fn() -> PlatformRuntimeDescriptorV1>(PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL) }
            .ok().map(|sym| sym()).map(|d| PluginDiscoveryPlatformRuntimeV1 {
                id:d.id.to_string(), name:d.name.to_string(), version:d.version.to_string(), backend_priority:d.backend_priority,
                system_tags:d.system_tags.iter().map(|v|v.to_string()).collect(),
            }).or_else(|| signature.as_ref().map(|s| PluginDiscoveryPlatformRuntimeV1 {
                id:s.id.clone(), name:s.name.clone(), version:s.version.clone(), backend_priority:0, system_tags:Vec::new(),
            }))
    } else { None };

    if signature.is_none() && descriptor.is_none() && platform_runtime.is_none() {
        return Err("binary exposes no supported NorthStar discovery metadata".to_owned());
    }
    let manifest = PluginDiscoveryManifestV1 {
        schema_version: PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
        artifact_file, artifact_size, artifact_sha256, signature, descriptor, platform_runtime,
        has_canonical_root, has_legacy_root,
    };
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    if let Some(parent)=out.parent(){ fs::create_dir_all(parent).map_err(|e|format!("create '{}': {e}",parent.display()))?; }
    fs::write(out, format!("{json}\n")).map_err(|e|format!("write '{}': {e}",out.display()))?;
    println!("[plugin-manifest] {} -> {} sha256={}", dll.display(), out.display(), manifest.artifact_sha256);
    Ok(())
}
