#![forbid(unsafe_op_in_unsafe_fn)]

mod buckets;
mod cell;
mod plan;
mod residency;

pub use buckets::{SceneBucketedCell, SceneBucketedCellPlan, SceneStreamingBucket};
pub use cell::{
    SceneCellCoord, SceneResidencyLayer, SceneStreamingBudget, SceneStreamingObserver,
    SceneStreamingProfile,
};
pub use plan::{
    SceneLayeredStreamingPlan, SceneStreamingPlan, SceneStreamingRequest, SceneStreamingRequestKind,
};
pub use residency::SceneResidencySet;

#[cfg(test)]
mod tests;
