#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned visibility control plane.
//!
//! This crate deliberately owns no GPU query objects. It consumes broad world
//! candidates plus delayed provider observations and produces stable visibility,
//! query demand, streaming pressure and LOD significance.

use std::collections::{BTreeMap, BTreeSet};

use newengine_math::Vec3;
use newengine_scene::{SceneActivationDomains, SceneActivationPlan, SceneCellCoord};
use newengine_visibility_api::{
    VisibilityObservationV1, VisibilityQueryBatchV1, VisibilityQueryCandidateV1,
    VisibilityResultBatchV1, VisibilitySphereV1, VisibilityVec3V1, VisibilityViewV1,
};

include!("visibility/types.rs");
include!("visibility/control.rs");
include!("visibility/heuristics.rs");
include!("visibility/tests.rs");
