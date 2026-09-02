use newengine_materials::{MaterialFlags, MaterialRef, MaterialRegistry};
use newengine_vfx_runtime::VfxDecalMaterialAssetRef;

use super::material_source::register_required_material_ref;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VfxDecalMaterialLastError(u64);

#[inline]
fn fnv1a_64(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(14695981039346656037u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
        })
}

pub(crate) fn tick_vfx_decal_material_bindings(
    world: &mut newengine_ecs::World,
    materials: &MaterialRegistry,
) {
    let pending = world
        .query::<VfxDecalMaterialAssetRef>()
        .filter_map(|(entity, binding)| {
            world
                .get::<MaterialRef>(entity)
                .is_none()
                .then_some((entity, binding.logical_ref.clone()))
        })
        .collect::<Vec<_>>();

    for (entity, logical_ref) in pending {
        match register_required_material_ref(
            materials,
            &logical_ref,
            MaterialFlags::NONE,
            &logical_ref,
        ) {
            Ok(material_id) => {
                let _ = world.insert(entity, MaterialRef { id: material_id });
                let _ = world.remove::<VfxDecalMaterialLastError>(entity);
            }
            Err(error) => {
                let hash = fnv1a_64(&error);
                let changed = world
                    .get::<VfxDecalMaterialLastError>(entity)
                    .is_none_or(|previous| previous.0 != hash);
                if changed {
                    newengine_ulog_api::ulog::warn!(
                        "fps-character: VFX impact decal material deferred entity={} material='{}' err='{}'",
                        entity.stable_u64(),
                        logical_ref,
                        error,
                    );
                    let _ = world.insert(entity, VfxDecalMaterialLastError(hash));
                }
            }
        }
    }
}
