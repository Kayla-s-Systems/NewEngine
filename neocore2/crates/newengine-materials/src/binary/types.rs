use crate::api::MaterialDescriptor;

/// A named material asset stored inside a `.nemat` container.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialBinaryAsset {
    /// Human-readable material name stored in UTF-8.
    pub name: String,
    /// Material descriptor payload.
    pub desc: MaterialDescriptor,
}
