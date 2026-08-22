use super::*;

#[derive(Debug, Clone)]
pub(crate) struct RegisteredServiceFact {
    pub(crate) service_id: String,
    pub(crate) owner_plugin_id: Option<String>,
}

impl RegisteredServiceFact {
    #[inline]
    pub(crate) fn new(service_id: String, owner_plugin_id: Option<String>) -> Self {
        Self {
            service_id,
            owner_plugin_id,
        }
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
        Self {
            plugin_id,
            descriptor,
            origin,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayProviderRouteFact {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_route_id: String,
    pub(crate) provider_abi: Option<String>,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin: GatewayProviderOrigin,
    pub(crate) system_tags: Vec<String>,
}

impl GatewayProviderRouteFact {
    #[cfg(test)]
    #[inline]
    pub(crate) fn new(
        gateway_id: String,
        service_kind: EngineServiceKind,
        provider_service_id: String,
        provider_route_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
    ) -> Self {
        Self::new_dynamic(
            gateway_id,
            service_kind.as_str().to_owned(),
            provider_service_id,
            provider_route_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            [system_tag::ENGINE_DOMAIN, system_tag::PROVIDER_BACKEND],
        )
    }

    #[cfg(test)]
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn new_dynamic<I, S>(
        gateway_id: String,
        service_kind: String,
        provider_service_id: String,
        provider_route_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        system_tags: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new_dynamic_with_origin(
            gateway_id,
            service_kind,
            provider_service_id,
            provider_route_id,
            None,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            GatewayProviderOrigin::EngineRuntime,
            system_tags,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn new_dynamic_with_origin<I, S>(
        gateway_id: String,
        service_kind: String,
        provider_service_id: String,
        provider_route_id: String,
        provider_abi: Option<String>,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        origin: GatewayProviderOrigin,
        system_tags: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut system_tags = system_tags
            .into_iter()
            .filter_map(|tag| newengine_service_api::normalize_system_tag(tag.as_ref()))
            .collect::<Vec<_>>();
        system_tags.sort();
        system_tags.dedup();
        Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_route_id,
            provider_abi,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            origin,
            system_tags,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayPolicyFact {
    pub(crate) gateway_id: String,
    pub(crate) override_mode: Option<GatewayOverrideMode>,
    pub(crate) system_tags: Vec<String>,
    pub(crate) preferred_system_tags: Vec<String>,
    pub(crate) forbidden_system_tags: Vec<String>,
    pub(crate) preference_bonus: i32,
    pub(crate) owner_id: String,
}

#[cfg(test)]
impl GatewayPolicyFact {
    #[inline]
    pub(crate) fn new<I, S>(
        gateway_id: String,
        override_mode: GatewayOverrideMode,
        system_tags: I,
        owner_id: String,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut system_tags = system_tags
            .into_iter()
            .filter_map(|tag| newengine_service_api::normalize_system_tag(tag.as_ref()))
            .collect::<Vec<_>>();
        system_tags.sort();
        system_tags.dedup();
        Self {
            gateway_id,
            override_mode: Some(override_mode),
            system_tags,
            preferred_system_tags: Vec::new(),
            forbidden_system_tags: Vec::new(),
            preference_bonus: 0,
            owner_id,
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
    EngineRuntime,
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
            Self::EngineRuntime => "engine-runtime",
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
            Self::EngineRuntime => 10_000,
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

        if normalized
            .iter()
            .any(|part| matches!(part.as_str(), "mods" | "mod" | "user_mods"))
        {
            return Self::UserMod;
        }

        if normalized.iter().any(|part| {
            matches!(
                part.as_str(),
                "gameplugins" | "game-plugins" | "profileplugins"
            )
        }) {
            return Self::GamePlugin;
        }

        if normalized
            .iter()
            .any(|part| matches!(part.as_str(), "plugins" | "plugin"))
        {
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

    pub(super) fn from_system_tags(tags: &[String]) -> Option<Self> {
        if tags
            .iter()
            .any(|tag| tag == system_tag::OVERRIDE_LOCKED || tag == system_tag::TRUST_ROOT)
        {
            return Some(Self::Locked);
        }
        if tags
            .iter()
            .any(|tag| tag == system_tag::OVERRIDE_PROFILE_CONTROLLED)
        {
            return Some(Self::ProfileControlled);
        }
        if tags.iter().any(|tag| tag == system_tag::OVERRIDE_OPEN) {
            return Some(Self::Open);
        }
        None
    }
}
