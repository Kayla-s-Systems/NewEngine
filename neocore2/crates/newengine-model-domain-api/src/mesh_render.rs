use serde::{Deserialize, Serialize};

/// Semantic render role for a resolved mesh/drawable instance.
///
/// This is domain metadata, not renderer-owned GPU state. Render extraction uses
/// the role to choose a pass; render providers still own pipelines, buffers and
/// draw execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshRenderRole {
    #[default]
    WorldOpaque,
    WorldMasked,
    WorldTransparent,

    TerrainPatch,
    FoliageInstanced,
    CharacterBody,
    FirstPersonViewModel,

    SkyBackground,
    CelestialBillboard,
    WeatherVolume,

    Decal,
    DebugPrimitive,
    EditorGizmo,
    CollisionProxy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshTransformPolicy {
    #[default]
    World,
    FollowCamera,
    ViewLocked,
    ScreenSpace,
    BoneAttached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshVisibilityPolicy {
    #[default]
    Frustum,
    FrustumAndDistance,
    AlwaysVisible,
    EditorOnly,
    DebugOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshDepthPolicy {
    #[default]
    ReadWrite,
    ReadOnly,
    Disabled,
    SkyBackgroundDepth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshShadowPolicy {
    None,
    CastOnly,
    ReceiveOnly,
    CastAndReceive,
    #[default]
    ProfileControlled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshCullPolicy {
    #[default]
    BackFace,
    FrontFace,
    None,
    ProfileControlled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MeshSortPolicy {
    #[default]
    Opaque,
    Transparent,
    SkyFirst,
    DebugLast,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshRenderOptions {
    pub role: MeshRenderRole,
    pub transform_policy: MeshTransformPolicy,
    pub visibility_policy: MeshVisibilityPolicy,
    pub depth_policy: MeshDepthPolicy,
    pub shadow_policy: MeshShadowPolicy,
    pub cull_policy: MeshCullPolicy,
    pub sort_policy: MeshSortPolicy,
}

impl Default for MeshRenderOptions {
    fn default() -> Self {
        Self::world_opaque()
    }
}

impl MeshRenderOptions {
    pub fn world_opaque() -> Self {
        Self {
            role: MeshRenderRole::WorldOpaque,
            transform_policy: MeshTransformPolicy::World,
            visibility_policy: MeshVisibilityPolicy::FrustumAndDistance,
            depth_policy: MeshDepthPolicy::ReadWrite,
            shadow_policy: MeshShadowPolicy::ProfileControlled,
            cull_policy: MeshCullPolicy::BackFace,
            sort_policy: MeshSortPolicy::Opaque,
        }
    }

    pub fn world_masked() -> Self {
        Self {
            role: MeshRenderRole::WorldMasked,
            transform_policy: MeshTransformPolicy::World,
            visibility_policy: MeshVisibilityPolicy::FrustumAndDistance,
            depth_policy: MeshDepthPolicy::ReadWrite,
            shadow_policy: MeshShadowPolicy::ProfileControlled,
            cull_policy: MeshCullPolicy::ProfileControlled,
            sort_policy: MeshSortPolicy::Opaque,
        }
    }

    pub fn terrain_patch() -> Self {
        Self {
            role: MeshRenderRole::TerrainPatch,
            transform_policy: MeshTransformPolicy::World,
            visibility_policy: MeshVisibilityPolicy::FrustumAndDistance,
            depth_policy: MeshDepthPolicy::ReadWrite,
            shadow_policy: MeshShadowPolicy::ReceiveOnly,
            cull_policy: MeshCullPolicy::BackFace,
            sort_policy: MeshSortPolicy::Opaque,
        }
    }

    pub fn foliage_instanced() -> Self {
        Self {
            role: MeshRenderRole::FoliageInstanced,
            transform_policy: MeshTransformPolicy::World,
            visibility_policy: MeshVisibilityPolicy::FrustumAndDistance,
            depth_policy: MeshDepthPolicy::ReadWrite,
            shadow_policy: MeshShadowPolicy::CastAndReceive,
            cull_policy: MeshCullPolicy::BackFace,
            sort_policy: MeshSortPolicy::Opaque,
        }
    }

    pub fn character_body() -> Self {
        Self {
            role: MeshRenderRole::CharacterBody,
            transform_policy: MeshTransformPolicy::World,
            visibility_policy: MeshVisibilityPolicy::FrustumAndDistance,
            depth_policy: MeshDepthPolicy::ReadWrite,
            shadow_policy: MeshShadowPolicy::CastAndReceive,
            cull_policy: MeshCullPolicy::BackFace,
            sort_policy: MeshSortPolicy::Opaque,
        }
    }

    pub fn first_person_view_model() -> Self {
        Self {
            role: MeshRenderRole::FirstPersonViewModel,
            transform_policy: MeshTransformPolicy::ViewLocked,
            visibility_policy: MeshVisibilityPolicy::AlwaysVisible,
            depth_policy: MeshDepthPolicy::ReadOnly,
            shadow_policy: MeshShadowPolicy::None,
            cull_policy: MeshCullPolicy::BackFace,
            sort_policy: MeshSortPolicy::Opaque,
        }
    }

    pub fn sky_background() -> Self {
        Self {
            role: MeshRenderRole::SkyBackground,
            transform_policy: MeshTransformPolicy::FollowCamera,
            visibility_policy: MeshVisibilityPolicy::AlwaysVisible,
            depth_policy: MeshDepthPolicy::SkyBackgroundDepth,
            shadow_policy: MeshShadowPolicy::None,
            cull_policy: MeshCullPolicy::None,
            sort_policy: MeshSortPolicy::SkyFirst,
        }
    }

    pub fn celestial_billboard() -> Self {
        Self {
            role: MeshRenderRole::CelestialBillboard,
            transform_policy: MeshTransformPolicy::FollowCamera,
            visibility_policy: MeshVisibilityPolicy::AlwaysVisible,
            depth_policy: MeshDepthPolicy::Disabled,
            shadow_policy: MeshShadowPolicy::None,
            cull_policy: MeshCullPolicy::None,
            sort_policy: MeshSortPolicy::SkyFirst,
        }
    }

    #[inline]
    pub fn is_sky_role(&self) -> bool {
        matches!(
            self.role,
            MeshRenderRole::SkyBackground
                | MeshRenderRole::CelestialBillboard
                | MeshRenderRole::WeatherVolume
        )
    }
}
