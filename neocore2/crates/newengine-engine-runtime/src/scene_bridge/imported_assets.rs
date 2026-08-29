#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_materials::MaterialId;
use newengine_primitives::PrimitiveId;

use crate::gameplay::DisplayMode;
use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PrimitiveMaterialBase {
    pub id: MaterialId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneImportedAssetKind {
    StaticMesh,
    SceneReference,
    TextureReference,
    MaterialReference,
    OpaqueReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneImportedAssetRepresentation {
    PrimitiveCube,
    PrimitivePlane,
    PrimitiveSphere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneImportedAssetAssemblyKind {
    StaticMeshActor,
    SceneAnchor,
    TextureCard,
    MaterialPreviewSphere,
    OpaqueProxy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneImportedAssetAssemblyDescriptor {
    pub assembly: SceneImportedAssetAssemblyKind,
    pub primitive_id: PrimitiveId,
    pub display_mode: DisplayMode,
    pub with_collision: bool,
    pub dynamic_collision: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneImportedAssetDescriptor {
    pub logical_path: String,
    pub import_kind: SceneImportedAssetKind,
    pub representation: SceneImportedAssetRepresentation,
    pub assembler_key: String,
    pub assembly: SceneImportedAssetAssemblyDescriptor,
    pub default_scale: [f32; 3],
    pub tint: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneImportedAssetAssembler {
    pub key: String,
    pub label: &'static str,
    pub import_kind: SceneImportedAssetKind,
    pub assembly: SceneImportedAssetAssemblyKind,
}

#[inline]
pub(super) fn imported_asset_primitive_id(
    descriptor: &SceneImportedAssetDescriptor,
) -> PrimitiveId {
    descriptor.assembly.primitive_id
}

#[inline]
pub(super) fn imported_asset_collision(
    descriptor: &SceneImportedAssetDescriptor,
) -> Option<PhysicsBodyDesc> {
    if !descriptor.assembly.with_collision {
        return None;
    }
    match descriptor.assembly.assembly {
        SceneImportedAssetAssemblyKind::StaticMeshActor
        | SceneImportedAssetAssemblyKind::OpaqueProxy => {
            Some(if descriptor.assembly.dynamic_collision {
                PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Box {
                    half_extents: [
                        descriptor.default_scale[0].abs().max(0.5),
                        descriptor.default_scale[1].abs().max(0.5),
                        descriptor.default_scale[2].abs().max(0.5),
                    ],
                })
            } else {
                PhysicsBodyDesc::static_solid(CollisionShapeDesc::Box {
                    half_extents: [
                        descriptor.default_scale[0].abs().max(0.5),
                        descriptor.default_scale[1].abs().max(0.5),
                        descriptor.default_scale[2].abs().max(0.5),
                    ],
                })
            })
        }
        SceneImportedAssetAssemblyKind::TextureCard => {
            Some(PhysicsBodyDesc::trigger(CollisionShapeDesc::Box {
                half_extents: [
                    descriptor.default_scale[0].abs().max(0.25),
                    0.05,
                    descriptor.default_scale[2].abs().max(0.25),
                ],
            }))
        }
        SceneImportedAssetAssemblyKind::MaterialPreviewSphere => {
            Some(PhysicsBodyDesc::static_solid(CollisionShapeDesc::Sphere {
                radius: descriptor.default_scale[0].abs().max(0.5),
            }))
        }
        SceneImportedAssetAssemblyKind::SceneAnchor => None,
    }
}

#[inline]
pub(super) fn builtin_asset_assemblers() -> Vec<SceneImportedAssetAssembler> {
    vec![
        SceneImportedAssetAssembler {
            key: "builtin.static_mesh_actor".to_string(),
            label: "Static Mesh Actor",
            import_kind: SceneImportedAssetKind::StaticMesh,
            assembly: SceneImportedAssetAssemblyKind::StaticMeshActor,
        },
        SceneImportedAssetAssembler {
            key: "builtin.scene_anchor".to_string(),
            label: "Scene Anchor",
            import_kind: SceneImportedAssetKind::SceneReference,
            assembly: SceneImportedAssetAssemblyKind::SceneAnchor,
        },
        SceneImportedAssetAssembler {
            key: "builtin.texture_card".to_string(),
            label: "Texture Card",
            import_kind: SceneImportedAssetKind::TextureReference,
            assembly: SceneImportedAssetAssemblyKind::TextureCard,
        },
        SceneImportedAssetAssembler {
            key: "builtin.material_preview_sphere".to_string(),
            label: "Material Preview Sphere",
            import_kind: SceneImportedAssetKind::MaterialReference,
            assembly: SceneImportedAssetAssemblyKind::MaterialPreviewSphere,
        },
        SceneImportedAssetAssembler {
            key: "builtin.opaque_proxy".to_string(),
            label: "Opaque Proxy",
            import_kind: SceneImportedAssetKind::OpaqueReference,
            assembly: SceneImportedAssetAssemblyKind::OpaqueProxy,
        },
    ]
}

#[inline]
pub(super) fn resolve_asset_assembler(
    registry: &[SceneImportedAssetAssembler],
    descriptor: &SceneImportedAssetDescriptor,
) -> SceneImportedAssetAssembler {
    registry
        .iter()
        .find(|it| it.key == descriptor.assembler_key)
        .cloned()
        .or_else(|| {
            registry
                .iter()
                .find(|it| {
                    it.import_kind == descriptor.import_kind
                        && it.assembly == descriptor.assembly.assembly
                })
                .cloned()
        })
        .unwrap_or_else(|| SceneImportedAssetAssembler {
            key: "builtin.fallback".to_string(),
            label: "Fallback Opaque Proxy",
            import_kind: descriptor.import_kind,
            assembly: descriptor.assembly.assembly,
        })
}
