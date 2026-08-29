#![forbid(unsafe_op_in_unsafe_fn)]

pub use newengine_core::{Engine, EngineError, EngineResult, Module, ModuleCtx, StartupConfig};
pub use newengine_service_api::{
    EngineRuntimeUnitKind, EngineRuntimeUnitSpec, RuntimeUnitRequirementSpec,
};

/// Canonical tags for a statically linked provider runtime unit.
pub const STATIC_PROVIDER_TAGS: &[&str] = &[
    "engine.runtime-unit",
    "static",
    "first-party",
    "provider-route",
];

/// Canonical tags for a statically linked lifecycle-module runtime unit.
pub const STATIC_MODULE_TAGS: &[&str] = &[
    "engine.runtime-unit",
    "static",
    "first-party",
    "lifecycle-module",
];

/// Factory for one statically linked runtime-unit candidate.
pub type RuntimeUnitFactory =
    fn(&mut Engine<()>, &StartupConfig) -> EngineResult<Option<Box<dyn Module<()>>>>;

/// Provider-neutral binding between a runtime-unit descriptor and its materializer.
///
/// This contract deliberately contains no catalog, profile, Host, plugin discovery,
/// or product policy. Composition layers can exchange factory registrations without
/// depending on `newengine-runtime-host`.
#[derive(Clone, Copy)]
pub struct RuntimeUnitRegistration {
    pub spec: EngineRuntimeUnitSpec,
    pub factory: RuntimeUnitFactory,
}

impl RuntimeUnitRegistration {
    #[inline]
    pub const fn new(spec: EngineRuntimeUnitSpec, factory: RuntimeUnitFactory) -> Self {
        Self { spec, factory }
    }
}
