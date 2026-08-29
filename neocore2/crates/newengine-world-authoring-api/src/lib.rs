#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral authored-world placement identity and transient authoring markers.
//!
//! This crate deliberately owns no scene, gameplay, renderer, physics, editor, or host
//! implementation. It is the message/component contract shared by authoring producers
//! and runtime consumers.

use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthoredMapPlacementSource {
    ProfilePrefab,
    DiscretePlacement,
}

/// Runtime-only authoring marker. It is attached only by editor mutations and
/// is cleared after a successful project save. Simulation must never set it.
#[derive(Clone, Copy, Debug, Default)]
pub struct AuthoredMapPlacementDirty;

/// Marks a live actor as a newly-created authored placement cloned from an
/// existing source element. It survives until the first successful project save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredMapPlacementCloneSource {
    pub placement_id: String,
}

impl AuthoredMapPlacementCloneSource {
    #[inline]
    pub fn new(placement_id: impl Into<String>) -> Self {
        Self {
            placement_id: placement_id.into(),
        }
    }
}

/// Runtime-only scale state for a derived replica that shares an authored placement
/// with the primary authoring actor. The owning subsystem decides how derived data
/// (for example collision) is rebuilt when the scale changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredMapPlacementReplicaScaleState {
    pub last_authored_scale: Vec3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredMapPlacement {
    pub map_ref: String,
    pub placement_id: String,
    pub source: AuthoredMapPlacementSource,
    /// True for the actor that owns authoring. False for runtime replicas generated
    /// from the same authored placement.
    pub primary: bool,
}

impl AuthoredMapPlacement {
    #[inline]
    pub fn new(
        map_ref: impl Into<String>,
        placement_id: impl Into<String>,
        source: AuthoredMapPlacementSource,
        primary: bool,
    ) -> Self {
        Self {
            map_ref: map_ref.into(),
            placement_id: placement_id.into(),
            source,
            primary,
        }
    }
}
