use crate::{EntityKind, EntityName};

/// Non-authoritative editor/runtime metadata attached to an entity.
///
/// This is not used for simulation logic. It exists to support tools and UX
/// such as inspector search and serialization of authoring data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityMeta {
    pub kind: EntityKind,
    pub name: EntityName,
    pub flags: u32,
}

impl Default for EntityMeta {
    #[inline]
    fn default() -> Self {
        Self {
            kind: EntityKind::new(0),
            name: EntityName::new(""),
            flags: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_default_is_unclassified_and_unnamed() {
        let metadata = EntityMeta::default();
        assert_eq!(metadata.kind.value(), 0);
        assert_eq!(metadata.name.as_str(), "");
        assert_eq!(metadata.flags, 0);
    }
}
