#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use newengine_plugin_api::PluginDescriptor;
use newengine_service_api::EngineServiceKind;

use super::metadata::descriptor_gateway_capabilities;
use super::provider::gateway_provider_service_id;

#[derive(Debug, Clone)]
pub(crate) struct RegisteredServiceFact {
    pub(crate) service_id: String,
    pub(crate) owner_plugin_id: Option<String>,
}

impl RegisteredServiceFact {
    #[inline]
    pub(crate) fn new(service_id: String, owner_plugin_id: Option<String>) -> Self {
        Self { service_id, owner_plugin_id }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PluginDescriptorFact {
    pub(crate) plugin_id: String,
    pub(crate) descriptor: PluginDescriptor,
    pub(crate) origin: GatewayProviderOrigin,
}

impl PluginDescriptorFact {
    #[inline]
    pub(crate) fn new(
        plugin_id: String,
        descriptor: PluginDescriptor,
        origin: GatewayProviderOrigin,
    ) -> Self {
        Self { plugin_id, descriptor, origin }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EngineOwnedGatewayFact {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: EngineServiceKind,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
}

impl EngineOwnedGatewayFact {
    #[inline]
    pub(crate) fn new(
        gateway_id: String,
        service_kind: EngineServiceKind,
        provider_service_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
    ) -> Self {
        Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
        }
    }
}

/// Host-assigned trust/origin tier used by gateway provider selection.
///
/// Plugins must not be trusted to self-declare this value in descriptor JSON.
/// The host/loader assigns it from the load source, profile metadata or dev
/// override policy before descriptor facts enter `ActiveGatewayRegistry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GatewayProviderOrigin {
    NullProvider,
    EngineOwned,
    FirstPartyPlugin,
    GamePlugin,
    UserMod,
    DevOverride,
}

impl GatewayProviderOrigin {
    #[inline]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NullProvider => "null-provider",
            Self::EngineOwned => "engine-owned",
            Self::FirstPartyPlugin => "first-party-plugin",
            Self::GamePlugin => "game-plugin",
            Self::UserMod => "user-mod",
            Self::DevOverride => "dev-override",
        }
    }

    #[inline]
    pub(crate) const fn origin_bias(self) -> i64 {
        match self {
            Self::DevOverride => 50_000,
            Self::UserMod => 40_000,
            Self::GamePlugin => 30_000,
            Self::FirstPartyPlugin => 20_000,
            Self::EngineOwned => 10_000,
            Self::NullProvider => 0,
        }
    }

    /// Conservative host-side classification used when no profile policy has
    /// supplied an explicit origin. This is intentionally path-derived and
    /// best-effort only; the descriptor JSON is never trusted for origin.
    pub(crate) fn from_plugin_path(path: &Path) -> Self {
        let normalized = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .map(|s| s.to_ascii_lowercase())
            .collect::<Vec<_>>();

        if normalized.iter().any(|part| {
            matches!(
                part.as_str(),
                "devoverrides" | "dev-overrides" | "dev_override"
            )
        }) {
            return Self::DevOverride;
        }

        if normalized.iter().any(|part| matches!(part.as_str(), "mods" | "mod" | "user_mods")) {
            return Self::UserMod;
        }

        if normalized.iter().any(|part| {
            matches!(part.as_str(), "gameplugins" | "game-plugins" | "profileplugins")
        }) {
            return Self::GamePlugin;
        }

        if normalized.iter().any(|part| matches!(part.as_str(), "plugins" | "plugin")) {
            return Self::FirstPartyPlugin;
        }

        Self::GamePlugin
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GatewayOverrideMode {
    Open,
    ProfileControlled,
    Locked,
}

impl GatewayOverrideMode {
    #[inline]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::ProfileControlled => "profile-controlled",
            Self::Locked => "locked",
        }
    }
}

#[inline]
fn gateway_override_mode(gateway_id: &str) -> GatewayOverrideMode {
    match gateway_id {
        "engine.plugin_host"
        | "engine.abi"
        | "engine.gateway_registry"
        | "engine.security"
        | "engine.scheduler.core"
        | "engine.capability_validator" => return GatewayOverrideMode::Locked,
        "engine.save" | "engine.network" => return GatewayOverrideMode::ProfileControlled,
        _ => {}
    }

    match EngineServiceKind::parse_engine_gateway_id(gateway_id) {
        Some(
            EngineServiceKind::Render
            | EngineServiceKind::Model
            | EngineServiceKind::ModelSkeletons
            | EngineServiceKind::ModelMaterials
            | EngineServiceKind::ModelCollisions
            | EngineServiceKind::Physics
            | EngineServiceKind::PhysicsContacts
            | EngineServiceKind::PhysicsConstraints
            | EngineServiceKind::Assets
            | EngineServiceKind::Materials
            | EngineServiceKind::Scene,
        ) => GatewayOverrideMode::ProfileControlled,
        Some(
            EngineServiceKind::RenderEffects
            | EngineServiceKind::RenderMaterials
            | EngineServiceKind::AssetFileTypes
            | EngineServiceKind::Input
            | EngineServiceKind::InputBindings
            | EngineServiceKind::InputActions
            | EngineServiceKind::InputContexts
            | EngineServiceKind::Camera
            | EngineServiceKind::CameraModes
            | EngineServiceKind::CameraAnimations
            | EngineServiceKind::Audio
            | EngineServiceKind::Ui
            | EngineServiceKind::Logging
            | EngineServiceKind::Loading
            | EngineServiceKind::Platform
            | EngineServiceKind::Ecs
            | EngineServiceKind::Entity
            | EngineServiceKind::PluginHost
            | EngineServiceKind::Abi
            | EngineServiceKind::GatewayRegistry
            | EngineServiceKind::Security
            | EngineServiceKind::SchedulerCore
            | EngineServiceKind::CapabilityValidator,
        ) => GatewayOverrideMode::Open,
        None => GatewayOverrideMode::Open,
    }
}

#[inline]
fn route_allowed_by_default_policy(gateway_id: &str, origin: GatewayProviderOrigin) -> bool {
    match gateway_override_mode(gateway_id) {
        GatewayOverrideMode::Open => true,
        // Profile-controlled gateways are still selectable by default; profile
        // loading can later clamp this further. The important safety rule here
        // is that locked trust-root gateways cannot be taken over by plugins.
        GatewayOverrideMode::ProfileControlled => true,
        GatewayOverrideMode::Locked => matches!(origin, GatewayProviderOrigin::EngineOwned),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveGatewayRoute {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: EngineServiceKind,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin: GatewayProviderOrigin,
    pub(crate) override_mode: GatewayOverrideMode,
    pub(crate) active_score: i64,
}

impl ActiveGatewayRoute {
    #[inline]
    fn new(
        gateway_id: String,
        service_kind: EngineServiceKind,
        provider_service_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        origin: GatewayProviderOrigin,
    ) -> Option<Self> {
        if !service_kind.matches_engine_gateway_id(&gateway_id) {
            log::warn!(
                "gateways: ignoring route with mixed domain levels gateway='{}' service_kind='{}' expected_gateway='{}' service='{}' owner='{}'",
                gateway_id,
                service_kind.as_str(),
                service_kind.engine_gateway_id(),
                provider_service_id,
                provider_owner_id,
            );
            return None;
        }

        if !route_allowed_by_default_policy(&gateway_id, origin) {
            log::warn!(
                "gateways: ignoring route blocked by override policy gateway='{}' service='{}' owner='{}' origin='{}' mode='{}'",
                gateway_id,
                provider_service_id,
                provider_owner_id,
                origin.as_str(),
                gateway_override_mode(&gateway_id).as_str(),
            );
            return None;
        }

        let override_mode = gateway_override_mode(&gateway_id);
        let active_score = origin.origin_bias() + i64::from(backend_priority);
        Some(Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            origin,
            override_mode,
            active_score,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveGatewayRegistry {
    routes: Vec<ActiveGatewayRoute>,
}

impl ActiveGatewayRegistry {
    pub(crate) fn from_facts(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        engine_owned_gateways: &[EngineOwnedGatewayFact],
    ) -> Self {
        let mut routes = Vec::new();

        for descriptor_fact in descriptors {
            for gateway in descriptor_gateway_capabilities(&descriptor_fact.descriptor) {
                let Some(provider_service_id) =
                    gateway_provider_service_id(&descriptor_fact.descriptor, &gateway)
                else {
                    continue;
                };

                let registered = services.iter().any(|service| {
                    service.service_id == provider_service_id
                        && service.owner_plugin_id.as_deref()
                            == Some(descriptor_fact.plugin_id.as_str())
                });
                if !registered {
                    continue;
                }

                if let Some(route) = ActiveGatewayRoute::new(
                    gateway.gateway_id,
                    gateway.service_kind,
                    provider_service_id,
                    descriptor_fact.plugin_id.clone(),
                    gateway.backend_capability_id,
                    gateway.backend_priority,
                    descriptor_fact.origin,
                ) {
                    routes.push(route);
                }
            }
        }

        for gateway in engine_owned_gateways {
            let registered = services.iter().any(|service| {
                service.service_id == gateway.provider_service_id && service.owner_plugin_id.is_none()
            });
            if !registered {
                continue;
            }

            if let Some(route) = ActiveGatewayRoute::new(
                gateway.gateway_id.clone(),
                gateway.service_kind,
                gateway.provider_service_id.clone(),
                gateway.provider_owner_id.clone(),
                gateway.backend_capability_id.clone(),
                gateway.backend_priority,
                GatewayProviderOrigin::EngineOwned,
            ) {
                routes.push(route);
            }
        }

        routes.sort_by(|a, b| {
            a.gateway_id
                .cmp(&b.gateway_id)
                .then_with(|| b.active_score.cmp(&a.active_score))
                .then_with(|| b.backend_priority.cmp(&a.backend_priority))
                .then_with(|| b.origin.origin_bias().cmp(&a.origin.origin_bias()))
                .then_with(|| a.service_kind.as_str().cmp(b.service_kind.as_str()))
                .then_with(|| a.provider_service_id.cmp(&b.provider_service_id))
                .then_with(|| a.provider_owner_id.cmp(&b.provider_owner_id))
        });

        Self { routes }
    }

    pub(crate) fn routes(&self) -> &[ActiveGatewayRoute] {
        &self.routes
    }

    pub(crate) fn gateway_ids(&self) -> Vec<String> {
        let mut out = self.routes.iter().map(|route| route.gateway_id.clone()).collect::<Vec<_>>();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn resolve_gateway(&self, gateway_id: &str) -> Option<String> {
        self.resolve_route(gateway_id)
            .map(|route| route.provider_service_id.clone())
    }

    pub(crate) fn resolve_route(&self, gateway_id: &str) -> Option<&ActiveGatewayRoute> {
        self.routes
            .iter()
            .filter(|route| route.gateway_id == gateway_id)
            .max_by(|a, b| {
                a.active_score
                    .cmp(&b.active_score)
                    .then_with(|| a.backend_priority.cmp(&b.backend_priority))
                    .then_with(|| a.origin.origin_bias().cmp(&b.origin.origin_bias()))
                    .then_with(|| b.provider_service_id.cmp(&a.provider_service_id))
                    .then_with(|| b.provider_owner_id.cmp(&a.provider_owner_id))
            })
    }

    pub(crate) fn has_gateway_capability(&self, gateway_id: &str, capability_id: &str) -> bool {
        self.routes.iter().any(|route| {
            route.gateway_id == gateway_id && route.backend_capability_id == capability_id
        })
    }
}

pub(crate) fn descriptor_engine_gateways(descriptor: &PluginDescriptor) -> Vec<String> {
    let mut out: Vec<String> = descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter_map(|gateway| gateway_provider_service_id(descriptor, &gateway).map(|_| gateway.gateway_id))
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn descriptor_max_gateway_priority(descriptor: &PluginDescriptor) -> i32 {
    descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter(|gateway| gateway_provider_service_id(descriptor, gateway).is_some())
        .map(|gateway| gateway.backend_priority)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use newengine_plugin_api::{CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor, PluginKind};

    use super::*;

    fn descriptor(
        plugin_id: &str,
        provider_service_id: &str,
        gateway_id: &str,
        backend_capability_id: &str,
        service_kind: &str,
        backend_priority: i32,
    ) -> PluginDescriptor {
        PluginDescriptor::builder(plugin_id, plugin_id, "1.0.0", PluginKind::Runtime)
            .provides_service(provider_service_id, 1, r#"{"methods":["info_json"]}"#)
            .push(
                CapabilityDesc::new(
                    backend_capability_id,
                    CapabilityRole::Provides,
                    CapabilityKind::Other,
                    1,
                )
                .with_json(format!(
                    r#"{{"service_kind":"{}","engine_gateway":"{}","contract":"{}","backend_priority":{}}}"#,
                    service_kind, gateway_id, provider_service_id, backend_priority
                )),
            )
            .build()
    }

    fn service(service_id: &str, owner: Option<&str>) -> RegisteredServiceFact {
        RegisteredServiceFact::new(
            service_id.to_owned(),
            owner.map(std::borrow::ToOwned::to_owned),
        )
    }

    #[test]
    fn plugin_origin_tier_overrides_engine_owned_even_with_lower_backend_priority() {
        let descriptors = vec![PluginDescriptorFact::new(
            "mod.camera".to_owned(),
            descriptor(
                "mod.camera",
                "mod.camera.api",
                "engine.camera",
                "camera.backend",
                "camera",
                0,
            ),
            GatewayProviderOrigin::UserMod,
        )];
        let services = vec![
            service("mod.camera.api", Some("mod.camera")),
            service("engine.camera", None),
        ];
        let engine_owned = vec![EngineOwnedGatewayFact::new(
            "engine.camera".to_owned(),
            EngineServiceKind::Camera,
            "engine.camera".to_owned(),
            "newengine-engine-runtime.camera-gateway".to_owned(),
            "camera.backend".to_owned(),
            5_000,
        )];

        let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &engine_owned);
        let route = registry.resolve_route("engine.camera").expect("engine.camera route");

        assert_eq!(route.provider_service_id, "mod.camera.api");
        assert_eq!(route.origin, GatewayProviderOrigin::UserMod);
        assert_eq!(route.active_score, 40_000);
    }

    #[test]
    fn engine_owned_is_used_when_no_plugin_provider_exists() {
        let services = vec![service("engine.camera", None)];
        let engine_owned = vec![EngineOwnedGatewayFact::new(
            "engine.camera".to_owned(),
            EngineServiceKind::Camera,
            "engine.camera".to_owned(),
            "newengine-engine-runtime.camera-gateway".to_owned(),
            "camera.backend".to_owned(),
            0,
        )];

        let registry = ActiveGatewayRegistry::from_facts(&[], &services, &engine_owned);
        let route = registry.resolve_route("engine.camera").expect("engine.camera route");

        assert_eq!(route.provider_service_id, "engine.camera");
        assert_eq!(route.origin, GatewayProviderOrigin::EngineOwned);
        assert_eq!(route.active_score, 10_000);
    }

    #[test]
    fn higher_priority_wins_inside_same_origin_tier() {
        let descriptors = vec![
            PluginDescriptorFact::new(
                "mod.camera.low".to_owned(),
                descriptor(
                    "mod.camera.low",
                    "mod.camera.low.api",
                    "engine.camera",
                    "camera.backend",
                    "camera",
                    10,
                ),
                GatewayProviderOrigin::UserMod,
            ),
            PluginDescriptorFact::new(
                "mod.camera.high".to_owned(),
                descriptor(
                    "mod.camera.high",
                    "mod.camera.high.api",
                    "engine.camera",
                    "camera.backend",
                    "camera",
                    20,
                ),
                GatewayProviderOrigin::UserMod,
            ),
        ];
        let services = vec![
            service("mod.camera.low.api", Some("mod.camera.low")),
            service("mod.camera.high.api", Some("mod.camera.high")),
        ];

        let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
        let route = registry.resolve_route("engine.camera").expect("engine.camera route");

        assert_eq!(route.provider_service_id, "mod.camera.high.api");
        assert_eq!(route.active_score, 40_020);
    }

    #[test]
    fn locked_gateway_rejects_plugin_route() {
        let descriptors = vec![PluginDescriptorFact::new(
            "mod.security".to_owned(),
            descriptor(
                "mod.security",
                "mod.security.api",
                "engine.security",
                "security.backend",
                "security",
                99_999,
            ),
            GatewayProviderOrigin::DevOverride,
        )];
        let services = vec![
            service("mod.security.api", Some("mod.security")),
            service("engine.security", None),
        ];
        let engine_owned = vec![EngineOwnedGatewayFact::new(
            "engine.security".to_owned(),
            EngineServiceKind::Security,
            "engine.security".to_owned(),
            "newengine.security".to_owned(),
            "security.backend".to_owned(),
            0,
        )];

        let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &engine_owned);
        let route = registry.resolve_route("engine.security").expect("engine.security route");

        assert_eq!(route.provider_service_id, "engine.security");
        assert_eq!(route.origin, GatewayProviderOrigin::EngineOwned);
    }

    #[test]
    fn tie_breakers_are_deterministic() {
        let descriptors = vec![
            PluginDescriptorFact::new(
                "mod.b".to_owned(),
                descriptor("mod.b", "b.camera.api", "engine.camera", "camera.backend", "camera", 1),
                GatewayProviderOrigin::UserMod,
            ),
            PluginDescriptorFact::new(
                "mod.a".to_owned(),
                descriptor("mod.a", "a.camera.api", "engine.camera", "camera.backend", "camera", 1),
                GatewayProviderOrigin::UserMod,
            ),
        ];
        let services = vec![
            service("b.camera.api", Some("mod.b")),
            service("a.camera.api", Some("mod.a")),
        ];

        let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
        let route = registry.resolve_route("engine.camera").expect("engine.camera route");

        assert_eq!(route.provider_service_id, "a.camera.api");
    }
    #[test]
    fn child_domain_route_is_selected_when_kind_and_gateway_match() {
        let descriptors = vec![PluginDescriptorFact::new(
            "mod.input.bindings".to_owned(),
            descriptor(
                "mod.input.bindings",
                "mod.input.bindings.api",
                "engine.input.bindings",
                "input.bindings.backend",
                "input.bindings",
                7,
            ),
            GatewayProviderOrigin::UserMod,
        )];
        let services = vec![service("mod.input.bindings.api", Some("mod.input.bindings"))];

        let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
        let route = registry.resolve_route("engine.input.bindings").expect("engine.input.bindings route");

        assert_eq!(route.service_kind, EngineServiceKind::InputBindings);
        assert_eq!(route.provider_service_id, "mod.input.bindings.api");
    }

    #[test]
    fn mixed_parent_and_child_domain_route_is_ignored() {
        let descriptors = vec![PluginDescriptorFact::new(
            "bad.input".to_owned(),
            descriptor(
                "bad.input",
                "bad.input.api",
                "engine.input.bindings",
                "input.bindings.backend",
                "input",
                100,
            ),
            GatewayProviderOrigin::UserMod,
        )];
        let services = vec![service("bad.input.api", Some("bad.input"))];

        let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);

        assert!(registry.resolve_route("engine.input.bindings").is_none());
    }
}
