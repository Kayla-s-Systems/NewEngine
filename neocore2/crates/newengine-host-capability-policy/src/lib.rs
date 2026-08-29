#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::{
    HostCapabilities, HostEnvironmentSnapshot, ProviderSelectionHint, RuntimeCapabilityPolicy,
};

/// Pure policy derivation. This crate does not probe hardware and does not register providers.
pub fn derive(
    environment: &HostEnvironmentSnapshot,
    capabilities: &HostCapabilities,
) -> RuntimeCapabilityPolicy {
    let preferred_gpu = capabilities.preferred_gpu();
    let hardware_gpu = capabilities.gpu.iter().any(|gpu| !gpu.is_software);
    let mut provider_hints = Vec::new();

    let mut render_tags = Vec::new();
    let mut forbidden_render_tags = Vec::new();
    match environment.os.as_str() {
        "windows" => {
            if preferred_gpu
                .and_then(|gpu| gpu.graphics_api.as_deref())
                .is_some_and(|api| api.contains("D3D12"))
            {
                render_tags.push("backend.d3d12".to_owned());
            }
            render_tags.push("backend.vulkan".to_owned());
            forbidden_render_tags.push("backend.metal".to_owned());
        }
        "linux" => {
            render_tags.push("backend.vulkan".to_owned());
            forbidden_render_tags.push("backend.d3d12".to_owned());
            forbidden_render_tags.push("backend.metal".to_owned());
        }
        "macos" => {
            render_tags.push("backend.metal".to_owned());
            render_tags.push("backend.vulkan".to_owned());
            forbidden_render_tags.push("backend.d3d12".to_owned());
        }
        _ => render_tags.push("backend.vulkan".to_owned()),
    }
    provider_hints.push(ProviderSelectionHint {
        gateway_id: "engine.render".to_owned(),
        preferred_system_tags: render_tags,
        forbidden_system_tags: forbidden_render_tags,
        preference_bonus: 2_000,
        reason: "host OS/hardware graphics API compatibility policy".to_owned(),
    });
    provider_hints.push(ProviderSelectionHint {
        gateway_id: "engine.input".to_owned(),
        preferred_system_tags: vec![format!("platform.{}", environment.os)],
        forbidden_system_tags: Vec::new(),
        preference_bonus: 1_000,
        reason: "native platform input preference".to_owned(),
    });

    RuntimeCapabilityPolicy {
        worker_threads: capabilities.cpu.logical_cores,
        preferred_gpu_stable_id: preferred_gpu.map(|gpu| gpu.stable_id.clone()),
        allow_software_rendering: !hardware_gpu,
        provider_hints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_host_capabilities_api::{CpuCapabilities, GpuCapabilities};

    #[test]
    fn windows_policy_prefers_d3d12_when_probe_reports_it() {
        let environment = HostEnvironmentSnapshot {
            os: "windows".to_owned(),
            ..Default::default()
        };
        let capabilities = HostCapabilities {
            cpu: CpuCapabilities {
                logical_cores: Some(20),
                ..Default::default()
            },
            gpu: vec![GpuCapabilities {
                index: 0,
                stable_id: "gpu0".to_owned(),
                graphics_api: Some("D3D12 feature_level=12_2".to_owned()),
                ..Default::default()
            }],
            preferred_gpu_index: Some(0),
            ..Default::default()
        };
        let policy = derive(&environment, &capabilities);
        assert_eq!(policy.worker_threads, Some(20));
        assert!(policy
            .provider_hints
            .iter()
            .any(|hint| hint.gateway_id == "engine.render"
                && hint
                    .preferred_system_tags
                    .iter()
                    .any(|tag| tag == "backend.d3d12")));
    }
}
