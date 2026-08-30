use std::{collections::BTreeSet, sync::OnceLock};

use super::cell::{SceneCellCoord, SceneStreamingBudget};

#[derive(Clone, Debug, Default)]
pub struct SceneResidencySet {
    cells: BTreeSet<SceneCellCoord>,
}

impl SceneResidencySet {
    #[inline]
    pub fn insert(&mut self, coord: SceneCellCoord) -> bool {
        self.cells.insert(coord)
    }

    #[inline]
    pub fn remove(&mut self, coord: &SceneCellCoord) -> bool {
        self.cells.remove(coord)
    }

    #[inline]
    pub fn contains(&self, coord: &SceneCellCoord) -> bool {
        self.cells.contains(coord)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = SceneCellCoord> + '_ {
        self.cells.iter().copied()
    }

    #[inline]
    pub fn desired_cells(center: SceneCellCoord, radius: i32) -> Vec<SceneCellCoord> {
        let radius = radius.clamp(0, SceneStreamingBudget::MAX_RESIDENT_RADIUS) as usize;
        ordered_residency_offsets()[radius]
            .iter()
            .map(|offset| SceneCellCoord {
                x: center.x.saturating_add(offset.x),
                z: center.z.saturating_add(offset.z),
            })
            .collect()
    }

    pub fn desired_cells_for_focuses(
        center: SceneCellCoord,
        radius: i32,
        secondary_focuses: impl IntoIterator<Item = (SceneCellCoord, i32)>,
    ) -> Vec<SceneCellCoord> {
        // Every authored focus stencil is bounded by MAX_RESIDENT_RADIUS (289 cells).
        // For the normal one/few-focus case, one contiguous vector plus a single sort
        // is cheaper and more cache-local than hashing every cell and then sorting the
        // hash table output anyway.
        let mut desired = Self::desired_cells(center, radius);
        for (focus, focus_radius) in secondary_focuses {
            desired.extend(Self::desired_cells(focus, focus_radius));
        }
        desired.sort_by_key(|coord| coord.distance_key(center));
        desired.dedup();
        desired
    }
}

fn ordered_residency_offsets() -> &'static Vec<Vec<SceneCellCoord>> {
    static OFFSETS: OnceLock<Vec<Vec<SceneCellCoord>>> = OnceLock::new();
    OFFSETS.get_or_init(|| {
        (0..=SceneStreamingBudget::MAX_RESIDENT_RADIUS)
            .map(|radius| {
                let side = (radius as usize).saturating_mul(2).saturating_add(1);
                let mut offsets = Vec::with_capacity(side.saturating_mul(side));
                for z in -radius..=radius {
                    for x in -radius..=radius {
                        offsets.push(SceneCellCoord { x, z });
                    }
                }
                offsets.sort_by_key(|coord| coord.distance_key(SceneCellCoord { x: 0, z: 0 }));
                offsets
            })
            .collect()
    })
}
