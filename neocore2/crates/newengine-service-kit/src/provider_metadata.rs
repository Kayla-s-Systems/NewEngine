#![forbid(unsafe_op_in_unsafe_fn)]

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct EngineGatewayProviderServiceDescription {
    pub id: String,
    pub version: u32,
    pub origin: &'static str,
    pub owner: String,
    pub capability: String,
    pub methods: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl EngineGatewayProviderServiceDescription {
    #[inline]
    pub fn new<I, M>(
        id: impl Into<String>,
        owner: impl Into<String>,
        capability: impl Into<String>,
        methods: I,
    ) -> Self
    where
        I: IntoIterator<Item = M>,
        M: Into<String>,
    {
        Self {
            id: id.into(),
            version: 1,
            origin: "engine-runtime",
            owner: owner.into(),
            capability: capability.into(),
            methods: methods.into_iter().map(Into::into).collect(),
            protocol: None,
            features: Vec::new(),
            gateway: None,
            notes: None,
        }
    }

    #[inline]
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    #[inline]
    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = Some(protocol.into());
        self
    }

    #[inline]
    pub fn features<I, F>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = F>,
        F: Into<String>,
    {
        self.features = features.into_iter().map(Into::into).collect();
        self
    }

    #[inline]
    pub fn gateway(mut self, gateway: impl Into<String>) -> Self {
        self.gateway = Some(gateway.into());
        self
    }

    #[inline]
    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

#[inline]
pub fn engine_gateway_provider_service_description<I, M>(
    id: impl Into<String>,
    owner: impl Into<String>,
    capability: impl Into<String>,
    methods: I,
) -> EngineGatewayProviderServiceDescription
where
    I: IntoIterator<Item = M>,
    M: Into<String>,
{
    EngineGatewayProviderServiceDescription::new(id, owner, capability, methods)
}
