use std::collections::BTreeSet;

use newengine_schema_api::{
    SchemaBindingFunctionV1, SchemaBindingManifestV1, ENGINE_SCHEMA_SERVICE_ID,
};
use serde_json::{json, Value};

use super::SchemaRegistryState;

impl SchemaRegistryState {
    pub fn binding_manifest_from_value(&self, request: Value) -> SchemaBindingManifestV1 {
        let target_language = request
            .get("target_language")
            .and_then(Value::as_str)
            .unwrap_or("generic")
            .to_owned();
        let module_id = request
            .get("module_id")
            .and_then(Value::as_str)
            .unwrap_or("engine.schema.bindings")
            .to_owned();
        let mut type_descriptors = self.records.values().cloned().collect::<Vec<_>>();
        type_descriptors.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        SchemaBindingManifestV1 {
            target_language,
            module_id,
            gateway: ENGINE_SCHEMA_SERVICE_ID.to_owned(),
            type_descriptors,
            functions: live_binding_functions(),
            diagnostics: self.registry_diagnostics.clone(),
            ..SchemaBindingManifestV1::default()
        }
    }

    pub fn dump_registry(&self) -> Value {
        json!({
            "schema": self.source_schema,
            "runtime_contract": newengine_schema_api::SCHEMA_RUNTIME_CONTRACT,
            "policy": self.policy,
            "gateway": newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID,
            "provider_route": crate::PROVIDER_ROUTE,
            "replaceable": true,
            "records": self.records.values().cloned().collect::<Vec<_>>(),
            "diagnostics": self.registry_diagnostics,
        })
    }

    pub fn shutdown(&mut self) -> Value {
        self.shutdown_count = self.shutdown_count.saturating_add(1);
        json!({
            "schema": "newengine.schema.shutdown.v1",
            "accepted": true,
            "idempotent": true,
            "shutdown_count": self.shutdown_count,
        })
    }
}

/// Build tooling bindings from the active runtime composition rather than from a
/// centrally maintained API list. `engine.schema` remains the authoritative bridge:
/// it projects the selected provider topology into a language-neutral binding manifest.
fn live_binding_functions() -> Vec<SchemaBindingFunctionV1> {
    let mut gateways = newengine_plugin_host::list_engine_gateway_routes()
        .into_iter()
        .filter(|route| route.active && route.gateway_id.starts_with("engine."))
        .map(|route| route.gateway_id)
        .collect::<BTreeSet<_>>();

    // The schema provider may be serving this request while topology snapshots are
    // still converging. Keep its own gateway eligible without introducing any method
    // constants here; methods are still discovered exclusively from describe().
    gateways.insert(ENGINE_SCHEMA_SERVICE_ID.to_owned());

    let mut functions = Vec::new();
    let mut seen = BTreeSet::new();
    for gateway in gateways {
        let Some(description) = newengine_plugin_host::describe_service(&gateway) else {
            continue;
        };
        for function in binding_functions_from_service_description(&gateway, &description) {
            if seen.insert((function.gateway.clone(), function.method.clone())) {
                functions.push(function);
            }
        }
    }
    functions.sort_by(|left, right| {
        left.gateway
            .cmp(&right.gateway)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.method.cmp(&right.method))
    });
    functions
}

fn binding_functions_from_service_description(
    gateway: &str,
    description_json: &str,
) -> Vec<SchemaBindingFunctionV1> {
    let Ok(description) = serde_json::from_str::<Value>(description_json) else {
        return Vec::new();
    };
    let Some(methods) = description.get("methods").and_then(Value::as_array) else {
        return Vec::new();
    };

    methods
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|method| !method.is_empty())
        .map(|method| SchemaBindingFunctionV1 {
            name: binding_name_from_method(method),
            method: method.to_owned(),
            request_type: String::new(),
            response_type: String::new(),
            gateway: gateway.to_owned(),
        })
        .collect()
}

fn binding_name_from_method(method: &str) -> String {
    let leaf = method.rsplit('.').next().unwrap_or(method);
    let mut name = leaf.to_owned();
    for suffix in ["_json_v1", "_bytes_v1", "_json", "_bytes", "_v1"] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped.to_owned();
            break;
        }
    }
    let mut normalized = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            normalized.push(ch);
        } else if !normalized.ends_with('_') {
            normalized.push('_');
        }
    }
    let normalized = normalized.trim_matches('_').to_owned();
    if normalized.is_empty() {
        "call".to_owned()
    } else if normalized
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_digit)
    {
        format!("_{normalized}")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_description_methods_become_gateway_scoped_bindings() {
        let functions = binding_functions_from_service_description(
            "engine.physics",
            r#"{"methods":["physics.raycast_json_v1","physics.overlap_v1"]}"#,
        );
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].gateway, "engine.physics");
        assert_eq!(functions[0].name, "raycast");
        assert_eq!(functions[0].method, "physics.raycast_json_v1");
        assert!(functions[0].request_type.is_empty());
        assert_eq!(functions[1].name, "overlap");
    }

    #[test]
    fn malformed_or_methodless_descriptions_do_not_invent_bindings() {
        assert!(binding_functions_from_service_description("engine.test", "not-json").is_empty());
        assert!(binding_functions_from_service_description(
            "engine.test",
            r#"{"features":["something"]}"#,
        )
        .is_empty());
    }

    #[test]
    fn method_names_are_stable_typescript_identifiers() {
        assert_eq!(
            binding_name_from_method("world.spawn-actor_json_v1"),
            "spawn_actor"
        );
        assert_eq!(binding_name_from_method("engine.123_v1"), "_123");
    }
}
