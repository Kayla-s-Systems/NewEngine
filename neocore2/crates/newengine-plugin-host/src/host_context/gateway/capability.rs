use newengine_plugin_api::{CapabilityKind, CapabilityRole};

use super::super::state::ctx;
use super::registry::emit_gateway_diagnostic;

fn parse_backend_priority(json: &str) -> i64 {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("backend_priority").and_then(|x| x.as_i64()))
        .unwrap_or(0)
}

fn emit_capability_active(
    capability_id: &str,
    service_id: &str,
    owner: &str,
    active_score: i64,
    backend_priority: i64,
    origin: crate::service_gateway::GatewayProviderOrigin,
) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.capability.active",
            "INFO",
            "Capability provider active",
            serde_json::json!({
                "capability_id": capability_id,
                "service_id": service_id,
                "owner": owner,
                "active_score": active_score,
                "backend_priority": backend_priority,
                "origin": origin.as_str()
            }),
        );
    });
}

fn emit_capability_shadowed(
    capability_id: &str,
    service_id: &str,
    owner: &str,
    active_service_id: &str,
    active_owner: &str,
    shadowed_score: i64,
    active_score: i64,
) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.capability.shadowed",
            "INFO",
            "Capability provider shadowed",
            serde_json::json!({
                "capability_id": capability_id,
                "service_id": service_id,
                "owner": owner,
                "active_service_id": active_service_id,
                "active_owner": active_owner,
                "shadowed_score": shadowed_score,
                "active_score": active_score
            }),
        );
    });
}

fn emit_capability_missing(capability_id: &str) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.capability.missing",
            "WARN",
            "Capability provider missing",
            serde_json::json!({ "capability_id": capability_id }),
        );
    });
}

fn emit_capability_conflict(capability_id: &str, score: i64, providers: &[serde_json::Value]) {
    emit_gateway_diagnostic(|| {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.capability.conflict",
            "WARN",
            "Capability provider score conflict",
            serde_json::json!({
                "capability_id": capability_id,
                "score": score,
                "providers": providers
            }),
        );
    });
}

pub fn resolve_service_for_backend_capability(capability_id: &str) -> Option<String> {
    let c = ctx();
    let services = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let descriptors = match c.plugin_descriptors.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let plugin_origins = match c.plugin_origins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut candidates: Vec<(
        i64,
        i64,
        String,
        String,
        crate::service_gateway::GatewayProviderOrigin,
    )> = Vec::new();

    for (service_id, entry) in services.iter() {
        let Some(owner) = entry.owner_plugin_id.as_deref() else {
            continue;
        };
        let Some(descriptor) = descriptors.get(owner) else {
            continue;
        };

        let Some(backend_capability) = descriptor
            .capabilities
            .iter()
            .find(|cap| cap.role == CapabilityRole::Provides && cap.id.as_str() == capability_id)
        else {
            continue;
        };

        let declares_registered_service = descriptor.capabilities.iter().any(|cap| {
            cap.role == CapabilityRole::Provides
                && cap.kind == CapabilityKind::ServiceV1
                && cap.id.as_str() == service_id
        });
        if !declares_registered_service {
            continue;
        }

        let backend_priority = parse_backend_priority(backend_capability.describe_json.as_str());
        let origin = plugin_origins
            .get(owner)
            .copied()
            .unwrap_or(crate::service_gateway::GatewayProviderOrigin::GamePlugin);
        candidates.push((
            origin.origin_bias() + backend_priority,
            backend_priority,
            service_id.clone(),
            owner.to_owned(),
            origin,
        ));
    }

    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
    });

    let Some((active_score, active_priority, active_service_id, active_owner, active_origin)) =
        candidates.first().cloned()
    else {
        emit_capability_missing(capability_id);
        return None;
    };

    let tied = candidates
        .iter()
        .filter(|(score, _, _, _, _)| *score == active_score)
        .map(|(score, priority, service_id, owner, origin)| {
            serde_json::json!({
                "service_id": service_id,
                "owner": owner,
                "score": score,
                "backend_priority": priority,
                "origin": origin.as_str()
            })
        })
        .collect::<Vec<_>>();
    if tied.len() > 1 {
        emit_capability_conflict(capability_id, active_score, &tied);
    }

    emit_capability_active(
        capability_id,
        &active_service_id,
        &active_owner,
        active_score,
        active_priority,
        active_origin,
    );

    for (score, _, service_id, owner, _) in candidates.iter().skip(1) {
        emit_capability_shadowed(
            capability_id,
            service_id,
            owner,
            &active_service_id,
            &active_owner,
            *score,
            active_score,
        );
    }

    Some(active_service_id)
}
