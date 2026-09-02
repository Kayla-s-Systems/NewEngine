    use super::unexpected_zero_pass_submit;

    #[test]
    fn backend_deferred_zero_pass_is_not_a_render_regression() {
        let report = newengine_core::render::RenderGraphSubmitReport {
            executed_passes: 0,
            skipped_passes: 7,
            backend_deferred: true,
            ..Default::default()
        };
        assert!(!unexpected_zero_pass_submit(7, &report));
    }

    #[test]
    fn admitted_non_empty_graph_with_zero_passes_is_still_a_regression() {
        let report = newengine_core::render::RenderGraphSubmitReport {
            executed_passes: 0,
            skipped_passes: 7,
            backend_deferred: false,
            ..Default::default()
        };
        assert!(unexpected_zero_pass_submit(7, &report));
    }
