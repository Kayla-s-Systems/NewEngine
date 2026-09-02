use super::*;

    fn index_with_cells(coords: &[(i32, i32)]) -> newengine_assets_api::MapIndexV1 {
        let mut index = newengine_assets_api::MapIndexV1 {
            map_id: "test".to_owned(),
            cell_size: 64.0,
            cells: coords
                .iter()
                .map(|(x, z)| {
                    newengine_assets_api::MapCellRefV1::canonical(AuthoredMapCellCoord::new(*x, *z))
                })
                .collect(),
            ..Default::default()
        };
        index.normalize();
        index
    }

    fn spec_with_cells(coords: &[(i32, i32)]) -> AuthoredMapStreamingSpec {
        AuthoredMapStreamingSpec {
            map_ref: "maps/test.ymap@map".to_owned(),
            index: index_with_cells(coords),
            initial_render_cells: vec![AuthoredMapCellCoord::new(0, 0)],
            initial_simulation_cells: vec![AuthoredMapCellCoord::new(0, 0)],
            initial_placement_ids: BTreeMap::new(),
            render_radius: 2,
            simulation_radius: 1,
            render_unload_radius: 3,
            simulation_unload_radius: 2,
            max_cells_per_tick: 2,
        }
    }

    fn deterministic_tuning() -> AuthoredMapStreamingRuntimeTuning {
        AuthoredMapStreamingRuntimeTuning {
            max_pending_jobs: 4,
            read_ahead_sec: 0.75,
            max_read_ahead_cells: 2,
            render_predict_radius: 1,
            simulation_predict_radius: 1,
            cell_jobs_limit: 2,
        }
    }

    #[test]
    fn desired_cell_generation_scales_with_radius_not_world_cell_count() {
        let spec = spec_with_cells(&[(-100, -100), (-1, -1), (0, 0), (1, 0), (1, 1), (100, 100)]);
        let controller = AuthoredMapStreamingController::new(&spec, deterministic_tuning());
        let (render, _) = controller.desired_domains(
            AuthoredMapCellCoord::new(0, 0),
            AuthoredMapCellCoord::new(0, 0),
        );
        assert!(render.contains(&AuthoredMapCellCoord::new(0, 0)));
        assert!(render.contains(&AuthoredMapCellCoord::new(1, 0)));
        assert!(render.contains(&AuthoredMapCellCoord::new(1, 1)));
        assert!(!render.contains(&AuthoredMapCellCoord::new(100, 100)));
    }

    #[test]
    fn controller_owns_prediction_and_dual_domain_desire() {
        let spec = spec_with_cells(&[(-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0), (0, 1)]);
        let mut controller = AuthoredMapStreamingController::new(&spec, deterministic_tuning());
        let focus = controller
            .focus_for_world_motion([1.0, 0.0, 1.0], [96.0, 0.0, 0.0])
            .expect("focus");
        assert_eq!(focus.center, AuthoredMapCellCoord::new(0, 0));
        assert!(focus.predicted_center.x >= focus.center.x);
        controller.replan(focus);
        assert!(controller.render_is_desired(AuthoredMapCellCoord::new(1, 0)));
        assert!(controller.simulation_is_desired(AuthoredMapCellCoord::new(0, 1)));
    }

    #[test]
    fn controller_emits_unload_plan_without_touching_ecs() {
        let mut spec = spec_with_cells(&[(-3, 0), (3, 0)]);
        spec.initial_render_cells = vec![AuthoredMapCellCoord::new(-3, 0)];
        spec.initial_simulation_cells = vec![AuthoredMapCellCoord::new(-3, 0)];
        let mut controller = AuthoredMapStreamingController::new(&spec, deterministic_tuning());
        let focus = AuthoredMapStreamingFocus {
            center: AuthoredMapCellCoord::new(3, 0),
            predicted_center: AuthoredMapCellCoord::new(3, 0),
            speed_mps: 0.0,
        };
        let plan = controller.replan(focus);
        assert_eq!(plan.unload_render, vec![AuthoredMapCellCoord::new(-3, 0)]);
        assert_eq!(
            plan.unload_simulation,
            vec![AuthoredMapCellCoord::new(-3, 0)]
        );
    }
