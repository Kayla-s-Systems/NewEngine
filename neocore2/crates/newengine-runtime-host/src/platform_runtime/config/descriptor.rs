use std::path::Path;

use newengine_platform_api::PlatformRuntimeRunFnV1;
use newengine_plugin_api::{
    CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor, PluginKind,
};

pub(super) fn platform_runtime_version_from_path(runtime_path: &Path) -> String {
    let Some(stem) = runtime_path.file_stem().and_then(|stem| stem.to_str()) else {
        return "-".to_owned();
    };

    stem.split('-')
        .find(|part| looks_like_semver(part))
        .map(str::to_owned)
        .unwrap_or_else(|| "-".to_owned())
}

fn looks_like_semver(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(major) = segments.next() else {
        return false;
    };
    let Some(minor) = segments.next() else {
        return false;
    };
    let Some(patch) = segments.next() else {
        return false;
    };
    segments.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && !patch.is_empty()
        && major.chars().all(|ch| ch.is_ascii_digit())
        && minor.chars().all(|ch| ch.is_ascii_digit())
        && patch.chars().all(|ch| ch.is_ascii_digit())
}

pub(super) fn ensure_platform_runtime_capabilities(
    mut descriptor: PluginDescriptor,
) -> PluginDescriptor {
    fn has_cap(
        descriptor: &PluginDescriptor,
        id: &str,
        role: CapabilityRole,
        kind: CapabilityKind,
        version: u32,
    ) -> bool {
        descriptor.capabilities.iter().any(|cap| {
            cap.id.as_str() == id && cap.role == role && cap.kind == kind && cap.version >= version
        })
    }

    // The platform runtime is an external event-loop entrypoint, not a plugin-owned
    // `platform.api` ServiceV1 provider. `engine.platform` is registered later as an
    // engine-runtime snapshot gateway by `snapshot_service.rs`; advertising a backend
    // route here makes the gateway registry try to bind `platform.api` every frame.
    let required = vec![
        CapabilityDesc::new(
            "platform.runtime.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
        )
        .with_json(r#"{"role":"platform-runtime"}"#),
        CapabilityDesc::new(
            "platform.surface.v1",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
        )
        .with_json(r#"{"role":"surface"}"#),
        CapabilityDesc::new(
            "platform.input.events.v1",
            CapabilityRole::Provides,
            CapabilityKind::EventsV1,
            1,
        )
        .with_json(r#"{"role":"input-events"}"#),
    ];

    for cap in required {
        if !has_cap(
            &descriptor,
            cap.id.as_str(),
            cap.role,
            cap.kind,
            cap.version,
        ) {
            descriptor.capabilities.push(cap);
        }
    }

    descriptor
}

pub(super) fn synthesize_platform_descriptor(
    id: &str,
    name: &str,
    version: &str,
) -> PluginDescriptor {
    PluginDescriptor::builder(id, name, version, PluginKind::Runtime)
        .push(
            CapabilityDesc::new(
                "platform.runtime.v1",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(r#"{"role":"platform-runtime"}"#),
        )
        .push(
            CapabilityDesc::new(
                "platform.surface.v1",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(r#"{"role":"surface"}"#),
        )
        .push(
            CapabilityDesc::new(
                "platform.input.events.v1",
                CapabilityRole::Provides,
                CapabilityKind::EventsV1,
                1,
            )
            .with_json(r#"{"role":"input-events"}"#),
        )
        .build()
}

#[allow(dead_code)]
fn _abi_marker(_: PlatformRuntimeRunFnV1) {}
