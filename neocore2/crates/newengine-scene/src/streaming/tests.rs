use std::collections::BTreeSet;

use super::*;

#[test]
fn desired_cell_stencil_is_ordered_and_translation_stable() {
    let origin = SceneCellCoord { x: 0, z: 0 };
    let shifted = SceneCellCoord { x: 100, z: -75 };
    let origin_cells = SceneResidencySet::desired_cells(origin, 3);
    let shifted_cells = SceneResidencySet::desired_cells(shifted, 3);
    assert_eq!(origin_cells.len(), 49);
    assert_eq!(shifted_cells.len(), origin_cells.len());
    assert!(origin_cells
        .windows(2)
        .all(|pair| pair[0].distance_key(origin) <= pair[1].distance_key(origin)));
    for (base, moved) in origin_cells.iter().zip(&shifted_cells) {
        assert_eq!(moved.x - shifted.x, base.x);
        assert_eq!(moved.z - shifted.z, base.z);
    }
}

#[test]
fn multi_focus_plan_keeps_primary_scene_and_prefetches_secondary_focus() {
    let center = SceneCellCoord { x: 0, z: 0 };
    let predicted = SceneCellCoord { x: 3, z: 0 };
    let budget = SceneStreamingBudget {
        resident_radius: 1,
        unload_radius: 4,
        max_commits_per_tick: 4,
    };
    let plan = SceneStreamingPlan::build_multi_focus(
        center,
        budget,
        [(predicted, 1)],
        std::iter::empty(),
        std::iter::empty(),
    );

    assert!(plan.desired.contains(&center));
    assert!(plan.desired.contains(&predicted));
    assert_eq!(
        plan.desired.iter().copied().collect::<BTreeSet<_>>().len(),
        plan.desired.len()
    );
}

#[test]
fn overlapping_focus_stencils_are_deduplicated_and_ordered() {
    let center = SceneCellCoord { x: 0, z: 0 };
    let desired = SceneResidencySet::desired_cells_for_focuses(
        center,
        2,
        [(SceneCellCoord { x: 1, z: 0 }, 2), (center, 1)],
    );
    assert_eq!(
        desired.iter().copied().collect::<BTreeSet<_>>().len(),
        desired.len()
    );
    assert!(desired
        .windows(2)
        .all(|pair| pair[0].distance_key(center) <= pair[1].distance_key(center)));
}

#[test]
fn load_requests_inherit_canonical_desired_priority_order() {
    let center = SceneCellCoord { x: 0, z: 0 };
    let desired = vec![
        SceneCellCoord { x: 3, z: 0 },
        SceneCellCoord { x: 1, z: 0 },
        SceneCellCoord { x: 2, z: 0 },
        SceneCellCoord { x: 1, z: 0 },
    ];
    let plan = SceneStreamingPlan::build_from_desired(
        center,
        SceneStreamingBudget::default(),
        desired,
        std::iter::empty(),
        std::iter::empty(),
    );
    assert_eq!(plan.loads.len(), 3);
    assert!(plan
        .loads
        .windows(2)
        .all(|pair| pair[0].priority_key <= pair[1].priority_key));
}

#[test]
fn predicted_bucket_is_prioritized_ahead_of_regular_far_cells() {
    let center = SceneCellCoord { x: 0, z: 0 };
    let predicted = SceneCellCoord { x: 3, z: 0 };
    let ordinary_far = SceneCellCoord { x: 0, z: 2 };
    let render = [center, ordinary_far, predicted];
    let plan = SceneBucketedCellPlan::from_desired_sets_with_prediction(
        center,
        render,
        render,
        [predicted],
    );

    let predicted_cell = plan
        .cells
        .iter()
        .find(|cell| cell.coord == predicted)
        .unwrap();
    let far_cell = plan
        .cells
        .iter()
        .find(|cell| cell.coord == ordinary_far)
        .unwrap();
    assert_eq!(predicted_cell.bucket, SceneStreamingBucket::PredictedNear);
    assert_eq!(far_cell.bucket, SceneStreamingBucket::VisibleFar);
    assert!(predicted_cell.score > far_cell.score);
}

#[test]
fn custom_layered_plan_preserves_unload_hysteresis_around_primary_center() {
    let center = SceneCellCoord { x: 0, z: 0 };
    let far_loaded = SceneCellCoord { x: -6, z: 0 };
    let profile = SceneStreamingProfile {
        render: SceneStreamingBudget {
            resident_radius: 1,
            unload_radius: 4,
            max_commits_per_tick: 2,
        },
        simulation: SceneStreamingBudget {
            resident_radius: 2,
            unload_radius: 5,
            max_commits_per_tick: 1,
        },
    };
    let render_desired = SceneResidencySet::desired_cells(center, 1);
    let simulation_desired = SceneResidencySet::desired_cells(center, 2);
    let plan = SceneLayeredStreamingPlan::build_from_desired(
        center,
        profile,
        render_desired,
        simulation_desired,
        [far_loaded],
        std::iter::empty(),
        std::iter::empty(),
        std::iter::empty(),
    );
    assert!(plan
        .render
        .unloads
        .iter()
        .any(|request| request.coord == far_loaded));
}
