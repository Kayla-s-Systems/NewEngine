use crate::api::MaterialDescriptor;

/// A binary material container.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialBinaryAsset {
    pub name: String,
    pub desc: MaterialDescriptor,
}
