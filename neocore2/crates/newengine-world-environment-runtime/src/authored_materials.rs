use newengine_material_domain_api::AuthoredMaterialSpec;
use newengine_material_runtime::authored_registration::register_authored_material;
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_world_environment_api::authored_profile::{
    AuthoredEnvironmentMaterialSetSpec, AuthoredEnvironmentPaletteSpec,
};

use crate::authored_sky::SkyVisualKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredEnvironmentMaterialRole {
    Terrain,
    Sky,
    TreeBark,
    TreeLeaf,
    TreeBranch,
}

#[derive(Clone, Copy)]
pub struct AuthoredEnvironmentMaterialDefinition<'a> {
    pub role: AuthoredEnvironmentMaterialRole,
    pub name: &'static str,
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    pub emissive_strength: f32,
    pub flags: MaterialFlags,
    pub spec: &'a AuthoredMaterialSpec,
}

#[derive(Clone, Copy)]
pub struct AuthoredEnvironmentMaterials {
    pub terrain: MaterialId,
    pub sky: MaterialId,
    pub tree_bark: MaterialId,
    pub tree_leaf: MaterialId,
    pub tree_branch: MaterialId,
}

impl AuthoredEnvironmentMaterials {
    fn from_registered(ids: &[(AuthoredEnvironmentMaterialRole, MaterialId)]) -> Self {
        fn find(
            ids: &[(AuthoredEnvironmentMaterialRole, MaterialId)],
            role: AuthoredEnvironmentMaterialRole,
        ) -> MaterialId {
            ids.iter()
                .find_map(|(candidate, id)| (*candidate == role).then_some(*id))
                .expect("all environment material roles are registered from canonical table")
        }
        Self {
            terrain: find(ids, AuthoredEnvironmentMaterialRole::Terrain),
            sky: find(ids, AuthoredEnvironmentMaterialRole::Sky),
            tree_bark: find(ids, AuthoredEnvironmentMaterialRole::TreeBark),
            tree_leaf: find(ids, AuthoredEnvironmentMaterialRole::TreeLeaf),
            tree_branch: find(ids, AuthoredEnvironmentMaterialRole::TreeBranch),
        }
    }

    #[inline]
    pub fn sky_visual_material(self, kind: SkyVisualKind) -> MaterialId {
        match kind {
            SkyVisualKind::Dome => self.sky,
        }
    }
}

#[inline]
pub fn register_authored_environment_material_definition(
    materials: &MaterialRegistry,
    definition: AuthoredEnvironmentMaterialDefinition<'_>,
) -> (AuthoredEnvironmentMaterialRole, MaterialId) {
    let id = register_authored_material(
        materials,
        definition.name,
        definition.base_color,
        definition.emissive,
        definition.emissive_strength,
        definition.flags,
        definition.spec,
    );
    (definition.role, id)
}

#[inline]
pub fn register_authored_environment_materials(
    materials: &MaterialRegistry,
    palette: &AuthoredEnvironmentPaletteSpec,
    specs: &AuthoredEnvironmentMaterialSetSpec,
) -> AuthoredEnvironmentMaterials {
    let definitions = [
        AuthoredEnvironmentMaterialDefinition {
            role: AuthoredEnvironmentMaterialRole::Terrain,
            name: "Environment/ProceduralTerrain",
            base_color: palette.terrain,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &specs.terrain,
        },
        AuthoredEnvironmentMaterialDefinition {
            role: AuthoredEnvironmentMaterialRole::Sky,
            name: "Environment/SkyDome",
            base_color: palette.sky,
            emissive: palette.sky_emissive,
            emissive_strength: 2.6,
            flags: MaterialFlags::DOUBLE_SIDED,
            spec: &specs.sky,
        },
        AuthoredEnvironmentMaterialDefinition {
            role: AuthoredEnvironmentMaterialRole::TreeBark,
            name: "Environment/Tree/Bark",
            base_color: palette.tree_bark,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &specs.tree_bark,
        },
        AuthoredEnvironmentMaterialDefinition {
            role: AuthoredEnvironmentMaterialRole::TreeLeaf,
            name: "Environment/Tree/Leaf",
            base_color: palette.tree_leaf,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::ALPHA_TEST)
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &specs.tree_leaf,
        },
        AuthoredEnvironmentMaterialDefinition {
            role: AuthoredEnvironmentMaterialRole::TreeBranch,
            name: "Environment/Tree/Branch",
            base_color: palette.tree_branch,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &specs.tree_branch,
        },
    ];
    let registered = definitions
        .into_iter()
        .map(|definition| register_authored_environment_material_definition(materials, definition))
        .collect::<Vec<_>>();
    AuthoredEnvironmentMaterials::from_registered(&registered)
}
