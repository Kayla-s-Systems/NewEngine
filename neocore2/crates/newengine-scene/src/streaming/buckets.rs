use std::collections::HashSet;

use super::cell::SceneCellCoord;

/// Coarse active-scene bucket inspired by mature open-world streamers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneStreamingBucket {
    ActiveSimulation,
    VisibleNear,
    VisibleFar,
    PredictedNear,
    SimulationOnly,
    InvisibleFar,
    Sleeping,
}

impl SceneStreamingBucket {
    #[inline]
    pub const fn priority(self) -> i32 {
        match self {
            Self::ActiveSimulation => 700,
            Self::VisibleNear => 650,
            Self::PredictedNear => 580,
            Self::VisibleFar => 520,
            Self::SimulationOnly => 360,
            Self::InvisibleFar => 160,
            Self::Sleeping => 0,
        }
    }

    #[inline]
    pub const fn wants_render_residency(self) -> bool {
        matches!(
            self,
            Self::VisibleNear | Self::VisibleFar | Self::PredictedNear
        )
    }

    #[inline]
    pub const fn wants_simulation_residency(self) -> bool {
        !matches!(self, Self::InvisibleFar | Self::Sleeping)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneBucketedCell {
    pub coord: SceneCellCoord,
    pub bucket: SceneStreamingBucket,
    pub score: i32,
}

impl SceneBucketedCell {
    #[inline]
    pub fn new(
        coord: SceneCellCoord,
        bucket: SceneStreamingBucket,
        center: SceneCellCoord,
    ) -> Self {
        let distance_penalty = coord.manhattan_distance(center).saturating_mul(12);
        Self {
            coord,
            bucket,
            score: bucket.priority().saturating_sub(distance_penalty),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneBucketedCellPlan {
    pub cells: Vec<SceneBucketedCell>,
}

impl SceneBucketedCellPlan {
    pub fn from_desired_sets(
        center: SceneCellCoord,
        render_desired: impl IntoIterator<Item = SceneCellCoord>,
        simulation_desired: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        Self::from_desired_sets_with_prediction(
            center,
            render_desired,
            simulation_desired,
            std::iter::empty(),
        )
    }

    /// Classify desired cells while preserving a distinct predictive bucket.
    pub fn from_desired_sets_with_prediction(
        center: SceneCellCoord,
        render_desired: impl IntoIterator<Item = SceneCellCoord>,
        simulation_desired: impl IntoIterator<Item = SceneCellCoord>,
        predicted_desired: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let render_set = render_desired.into_iter().collect::<HashSet<_>>();
        let simulation_set = simulation_desired.into_iter().collect::<HashSet<_>>();
        let predicted_set = predicted_desired.into_iter().collect::<HashSet<_>>();

        // `HashSet::union` is unique by construction, so the old trailing `dedup`
        // after sorting could never remove an element.
        let mut all = Vec::with_capacity(render_set.len().saturating_add(simulation_set.len()));
        all.extend(render_set.iter().copied());
        all.extend(simulation_set.difference(&render_set).copied());
        all.sort_by_key(|coord| coord.distance_key(center));

        let mut cells = all
            .into_iter()
            .map(|coord| {
                let dist = coord.chebyshev_distance(center);
                let bucket = if render_set.contains(&coord) {
                    if dist <= 1 {
                        SceneStreamingBucket::VisibleNear
                    } else if predicted_set.contains(&coord) {
                        SceneStreamingBucket::PredictedNear
                    } else {
                        SceneStreamingBucket::VisibleFar
                    }
                } else if simulation_set.contains(&coord) {
                    SceneStreamingBucket::SimulationOnly
                } else {
                    SceneStreamingBucket::Sleeping
                };
                SceneBucketedCell::new(coord, bucket, center)
            })
            .collect::<Vec<_>>();
        cells.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| {
                a.coord
                    .distance_key(center)
                    .cmp(&b.coord.distance_key(center))
            })
        });
        Self { cells }
    }
}
