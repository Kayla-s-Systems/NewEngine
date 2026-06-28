use serde::{Deserialize, Serialize};

use crate::ModelSkeletonMetadata;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelAssetRequest {
    pub model: String,
    pub manifest: Option<String>,
    pub skeleton: Option<String>,
    pub texture_dictionary: Option<String>,
    pub collisions: Vec<ModelCollisionRef>,
    pub target_height: f32,
    pub eye_height_ratio: f32,
}

impl Default for ModelAssetRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            manifest: None,
            skeleton: None,
            texture_dictionary: None,
            collisions: Vec::new(),
            target_height: 1.8,
            eye_height_ratio: 0.91,
        }
    }
}

impl ModelAssetRequest {
    #[inline]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_manifest(mut self, manifest: impl Into<String>) -> Self {
        self.manifest = Some(manifest.into());
        self
    }

    #[inline]
    pub fn with_skeleton(mut self, skeleton: impl Into<String>) -> Self {
        self.skeleton = Some(skeleton.into());
        self
    }

    #[inline]
    pub fn with_texture_dictionary(mut self, dictionary: impl Into<String>) -> Self {
        self.texture_dictionary = Some(dictionary.into());
        self
    }

    #[inline]
    pub fn with_collision(mut self, collision: ModelCollisionRef) -> Self {
        self.collisions.push(collision);
        self
    }

    #[inline]
    pub fn with_human_scale(mut self, target_height: f32, eye_height_ratio: f32) -> Self {
        self.target_height = target_height;
        self.eye_height_ratio = eye_height_ratio;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelAssetBundle {
    pub source: String,
    pub parts: Vec<ModelMeshPart>,
    pub skeleton: Option<ModelSkeletonMetadata>,
    pub texture_dictionary: Option<String>,
    pub collisions: Vec<ModelCollisionRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelMeshPart {
    pub material_slot: String,
    pub mesh: newengine_primitives::PrimitiveMesh,
    pub material: ModelMaterialBinding,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelMaterialBinding {
    pub slot: String,
    /// Preferred authored material selector, e.g. `materials/foo.nemat@bar`.
    /// Importers may project a renderer-agnostic descriptor, but runtime graph
    /// resolution should follow this reference.
    pub material_ref: Option<String>,
    pub descriptor: newengine_materials::MaterialDescriptor,
    pub textures: newengine_materials::MaterialTextureBindings,
    pub fallback_color: [f32; 4],
    pub resolution_policy: String,
}

impl Default for ModelMaterialBinding {
    fn default() -> Self {
        let descriptor = newengine_materials::MaterialDescriptor::default();
        let fallback_color = descriptor.base_color;
        Self {
            slot: String::new(),
            material_ref: None,
            descriptor,
            textures: newengine_materials::MaterialTextureBindings::default(),
            fallback_color,
            resolution_policy: "runtime_strict_ydd_nemat_ytd_chain".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConstructionManifest {
    pub id: String,
    pub model: String,
    pub skeleton: Option<ModelSkeletonRef>,
    pub material_set: ModelMaterialSetRef,
    pub collisions: Vec<ModelCollisionRef>,
    pub target_height: f32,
    pub eye_height_ratio: f32,
}

impl Default for ModelConstructionManifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            model: String::new(),
            skeleton: None,
            material_set: ModelMaterialSetRef::default(),
            collisions: Vec::new(),
            target_height: 1.8,
            eye_height_ratio: 0.91,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelSkeletonRef {
    pub source: String,
    pub format: String,
    pub humanoid_profile: Option<String>,
}

impl Default for ModelSkeletonRef {
    fn default() -> Self {
        Self {
            source: String::new(),
            format: "auto".to_owned(),
            humanoid_profile: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelMaterialSetRef {
    pub texture_dictionary: Option<String>,
    pub material_manifest: Option<String>,
}

impl Default for ModelMaterialSetRef {
    fn default() -> Self {
        Self {
            texture_dictionary: None,
            material_manifest: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelCollisionRef {
    pub name: String,
    pub kind: ModelCollisionKind,
    pub anchor: Option<String>,
    pub radius: f32,
    pub half_height: f32,
    pub half_extents: [f32; 3],
    pub mesh: Option<String>,
}

impl Default for ModelCollisionRef {
    fn default() -> Self {
        Self {
            name: "body".to_owned(),
            kind: ModelCollisionKind::Capsule,
            anchor: Some("hips".to_owned()),
            radius: 0.32,
            half_height: 0.82,
            half_extents: [0.32, 0.82, 0.32],
            mesh: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCollisionKind {
    None,
    Capsule,
    Box,
    Sphere,
    Mesh,
}

impl Default for ModelCollisionKind {
    fn default() -> Self {
        Self::Capsule
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelConstructionValidation {
    pub valid: bool,
    pub resolved: Option<ModelAssetRequest>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
