#![forbid(unsafe_op_in_unsafe_fn)]

mod client;
mod runtime_module;
mod service_api;
mod types;

pub use runtime_module::PhysicsBackendRuntimeModule;
pub use types::ResolvedPhysicsBackendConfig;

/// Backend-neutral implementation unit consumed by the generic runtime-unit catalog.
///
/// `physics.backend` is the logical capability that activates this bridge. The unit itself
/// provides the in-process runtime PhysicsApi and binds whatever provider was selected by
/// the immutable CompositionPlan; it never selects Jolt/Bullet/etc. by name.
pub const PHYSICS_RUNTIME_UNIT_ID: &str = "engine.runtime-adapter.physics";
pub const PHYSICS_RUNTIME_UNIT_SPEC: newengine_service_api::EngineRuntimeUnitSpec =
    newengine_service_api::EngineRuntimeUnitSpec::new(
        PHYSICS_RUNTIME_UNIT_ID,
        1,
        newengine_service_api::EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.physics-api"],
        &[newengine_physics_api::PHYSICS_BACKEND_CAPABILITY_ID],
        &["engine.runtime-unit", "backend-neutral", "service-adapter"],
    );
fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    startup: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    Ok(Some(Box::new(PhysicsBackendRuntimeModule::new(
        startup.modules_dir.clone(),
    ))))
}

/// Statically linked materializer owned by this adapter crate.
pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        PHYSICS_RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
