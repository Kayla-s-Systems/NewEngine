#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{
    PluginDescriptorV2, PluginDiscoveryDescriptorV1, PluginDiscoveryManifestV1,
    PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION, PLUGIN_DISCOVERY_MANIFEST_SUFFIX,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn sidecar_path(path: &Path) -> PathBuf {
    path.with_extension(PLUGIN_DISCOVERY_MANIFEST_SUFFIX)
}

pub(crate) fn read_verified_manifest(path: &Path) -> Result<PluginDiscoveryManifestV1, String> {
    let sidecar = sidecar_path(path);
    let bytes = std::fs::read(&sidecar).map_err(|e| {
        format!(
            "discovery sidecar missing/unreadable '{}': {e}",
            sidecar.display()
        )
    })?;
    let manifest: PluginDiscoveryManifestV1 = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid discovery sidecar '{}': {e}", sidecar.display()))?;
    verify_artifact_against_manifest(path, &manifest)?;
    Ok(manifest)
}

/// Verifies the currently present artifact against a manifest snapshot captured
/// earlier. This deliberately does not re-read the sidecar; callers that own a
/// frozen composition inventory must pass that immutable snapshot here.
pub(crate) fn verify_artifact_against_manifest(
    path: &Path,
    manifest: &PluginDiscoveryManifestV1,
) -> Result<(), String> {
    if manifest.schema_version != PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported frozen discovery schema={} expected={} path='{}'",
            manifest.schema_version,
            PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
            path.display()
        ));
    }
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| "plugin file name is not UTF-8".to_owned())?;
    if manifest.artifact_file != file_name {
        return Err(format!(
            "frozen discovery artifact mismatch manifest='{}' actual='{}'",
            manifest.artifact_file, file_name
        ));
    }
    let meta =
        std::fs::metadata(path).map_err(|e| format!("metadata '{}': {e}", path.display()))?;
    if meta.len() != manifest.artifact_size {
        return Err(format!(
            "frozen discovery size mismatch manifest={} actual={} path='{}'",
            manifest.artifact_size,
            meta.len(),
            path.display()
        ));
    }
    let actual_hash = sha256_file(path)?;
    if !actual_hash.eq_ignore_ascii_case(manifest.artifact_sha256.trim()) {
        return Err(format!(
            "frozen discovery SHA-256 mismatch path='{}' manifest={} actual={}",
            path.display(),
            manifest.artifact_sha256,
            actual_hash
        ));
    }
    Ok(())
}

pub(crate) fn verify_live_descriptor_against_manifest(
    path: &Path,
    descriptor: &PluginDescriptorV2,
    manifest: &PluginDiscoveryManifestV1,
) -> Result<(), String> {
    verify_artifact_against_manifest(path, manifest)?;
    let planned = manifest.descriptor.as_ref().ok_or_else(|| {
        format!(
            "frozen discovery manifest has no plugin descriptor for '{}'",
            path.display()
        )
    })?;
    let live = PluginDiscoveryDescriptorV1::from_descriptor_v2(descriptor);
    if planned != &live {
        return Err(format!(
            "live descriptor does not match frozen discovery metadata path='{}' plugin='{}'",
            path.display(),
            live.id
        ));
    }
    Ok(())
}

/// Compatibility path for direct/manual loads that do not own a frozen plan.
/// Production bootstrap uses `verify_live_descriptor_against_manifest` with the
/// manifest captured by `FrozenPluginCompositionPlan`.
pub(crate) fn verify_live_descriptor(
    path: &Path,
    descriptor: &PluginDescriptorV2,
) -> Result<(), String> {
    let manifest = read_verified_manifest(path)?;
    verify_live_descriptor_against_manifest(path, descriptor, &manifest)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|e| format!("open '{}' for SHA-256: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 128];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read '{}' for SHA-256: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_plugin_api::PluginDiscoveryManifestV1;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_artifact(name: &str, bytes: &[u8]) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("northstar-sidecar-{nonce}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("temp artifact");
        path
    }

    fn manifest_for(path: &Path, bytes: &[u8]) -> PluginDiscoveryManifestV1 {
        PluginDiscoveryManifestV1 {
            schema_version: PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
            artifact_file: path.file_name().unwrap().to_string_lossy().to_string(),
            artifact_size: bytes.len() as u64,
            artifact_sha256: format!("{:x}", Sha256::digest(bytes)),
            signature: None,
            descriptor: None,
            platform_runtime: None,
            has_canonical_root: false,
            has_legacy_root: false,
        }
    }

    #[test]
    fn sidecar_replaces_only_final_extension() {
        assert_eq!(
            sidecar_path(Path::new("a/b/plugin-1.0-release.dll")),
            PathBuf::from("a/b/plugin-1.0-release.nspmeta.json")
        );
    }

    #[test]
    fn frozen_manifest_rejects_artifact_mutation_without_rereading_sidecar() {
        let original = b"not-a-real-dll-a";
        let path = temp_artifact("provider.dll", original);
        let manifest = manifest_for(&path, original);
        verify_artifact_against_manifest(&path, &manifest).expect("original artifact");

        std::fs::write(&path, b"not-a-real-dll-b").expect("mutate artifact");
        let error = verify_artifact_against_manifest(&path, &manifest).expect_err("must reject");
        assert!(error.contains("SHA-256 mismatch") || error.contains("size mismatch"));

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
