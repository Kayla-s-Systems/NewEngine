#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use libloading::Library;
use newengine_plugin_api::{
    PluginBootstrapPhase, PluginDescriptor, PluginInfo, PluginModuleDyn, PluginRootV1Ref,
    PluginSignatureV1,
};

use super::graph::ScannedDynlibKind;
use crate::manager::adapter::{ModuleAdapterAny, V1Adapter, V2Adapter, V3Adapter};

pub(super) const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";
pub(super) const PLUGIN_SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1\0";
pub(super) const PLUGIN_ROOT_SYMBOL: &[u8] = b"export_plugin_root\0";

#[derive(Debug, Clone, Default)]
pub(super) struct ScanPluginProbe {
    pub(super) signature: Option<PluginSignatureV1>,
    pub(super) info: Option<PluginInfo>,
    pub(super) descriptor: Option<PluginDescriptor>,
}

pub(super) fn probe_plugin_metadata(lib: &Library) -> Result<ScanPluginProbe, String> {
    let mut out = ScanPluginProbe::default();

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL) }
    {
        out.signature = Some(unsafe { sym() });
    }

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) }
    {
        let root = unsafe { sym() };
        let (_module, info, descriptor) = select_abi_for_scan(root);
        out.info = Some(info);
        out.descriptor = descriptor;
    }

    Ok(out)
}

pub(super) fn platform_runtime_identity_from_probe(
    path: &Path,
    probe: &ScanPluginProbe,
) -> (String, String) {
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

    match (id, version) {
        (Some(id), Some(version)) => (id, version),
        (Some(id), None) => (id, "-".to_owned()),
        _ => infer_platform_runtime_identity(path),
    }
}

pub(super) fn build_scanned_plugin_kind(probe: &ScanPluginProbe) -> Option<ScannedDynlibKind> {
    if probe.signature.is_none() && probe.info.is_none() && probe.descriptor.is_none() {
        return None;
    }

    let id = probe
        .signature
        .as_ref()
        .map(|s| s.id.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe.info
                .as_ref()
                .map(|i| i.id.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "<unknown-plugin>".to_owned());

    let version = probe
        .signature
        .as_ref()
        .map(|s| s.version.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe.info
                .as_ref()
                .map(|i| i.version.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "-".to_owned());

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

    Some(ScannedDynlibKind::Plugin {
        id,
        version,
        phase,
        descriptor_kind,
        declared_capabilities,
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

fn select_abi_for_scan(
    root: PluginRootV1Ref,
) -> (ModuleAdapterAny, PluginInfo, Option<PluginDescriptor>) {
    if let Some(create_v3) = root.create_v3() {
        let m3 = create_v3();
        let d = m3.descriptor_v3();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        (
            ModuleAdapterAny::V3(V3Adapter { module: m3 }),
            info,
            Some(d),
        )
    } else if let Some(create_v2) = root.create_v2() {
        let m2 = create_v2();
        let d = m2.descriptor();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        (
            ModuleAdapterAny::V2(V2Adapter { module: m2 }),
            info,
            Some(d),
        )
    } else {
        let m1: PluginModuleDyn<'static> = root.create()();
        let info = m1.info();
        (ModuleAdapterAny::V1(V1Adapter { module: m1 }), info, None)
    }
}
