#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{
    PluginDescriptorV2, PluginDiscoveryDescriptorV1, PluginDiscoveryManifestV2,
    PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE, PLUGIN_DISCOVERY_EMBEDDED_MAGIC,
    PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION, PLUGIN_DISCOVERY_MANIFEST_SUFFIX,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedPluginDiscoveryManifest {
    pub(crate) manifest: PluginDiscoveryManifestV2,
    pub(crate) artifact_size: u64,
    pub(crate) artifact_sha256: String,
}

pub(super) fn sidecar_path(path: &Path) -> PathBuf {
    path.with_extension(PLUGIN_DISCOVERY_MANIFEST_SUFFIX)
}

/// Reads declarative discovery metadata without mapping the dynamic library.
///
/// Canonical release artifacts keep the manifest inside the DLL trailer. A sidecar
/// fallback remains temporarily for dev/debug and migration compatibility.
pub(super) fn read_manifest_metadata(path: &Path) -> Result<PluginDiscoveryManifestV2, String> {
    if let Some(manifest) = read_embedded_manifest(path)? {
        verify_manifest_identity(path, &manifest)?;
        return Ok(manifest);
    }

    let sidecar = sidecar_path(path);
    let bytes = std::fs::read(&sidecar).map_err(|e| {
        format!(
            "embedded discovery metadata missing and sidecar unreadable '{}': {e}",
            sidecar.display()
        )
    })?;
    let manifest: PluginDiscoveryManifestV2 = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid discovery sidecar '{}': {e}", sidecar.display()))?;
    verify_manifest_identity(path, &manifest)?;
    Ok(manifest)
}

fn read_embedded_manifest(path: &Path) -> Result<Option<PluginDiscoveryManifestV2>, String> {
    const MAX_EMBEDDED_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

    let mut file = File::open(path)
        .map_err(|e| format!("open '{}' for embedded metadata: {e}", path.display()))?;
    let file_len = file
        .metadata()
        .map_err(|e| format!("metadata '{}': {e}", path.display()))?
        .len();
    if file_len < PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE {
        return Ok(None);
    }

    file.seek(SeekFrom::End(
        -(PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE as i64),
    ))
    .map_err(|e| format!("seek embedded footer '{}': {e}", path.display()))?;
    let mut len_bytes = [0u8; 8];
    let mut magic = [0u8; 16];
    file.read_exact(&mut len_bytes)
        .map_err(|e| format!("read embedded length '{}': {e}", path.display()))?;
    file.read_exact(&mut magic)
        .map_err(|e| format!("read embedded magic '{}': {e}", path.display()))?;
    if &magic != PLUGIN_DISCOVERY_EMBEDDED_MAGIC {
        return Ok(None);
    }

    let payload_len = u64::from_le_bytes(len_bytes);
    if payload_len == 0 || payload_len > MAX_EMBEDDED_MANIFEST_BYTES {
        return Err(format!(
            "invalid embedded manifest length={} path='{}'",
            payload_len,
            path.display()
        ));
    }
    let trailer_len = payload_len
        .checked_add(PLUGIN_DISCOVERY_EMBEDDED_FOOTER_SIZE)
        .ok_or_else(|| "embedded manifest length overflow".to_owned())?;
    if trailer_len > file_len {
        return Err(format!(
            "embedded manifest exceeds artifact length path='{}'",
            path.display()
        ));
    }

    file.seek(SeekFrom::Start(file_len - trailer_len))
        .map_err(|e| format!("seek embedded manifest '{}': {e}", path.display()))?;
    let mut payload = vec![0u8; payload_len as usize];
    file.read_exact(&mut payload)
        .map_err(|e| format!("read embedded manifest '{}': {e}", path.display()))?;
    let manifest = serde_json::from_slice(&payload)
        .map_err(|e| format!("invalid embedded manifest '{}': {e}", path.display()))?;
    Ok(Some(manifest))
}

pub(crate) fn read_verified_manifest(
    path: &Path,
) -> Result<VerifiedPluginDiscoveryManifest, String> {
    let manifest = read_manifest_metadata(path)?;
    let artifact_size = std::fs::metadata(path)
        .map_err(|e| format!("metadata '{}': {e}", path.display()))?
        .len();
    let artifact_sha256 = sha256_file(path)?;
    Ok(VerifiedPluginDiscoveryManifest {
        manifest,
        artifact_size,
        artifact_sha256,
    })
}

/// Verifies the currently present artifact against the fingerprint captured during
/// discovery. The sidecar itself is deliberately not re-read: composition owns an
/// immutable metadata + observed-artifact snapshot after scanning.
pub(crate) fn verify_artifact_against_manifest(
    path: &Path,
    snapshot: &VerifiedPluginDiscoveryManifest,
) -> Result<(), String> {
    verify_manifest_identity(path, &snapshot.manifest)?;
    let meta =
        std::fs::metadata(path).map_err(|e| format!("metadata '{}': {e}", path.display()))?;
    if meta.len() != snapshot.artifact_size {
        return Err(format!(
            "frozen discovery size mismatch scanned={} actual={} path='{}'",
            snapshot.artifact_size,
            meta.len(),
            path.display()
        ));
    }
    let actual_hash = sha256_file(path)?;
    if !actual_hash.eq_ignore_ascii_case(snapshot.artifact_sha256.trim()) {
        return Err(format!(
            "frozen discovery SHA-256 mismatch path='{}' scanned={} actual={}",
            path.display(),
            snapshot.artifact_sha256,
            actual_hash
        ));
    }
    Ok(())
}

fn verify_manifest_identity(
    path: &Path,
    manifest: &PluginDiscoveryManifestV2,
) -> Result<(), String> {
    if manifest.schema_version != PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported discovery schema={} expected={} path='{}'",
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
            "discovery artifact mismatch manifest='{}' actual='{}'",
            manifest.artifact_file, file_name
        ));
    }
    Ok(())
}

pub(crate) fn verify_live_descriptor_against_manifest(
    path: &Path,
    descriptor: &PluginDescriptorV2,
    snapshot: &VerifiedPluginDiscoveryManifest,
) -> Result<(), String> {
    verify_artifact_against_manifest(path, snapshot)?;
    verify_live_descriptor_metadata_against_manifest(path, descriptor, &snapshot.manifest)
}

fn verify_live_descriptor_metadata_against_manifest(
    path: &Path,
    descriptor: &PluginDescriptorV2,
    manifest: &PluginDiscoveryManifestV2,
) -> Result<(), String> {
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
pub(crate) fn verify_live_descriptor(
    path: &Path,
    descriptor: &PluginDescriptorV2,
) -> Result<(), String> {
    let snapshot = read_verified_manifest(path)?;
    verify_live_descriptor_metadata_against_manifest(path, descriptor, &snapshot.manifest)
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

    fn manifest_for(path: &Path) -> PluginDiscoveryManifestV2 {
        PluginDiscoveryManifestV2 {
            schema_version: PLUGIN_DISCOVERY_MANIFEST_SCHEMA_VERSION,
            artifact_file: path.file_name().unwrap().to_string_lossy().to_string(),
            signature: None,
            descriptor: None,
            platform_runtime: None,
            has_canonical_root: false,
            has_legacy_root: false,
        }
    }

    fn append_embedded_manifest(path: &Path, manifest: &PluginDiscoveryManifestV2) {
        use std::io::Write;
        let payload = serde_json::to_vec(manifest).expect("json");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open artifact for embedded metadata");
        file.write_all(&payload).expect("payload");
        file.write_all(&(payload.len() as u64).to_le_bytes())
            .expect("length");
        file.write_all(PLUGIN_DISCOVERY_EMBEDDED_MAGIC)
            .expect("magic");
    }

    #[test]
    fn embedded_metadata_is_preferred_and_needs_no_sidecar() {
        let bytes = b"not-a-real-dll-with-overlay";
        let path = temp_artifact("embedded-provider.dll", bytes);
        let manifest = manifest_for(&path);
        append_embedded_manifest(&path, &manifest);
        assert!(!sidecar_path(&path).exists());

        let metadata = read_manifest_metadata(&path).expect("embedded metadata lookup");
        assert_eq!(metadata, manifest);
        let snapshot = read_verified_manifest(&path).expect("fingerprinted embedded artifact");
        assert_eq!(snapshot.manifest, manifest);
        assert!(snapshot.artifact_size > bytes.len() as u64);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn sidecar_replaces_only_final_extension() {
        assert_eq!(
            sidecar_path(Path::new("a/b/plugin-1.0-release.dll")),
            PathBuf::from("a/b/plugin-1.0-release.nspmeta.json")
        );
    }

    #[test]
    fn metadata_lookup_does_not_hash_artifact() {
        let bytes = b"not-a-real-dll-target";
        let path = temp_artifact("provider.dll", bytes);
        let manifest = manifest_for(&path);
        let sidecar = sidecar_path(&path);
        std::fs::write(
            &sidecar,
            serde_json::to_vec_pretty(&manifest).expect("json"),
        )
        .expect("sidecar");

        let metadata = read_manifest_metadata(&path).expect("metadata lookup");
        assert_eq!(metadata.artifact_file, "provider.dll");
        let snapshot =
            read_verified_manifest(&path).expect("verified lookup fingerprints artifact");
        assert_eq!(snapshot.artifact_size, bytes.len() as u64);
        assert_eq!(
            snapshot.artifact_sha256,
            format!("{:x}", Sha256::digest(bytes))
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn frozen_snapshot_rejects_artifact_mutation_without_rereading_sidecar() {
        let original = b"not-a-real-dll-a";
        let path = temp_artifact("provider.dll", original);
        let manifest = manifest_for(&path);
        std::fs::write(
            sidecar_path(&path),
            serde_json::to_vec_pretty(&manifest).expect("json"),
        )
        .expect("sidecar");
        let snapshot = read_verified_manifest(&path).expect("capture fingerprint");
        verify_artifact_against_manifest(&path, &snapshot).expect("original artifact");

        std::fs::write(&path, b"not-a-real-dll-b").expect("mutate artifact");
        let error = verify_artifact_against_manifest(&path, &snapshot).expect_err("must reject");
        assert!(error.contains("SHA-256 mismatch") || error.contains("size mismatch"));

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
