#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_tags_api::{
    tags_method, TagDescriptorV1, TagDomain, TagId, TagsDescribeRequestV1, TagsDescribeResponseV1,
    TagsResolveRequestV1, TagsResolveResponseV1, TagsServiceInfoV1, TagsSnapshotRequestV1,
    TagsSnapshotResponseV1, TagsValidateSetRequestV1, TagsValidateSetResponseV1,
    TAGS_REGISTRY_CAPABILITY_ID, TAGS_SERVICE_ID, TAGS_SERVICE_METHODS,
};
use serde::Deserialize;
use serde_json::Value;

pub const PROVIDER_ROUTE: &str = "engine.tags.foundation";
const OWNER: &str = "newengine-tags-runtime.foundation-provider";

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    tags: Vec<RawTag>,
}

#[derive(Debug, Deserialize)]
struct RawTag {
    tag: String,
    #[serde(default)]
    domain: Value,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    description: String,
}

#[derive(Default)]
struct TagsState {
    tags: BTreeMap<String, TagDescriptorV1>,
    source: String,
}

impl TagsState {
    fn load_from_json(text: &str, source: impl Into<String>) -> Result<Self, String> {
        let config: Config = serde_json::from_str(text)
            .map_err(|error| format!("tags catalog decode failed: {error}"))?;
        let tags = config
            .tags
            .into_iter()
            .map(|raw| {
                let descriptor = TagDescriptorV1 {
                    tag: TagId::new(raw.tag),
                    domain: parse_domain(&raw.domain),
                    display_name: raw.display_name,
                    description: raw.description,
                    parent: None,
                    aliases: Vec::new(),
                };
                (descriptor.tag.0.clone(), descriptor)
            })
            .collect();
        Ok(Self {
            tags,
            source: source.into(),
        })
    }

    fn load_from_startup(startup: &newengine_core::StartupConfig) -> Result<Self, String> {
        let (path, text) =
            newengine_core::read_plugin_runtime_data_string(startup, PROVIDER_ROUTE, "catalog")?;
        Self::load_from_json(&text, path.to_string_lossy())
    }

    fn info(&self) -> TagsServiceInfoV1 {
        TagsServiceInfoV1::default()
    }

    fn describe(&self, req: TagsDescribeRequestV1) -> TagsDescribeResponseV1 {
        let domain_filter = req.domain_filter.unwrap_or_default().to_ascii_lowercase();
        let tags = self
            .tags
            .values()
            .filter(|tag| {
                domain_filter.is_empty()
                    || format!("{:?}", tag.domain).to_ascii_lowercase() == domain_filter
            })
            .cloned()
            .collect();
        TagsDescribeResponseV1 {
            accepted: true,
            tags,
            diagnostics: Vec::new(),
        }
    }

    fn resolve(&self, req: TagsResolveRequestV1) -> TagsResolveResponseV1 {
        let descriptor = self.tags.get(req.tag.trim()).cloned();
        TagsResolveResponseV1 {
            accepted: descriptor.is_some(),
            descriptor,
            diagnostics: if req.tag.trim().is_empty() {
                vec!["tag is empty".to_owned()]
            } else {
                Vec::new()
            },
        }
    }

    fn snapshot(&self, req: TagsSnapshotRequestV1) -> TagsSnapshotResponseV1 {
        TagsSnapshotResponseV1 {
            accepted: true,
            sets: vec![newengine_tags_api::TagSetSnapshotV1 {
                owner: if req.owner_prefix.is_empty() {
                    "engine.tags.registry".to_owned()
                } else {
                    req.owner_prefix
                },
                entity: None,
                tags: self.tags.keys().cloned().map(TagId::new).collect(),
                source: self.source.clone(),
            }],
            diagnostics: Vec::new(),
        }
    }

    fn validate(&self, req: TagsValidateSetRequestV1) -> TagsValidateSetResponseV1 {
        let (normalized_tags, unknown_tags): (Vec<_>, Vec<_>) = req
            .tags
            .into_iter()
            .partition(|tag| self.tags.contains_key(tag.as_str()));
        TagsValidateSetResponseV1 {
            accepted: unknown_tags.is_empty(),
            normalized_tags,
            unknown_tags,
            diagnostics: Vec::new(),
        }
    }
}

fn parse_domain(value: &Value) -> TagDomain {
    match value.as_str().unwrap_or_default() {
        "Gameplay" => TagDomain::Gameplay,
        "State" => TagDomain::State,
        "Faction" => TagDomain::Faction,
        "Item" => TagDomain::Item,
        "Weapon" => TagDomain::Weapon,
        "Mission" => TagDomain::Mission,
        "Animation" => TagDomain::Animation,
        "Navigation" => TagDomain::Navigation,
        "Debug" => TagDomain::Debug,
        other if !other.is_empty() => TagDomain::Custom(other.to_owned()),
        _ => TagDomain::Gameplay,
    }
}

fn envelope(payload: &Blob, default_method: &str) -> Result<(String, Value), RString> {
    let value = payload_json(payload).map_err(RString::from)?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or(default_method)
        .to_owned();
    Ok((method, value.get("request").cloned().unwrap_or(Value::Null)))
}

fn invoke(state: &mut TagsState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, tags_method::DESCRIBE_TAGS_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    let decode = |e: serde_json::Error| RResult::RErr(RString::from(e.to_string()));
    match method.as_str() {
        tags_method::INFO_JSON => ok_json(state.info()),
        tags_method::DESCRIBE_TAGS_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.describe(v)),
            Err(e) => decode(e),
        },
        tags_method::RESOLVE_TAG_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.resolve(v)),
            Err(e) => decode(e),
        },
        tags_method::SNAPSHOT_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.snapshot(v)),
            Err(e) => decode(e),
        },
        tags_method::VALIDATE_TAG_SET_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.validate(v)),
            Err(e) => decode(e),
        },
        other => RResult::RErr(RString::from(format!(
            "tags.api: unknown invoke method '{other}'"
        ))),
    }
}

fn register_tags_gateway_with_state(state: TagsState) -> bool {
    let description = engine_gateway_provider_service_description(
        TAGS_SERVICE_ID, PROVIDER_ROUTE, TAGS_REGISTRY_CAPABILITY_ID, TAGS_SERVICE_METHODS.iter().copied(),
    ).gateway("engine.tags")
     .protocol("newengine.tags.foundation/v1")
     .features(["single-purpose-provider", "replaceable-gateway-route", "dto-only-boundary"])
     .notes("Owns only baseline tag registry semantics; no tasks, animation, navigation, AI, world mutation, or Host composition.");
    let service = JsonServiceRouter::with_state(TAGS_SERVICE_ID, state)
        .describe_json(&description)
        .get_json(tags_method::INFO_JSON, |state| state.info())
        .post_json(tags_method::DESCRIBE_TAGS_JSON_V1, |state, req| {
            state.describe(req)
        })
        .post_json(tags_method::RESOLVE_TAG_JSON_V1, |state, req| {
            state.resolve(req)
        })
        .post_json(tags_method::SNAPSHOT_JSON_V1, |state, req| {
            state.snapshot(req)
        })
        .post_json(tags_method::VALIDATE_TAG_SET_JSON_V1, |state, req| {
            state.validate(req)
        })
        .blob(tags_method::INVOKE_JSON, invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.tags",
        service_kind: newengine_service_api::EngineServiceKind::Tags,
        provider_service: TAGS_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: TAGS_REGISTRY_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service,
    })
    .is_ok()
}

pub fn register_tags_gateway_best_effort() -> bool {
    let Some(startup) = newengine_core::startup::last_startup_config() else {
        return false;
    };
    TagsState::load_from_startup(startup)
        .map(register_tags_gateway_with_state)
        .unwrap_or(false)
}

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.tags",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_tags_api::TAGS_REGISTRY_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    startup: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let state =
        TagsState::load_from_startup(startup).map_err(newengine_core::EngineError::other)?;
    if !register_tags_gateway_with_state(state) {
        return Err(newengine_core::EngineError::other(
            "failed to register configured tags provider",
        ));
    }
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_loads_tag_domain_only() {
        let state = TagsState::load_from_json(
            r#"{"tags":[{"tag":"state.idle","domain":"State"}]}"#,
            "test",
        )
        .unwrap();
        assert!(!state.tags.is_empty());
    }
}
