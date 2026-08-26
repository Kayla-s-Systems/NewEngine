use super::*;

// Editor accessors are grouped by action handling, actor mutation, snapshots, hierarchy, and UI publication.
include!("editor/selection_actions.rs");
include!("editor/actor_mutation.rs");
include!("editor/snapshots.rs");
include!("editor/hierarchy.rs");
include!("editor/publishing.rs");
