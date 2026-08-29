#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{
    PluginBootstrapPhase, PluginDescriptor, PluginDescriptorV2, PluginInfo, PluginSignatureV1,
};

use super::graph::ScannedDynlibKind;

/// Sidecar-decoded discovery facts. Runtime discovery never maps a DLL to fill this
/// structure; the build-time manifest exporter is the only ABI-probe boundary.
#[derive(Debug, Clone, Default)]
pub(super) struct ScanPluginProbe {
    pub(super) signature: Option<PluginSignatureV1>,
    pub(super) info: Option<PluginInfo>,
    pub(super) descriptor: Option<PluginDescriptor>,
    pub(super) descriptor_v2: Option<PluginDescriptorV2>,
    pub(super) has_canonical_root: bool,
    pub(super) has_legacy_root: bool,
}

fn probe_identity_from_probe(probe: &ScanPluginProbe) -> (Option<String>, Option<String>) {
    let id = probe
        .signature
        .as_ref()
        .map(|s| s.id.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe
                .descriptor_v2
                .as_ref()
                .map(|d| d.id.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            probe
                .descriptor
                .as_ref()
                .map(|d| d.id.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            probe
                .info
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
            probe
                .descriptor_v2
                .as_ref()
                .map(|d| d.version.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            probe
                .descriptor
                .as_ref()
                .map(|d| d.version.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            probe
                .info
                .as_ref()
                .map(|i| i.version.to_string())
                .filter(|v| !v.trim().is_empty())
        });

    (id, version)
}

pub(super) fn build_scanned_plugin_kind(probe: &ScanPluginProbe) -> Option<ScannedDynlibKind> {
    if probe.has_legacy_root && !probe.has_canonical_root {
        return None;
    }
    if probe.signature.is_none()
        && probe.info.is_none()
        && probe.descriptor.is_none()
        && probe.descriptor_v2.is_none()
    {
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
        .descriptor_v2
        .as_ref()
        .map(|d| d.kind)
        .or_else(|| probe.descriptor.as_ref().map(|d| d.kind))
        .or_else(|| probe.signature.as_ref().map(|s| s.kind));
    let declared_capabilities = probe
        .descriptor_v2
        .as_ref()
        .map(|d| d.capabilities.len())
        .or_else(|| probe.descriptor.as_ref().map(|d| d.capabilities.len()));
    let service_gateways = probe
        .descriptor_v2
        .as_ref()
        .map(crate::service_gateway::descriptor_engine_gateways_v2)
        .or_else(|| {
            probe
                .descriptor
                .as_ref()
                .map(crate::service_gateway::descriptor_engine_gateways)
        })
        .unwrap_or_default();
    let backend_priority = probe
        .descriptor_v2
        .as_ref()
        .map(crate::service_gateway::descriptor_max_gateway_priority_v2)
        .or_else(|| {
            probe
                .descriptor
                .as_ref()
                .map(crate::service_gateway::descriptor_max_gateway_priority)
        })
        .unwrap_or(0);

    Some(ScannedDynlibKind::Plugin {
        id,
        version,
        phase,
        descriptor_kind,
        declared_capabilities,
        descriptor: probe.descriptor.clone(),
        descriptor_v2: probe.descriptor_v2.clone(),
        service_gateways,
        backend_priority,
    })
}
