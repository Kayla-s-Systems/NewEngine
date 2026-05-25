#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use libloading::Library;
use newengine_plugin_api::{
    PluginBootstrapPhase, PluginDescriptor, PluginInfo, PluginRootV1Ref, PluginSignatureV1,
};

use super::graph::ScannedDynlibKind;

pub(super) const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1";
pub(super) const PLUGIN_SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1";
pub(super) const PLUGIN_ROOT_SYMBOL: &[u8] = newengine_plugin_api::PLUGIN_ROOT_SYMBOL_BYTES;
pub(super) const LEGACY_PLUGIN_ROOT_SYMBOL: &[u8] = newengine_plugin_api::LEGACY_PLUGIN_ROOT_SYMBOL_BYTES;

#[derive(Debug, Clone, Default)]
pub(super) struct ScanPluginProbe {
    pub(super) signature: Option<PluginSignatureV1>,
    pub(super) info: Option<PluginInfo>,
    pub(super) descriptor: Option<PluginDescriptor>,
    pub(super) has_canonical_root: bool,
    pub(super) has_legacy_root: bool,
}

pub(super) fn probe_plugin_metadata(lib: &Library) -> Result<ScanPluginProbe, String> {
    let mut out = ScanPluginProbe::default();

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL) }
    {
        out.signature = Some(unsafe { sym() });
    }

    out.has_canonical_root = unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) }.is_ok();
    out.has_legacy_root = unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(LEGACY_PLUGIN_ROOT_SYMBOL) }.is_ok();

    if out.has_legacy_root && !out.has_canonical_root {
        log::warn!(
            "plugins: stale plugin ABI detected: found legacy root symbol '{}' but missing canonical root symbol '{}'; rebuild the plugin",
            newengine_plugin_api::LEGACY_PLUGIN_ROOT_SYMBOL_NAME,
            newengine_plugin_api::PLUGIN_ROOT_SYMBOL_NAME,
        );
    }

    // Discovery is intentionally signature-only. Calling root.create() or
    // root.ui_assets_v1() during scan can execute stale ABI prefix callbacks
    // from DLLs compiled before the canonical PluginModule cleanup. The loader
    // performs the full canonical-root validation only after selection.
    Ok(out)
}

fn probe_identity_from_probe(probe: &ScanPluginProbe) -> (Option<String>, Option<String>) {
    let id = probe
        .signature
        .as_ref()
        .map(|s| s.id.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe.descriptor
                .as_ref()
                .map(|d| d.id.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            probe.info
                .as_ref()
                .map(|i| i.id.to_string())
                .filter(|v| !v.trim().is_empty())
        });

    let version = probe
        .signature
        .as_ref()
        .map(|s| s.version.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe.descriptor
                .as_ref()
                .map(|d| d.version.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            probe.info
                .as_ref()
                .map(|i| i.version.to_string())
                .filter(|v| !v.trim().is_empty())
        });

    (id, version)
}

pub(super) fn platform_runtime_identity_from_probe(
    path: &Path,
    probe: &ScanPluginProbe,
) -> (String, String) {
    match probe_identity_from_probe(probe) {
        (Some(id), Some(version)) => (id, version),
        (Some(id), None) => (id, "-".to_owned()),
        _ => infer_platform_runtime_identity(path),
    }
}

pub(super) fn build_scanned_plugin_kind(probe: &ScanPluginProbe) -> Option<ScannedDynlibKind> {
    if probe.has_legacy_root && !probe.has_canonical_root {
        return None;
    }

    if probe.signature.is_none() && probe.info.is_none() && probe.descriptor.is_none() {
        return None;
    }

    let (id, version) = probe_identity_from_probe(probe);

    let id = id.unwrap_or_else(|| "<unknown-plugin>".to_owned());
    let version = version.unwrap_or_else(|| "-".to_owned());

    let phase = probe
        .signature
        .as_ref()
        .map(|s| s.bootstrap_phase)
        .unwrap_or(PluginBootstrapPhase::Engine);

    let descriptor_kind = probe
        .descriptor
        .as_ref()
        .map(|d| d.kind)
        .or_else(|| probe.signature.as_ref().map(|s| s.kind));

    let declared_capabilities = probe.descriptor.as_ref().map(|d| d.capabilities.len());
    let service_gateways = probe
        .descriptor
        .as_ref()
        .map(crate::service_gateway::descriptor_engine_gateways)
        .unwrap_or_default();

    let backend_priority = probe
        .descriptor
        .as_ref()
        .map(crate::service_gateway::descriptor_max_gateway_priority)
        .unwrap_or(0);

    Some(ScannedDynlibKind::Plugin {
        id,
        version,
        phase,
        descriptor_kind,
        declared_capabilities,
        service_gateways,
        backend_priority,
    })
}


fn infer_platform_runtime_identity(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<platform-runtime>".to_owned());

    let parts: Vec<&str> = stem.split('-').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return ("<platform-runtime>".to_owned(), "-".to_owned());
    }

    let version_index = parts
        .iter()
        .position(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()));

    match version_index {
        Some(idx) => {
            let id = parts[..idx].join("-");
            let raw_version = parts[idx..].join("-");
            let version = normalize_version_suffix(&raw_version);

            let id = if id.trim().is_empty() {
                "<platform-runtime>".to_owned()
            } else {
                id
            };

            let version = if version.trim().is_empty() {
                "-".to_owned()
            } else {
                version
            };

            (id, version)
        }
        None => (stem, "-".to_owned()),
    }
}

#[inline]
fn normalize_version_suffix(raw: &str) -> String {
    raw.strip_suffix("-dev")
        .or_else(|| raw.strip_suffix("-debug"))
        .or_else(|| raw.strip_suffix("-release"))
        .unwrap_or(raw)
        .to_owned()
}

