#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::ApiVersion;
use newengine_service_api::{hash_u128, InterfaceId};

pub const PROVIDER_CONTRACT_V1: ApiVersion = ApiVersion::new(1, 0, 0);

pub const I_GAMEPLAY_SYSTEM_PROVIDER_V1: InterfaceId =
    InterfaceId::new(hash_u128("newengine.gameplay.IGameplaySystemProvider.v1"));
pub const I_GAMEPLAY_CONTENT_PROVIDER_V1: InterfaceId =
    InterfaceId::new(hash_u128("newengine.gameplay.IGameplayContentProvider.v1"));
pub const I_GAMEPLAY_UI_PROVIDER_V1: InterfaceId =
    InterfaceId::new(hash_u128("newengine.gameplay.IGameplayUiProvider.v1"));
pub const I_GAMEPLAY_PHYSICS_QUERY_PROVIDER_V1: InterfaceId = InterfaceId::new(hash_u128(
    "newengine.gameplay.IGameplayPhysicsQueryProvider.v1",
));
pub const I_SCENE_BOOTSTRAP_PROVIDER_V1: InterfaceId =
    InterfaceId::new(hash_u128("newengine.scene.ISceneBootstrapProvider.v1"));
pub const I_WORLD_RUNTIME_PROVIDER_V1: InterfaceId =
    InterfaceId::new(hash_u128("newengine.world.IWorldRuntimeProvider.v1"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeProviderDescriptor {
    pub id: &'static str,
    pub interface_id: InterfaceId,
    pub version: ApiVersion,
    pub capabilities: &'static [&'static str],
}

impl RuntimeProviderDescriptor {
    #[inline]
    pub const fn new(
        id: &'static str,
        interface_id: InterfaceId,
        version: ApiVersion,
        capabilities: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            interface_id,
            version,
            capabilities,
        }
    }

    #[inline]
    pub const fn gameplay_system(id: &'static str) -> Self {
        Self::new(
            id,
            I_GAMEPLAY_SYSTEM_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
            &["gameplay.system.phases"],
        )
    }

    #[inline]
    pub const fn gameplay_content(id: &'static str) -> Self {
        Self::new(
            id,
            I_GAMEPLAY_CONTENT_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
            &["gameplay.content.install"],
        )
    }

    #[inline]
    pub const fn gameplay_ui(id: &'static str) -> Self {
        Self::new(
            id,
            I_GAMEPLAY_UI_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
            &["gameplay.ui.frame", "gameplay.ui.input-capture"],
        )
    }

    #[inline]
    pub const fn gameplay_physics_queries(id: &'static str) -> Self {
        Self::new(
            id,
            I_GAMEPLAY_PHYSICS_QUERY_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
            &["gameplay.physics.queries"],
        )
    }

    #[inline]
    pub const fn world_runtime(id: &'static str) -> Self {
        Self::new(
            id,
            I_WORLD_RUNTIME_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
            &["world.runtime.tick", "world.runtime.prelaunch"],
        )
    }

    #[inline]
    pub const fn scene_bootstrap(id: &'static str) -> Self {
        Self::new(
            id,
            I_SCENE_BOOTSTRAP_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
            &["scene.bootstrap"],
        )
    }
}

pub fn validate_provider_contract(
    descriptor: RuntimeProviderDescriptor,
    expected_interface: InterfaceId,
    minimum_version: ApiVersion,
) -> Result<(), String> {
    if descriptor.id.trim().is_empty() {
        return Err("provider descriptor id is empty".to_owned());
    }
    if descriptor.interface_id != expected_interface {
        return Err(format!(
            "provider '{}' interface mismatch expected={:?} got={:?}",
            descriptor.id, expected_interface, descriptor.interface_id
        ));
    }
    if descriptor.version < minimum_version {
        return Err(format!(
            "provider '{}' contract version too old expected>={}.{}.{} got={}.{}.{}",
            descriptor.id,
            minimum_version.major,
            minimum_version.minor,
            minimum_version.patch,
            descriptor.version.major,
            descriptor.version.minor,
            descriptor.version.patch,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_ids_distinguish_provider_contract_families() {
        assert_ne!(
            I_GAMEPLAY_SYSTEM_PROVIDER_V1,
            I_GAMEPLAY_CONTENT_PROVIDER_V1
        );
        assert_ne!(I_GAMEPLAY_SYSTEM_PROVIDER_V1, I_SCENE_BOOTSTRAP_PROVIDER_V1);
        assert_ne!(I_GAMEPLAY_SYSTEM_PROVIDER_V1, I_WORLD_RUNTIME_PROVIDER_V1);
    }

    #[test]
    fn provider_validation_rejects_wrong_interface() {
        let descriptor = RuntimeProviderDescriptor::scene_bootstrap("test.scene");
        assert!(validate_provider_contract(
            descriptor,
            I_GAMEPLAY_SYSTEM_PROVIDER_V1,
            PROVIDER_CONTRACT_V1,
        )
        .is_err());
    }
}
