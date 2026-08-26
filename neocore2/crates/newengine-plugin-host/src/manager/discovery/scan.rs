#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use super::graph::{DiscoveryGraph, ScannedDynlib, ScannedDynlibKind};
use super::metadata::{build_scanned_plugin_kind, ScanPluginProbe};
use super::sidecar::read_verified_manifest;
use crate::manager::types::PluginLoadError;
use crate::paths::is_dynamic_lib;
use newengine_ulog_api::path_format::display_clean;

pub(super) fn scan_plugins_dir(dir: &Path) -> Result<DiscoveryGraph, PluginLoadError> {
    let rd = std::fs::read_dir(dir).map_err(|e| PluginLoadError {
        path: dir.to_path_buf(),
        message: format!("read_dir failed: {e}"),
    })?;

    let mut entries_total: usize = 0;
    let mut skipped_non_dynlib: usize = 0;
    let mut dynlib_paths: Vec<PathBuf> = Vec::new();
    let mut scan_errors: Vec<String> = Vec::new();
    for ent in rd {
        entries_total = entries_total.saturating_add(1);

        let ent = ent.map_err(|e| PluginLoadError {
            path: dir.to_path_buf(),
            message: format!("read_dir entry failed: {e}"),
        })?;

        let path = ent.path();
        if !is_dynamic_lib(&path) {
            skipped_non_dynlib = skipped_non_dynlib.saturating_add(1);
            continue;
        }

        dynlib_paths.push(path);
    }

    dynlib_paths.sort_by_key(|a| sort_key(a));

    let mut items: Vec<ScannedDynlib> = Vec::with_capacity(dynlib_paths.len());

    for path in dynlib_paths {
        match scan_dynamic_lib(&path) {
            Ok(v) => items.push(v),
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "plugins: scan failed for '{}': {}",
                    display_clean(&path),
                    e
                );
                scan_errors.push(format!("{}: {}", display_clean(&path), e));
            }
        }
    }

    items.sort_by(|a, b| sort_key(&a.path).cmp(&sort_key(&b.path)));

    let mut platform_runtime_count = 0usize;
    let mut bootstrap_total = 0usize;
    let mut engine_total = 0usize;
    let mut unknown_dynlibs: Vec<String> = Vec::new();

    for item in &items {
        match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => {
                platform_runtime_count = platform_runtime_count.saturating_add(1);
            }
            ScannedDynlibKind::Plugin { phase, .. } => match phase {
                newengine_plugin_api::PluginBootstrapPhase::Bootstrap => {
                    bootstrap_total = bootstrap_total.saturating_add(1);
                }
                newengine_plugin_api::PluginBootstrapPhase::Platform
                | newengine_plugin_api::PluginBootstrapPhase::Engine => {
                    engine_total = engine_total.saturating_add(1);
                }
            },
            ScannedDynlibKind::Unknown => {
                unknown_dynlibs.push(item.file_name.clone());
            }
        }
    }

    Ok(DiscoveryGraph {
        dir: dir.to_path_buf(),
        entries_total,
        skipped_non_dynlib,
        items,
        scan_errors,
        platform_runtime_count,
        bootstrap_total,
        engine_total,
        unknown_dynlibs,
    })
}

fn scan_dynamic_lib(path: &Path) -> Result<ScannedDynlib, String> {
    let file_name = file_name_only(path);
    let manifest = read_verified_manifest(path)?;

    if let Some(platform) = manifest.platform_runtime.as_ref() {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            discovery_manifest: Some(manifest.clone()),
            kind: ScannedDynlibKind::PlatformRuntime {
                id: platform.id.clone(),
                version: platform.version.clone(),
                system_tags: platform.system_tags.clone(),
                backend_priority: platform.backend_priority,
            },
        });
    }

    let signature = manifest
        .signature
        .as_ref()
        .map(|signature| {
            Ok::<newengine_plugin_api::PluginSignatureV1, String>(
                newengine_plugin_api::PluginSignatureV1 {
                    id: signature.id.clone().into(),
                    name: signature.name.clone().into(),
                    version: signature.version.clone().into(),
                    kind: newengine_plugin_api::plugin_kind_from_u8(signature.kind)?,
                    bootstrap_phase: newengine_plugin_api::bootstrap_phase_from_u8(
                        signature.bootstrap_phase,
                    )?,
                },
            )
        })
        .transpose()?;
    let descriptor_v2 = manifest
        .descriptor
        .as_ref()
        .map(newengine_plugin_api::PluginDiscoveryDescriptorV1::to_descriptor_v2)
        .transpose()?;
    let probe = ScanPluginProbe {
        signature,
        info: None,
        descriptor: None,
        descriptor_v2,
        has_canonical_root: manifest.has_canonical_root,
        has_legacy_root: manifest.has_legacy_root,
    };

    if let Some(kind) = build_scanned_plugin_kind(&probe) {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            discovery_manifest: Some(manifest.clone()),
            kind,
        });
    }

    Ok(ScannedDynlib {
        path: path.to_path_buf(),
        file_name,
        discovery_manifest: Some(manifest),
        kind: ScannedDynlibKind::Unknown,
    })
}

#[inline]
fn file_name_only(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<unnamed>".to_owned())
}

#[inline]
fn sort_key(path: &Path) -> (String, String) {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<unnamed>".to_owned());
    (file_name, display_clean(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_plugin_api::{
        bootstrap_phase_to_u8, plugin_kind_to_u8, PluginBootstrapPhase, PluginDescriptorV2,
        PluginDiscoveryDescriptorV1, PluginDiscoveryManifestV1, PluginDiscoverySignatureV1,
        PluginKind, PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
    };
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn discovery_uses_sidecar_without_mapping_binary() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("northstar-zero-map-{nonce}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let dll = dir.join("fake-provider.dll");
        // Deliberately not a PE/ELF/Mach-O image. Any Library::new() in discovery
        // would make this regression fail.
        let bytes = b"northstar-sidecar-only-not-a-dynamic-library";
        std::fs::write(&dll, bytes).expect("fake dll");

        let descriptor = PluginDescriptorV2 {
            id: "test.fake.provider".into(),
            name: "Fake Provider".into(),
            version: "1.2.3".into(),
            kind: PluginKind::Runtime,
            capabilities: Vec::new().into(),
            extension_json: "".into(),
        };
        let manifest = PluginDiscoveryManifestV1 {
            schema_version: PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
            artifact_file: "fake-provider.dll".to_owned(),
            artifact_size: bytes.len() as u64,
            artifact_sha256: format!("{:x}", Sha256::digest(bytes)),
            signature: Some(PluginDiscoverySignatureV1 {
                id: "test.fake.provider".to_owned(),
                name: "Fake Provider".to_owned(),
                version: "1.2.3".to_owned(),
                kind: plugin_kind_to_u8(PluginKind::Runtime),
                bootstrap_phase: bootstrap_phase_to_u8(PluginBootstrapPhase::Engine),
            }),
            descriptor: Some(PluginDiscoveryDescriptorV1::from_descriptor_v2(&descriptor)),
            platform_runtime: None,
            has_canonical_root: true,
            has_legacy_root: false,
        };
        let sidecar = dll.with_extension(newengine_plugin_api::PLUGIN_DISCOVERY_MANIFEST_SUFFIX);
        std::fs::write(
            &sidecar,
            serde_json::to_vec_pretty(&manifest).expect("json"),
        )
        .expect("sidecar");

        let scanned = scan_dynamic_lib(&dll).expect("sidecar-only discovery must succeed");
        assert_eq!(scanned.discovery_manifest.as_ref(), Some(&manifest));
        match scanned.kind {
            ScannedDynlibKind::Plugin { id, version, .. } => {
                assert_eq!(id, "test.fake.provider");
                assert_eq!(version, "1.2.3");
            }
            other => panic!("expected plugin, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(dir);
    }
}
